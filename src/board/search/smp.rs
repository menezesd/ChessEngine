//! Lazy SMP (Symmetric `MultiProcessing`) parallel search.
//!
//! Implements parallel search where multiple threads search the same position
//! independently with different depth offsets. All threads share a common
//! transposition table, which provides natural coordination.
//!
//! Key insights from chess programming community:
//! - Separate killer/history tables per thread reduce correlated pruning failures
//! - Helper threads searching at depth+1 populate TT for main thread
//! - Time-to-depth speedup is modest, but playing strength gains are significant

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::board::nnue::NnueNetwork;
use crate::board::{Board, Move};
use crate::tt::TranspositionTable;

use super::simple::simple_search;
use super::{SearchConfig, SearchInfoCallback, SearchParams, SearchResult, SearchState};

/// Shared state across all worker threads
pub struct SharedSearchState {
    /// Thread-safe transposition table
    pub tt: Arc<TranspositionTable>,
    /// Thread-safe pawn hash table
    pub pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
    /// Shared NNUE network (optional)
    pub nnue: Option<Arc<NnueNetwork>>,
    /// Stop flag checked by all workers
    pub stop: Arc<AtomicBool>,
    /// Global node counter (sum of all workers)
    pub total_nodes: Arc<AtomicU64>,
    /// Maximum selective depth seen
    pub max_seldepth: Arc<AtomicU64>,
    /// TT generation for aging
    pub generation: u16,
    /// Search parameters
    pub params: SearchParams,
}

impl SharedSearchState {
    /// Create with a specific TT, pawn hash table, and optional NNUE network
    pub fn new(
        tt: Arc<TranspositionTable>,
        pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
        nnue: Option<Arc<NnueNetwork>>,
        stop: Arc<AtomicBool>,
        generation: u16,
    ) -> Self {
        SharedSearchState {
            tt,
            pawn_hash,
            nnue,
            stop,
            total_nodes: Arc::new(AtomicU64::new(0)),
            max_seldepth: Arc::new(AtomicU64::new(0)),
            generation,
            params: SearchParams::default(),
        }
    }

    /// Update seldepth if this value is higher
    pub fn update_seldepth(&self, seldepth: u32) {
        let mut current = self.max_seldepth.load(Ordering::Relaxed);
        while seldepth as u64 > current {
            match self.max_seldepth.compare_exchange_weak(
                current,
                seldepth as u64,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Add nodes to global counter
    pub fn add_nodes(&self, nodes: u64) {
        self.total_nodes.fetch_add(nodes, Ordering::Relaxed);
    }
}

/// Result from a single worker thread
#[derive(Debug, Clone)]
pub struct WorkerResult {
    pub worker_id: usize,
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
}

/// Configuration for SMP search
#[derive(Clone)]
pub struct SmpConfig {
    /// Number of worker threads
    pub num_threads: usize,
    /// Maximum depth to search
    pub max_depth: u32,
    /// Time limit in milliseconds (0 = unlimited)
    pub time_limit_ms: u64,
    /// Node limit (0 = unlimited)
    pub node_limit: u64,
    /// Optional callback for iteration info
    pub info_callback: Option<SearchInfoCallback>,
}

impl Default for SmpConfig {
    fn default() -> Self {
        SmpConfig {
            num_threads: 1,
            max_depth: 64,
            time_limit_ms: 0,
            node_limit: 0,
            info_callback: None,
        }
    }
}

impl SmpConfig {
    /// Create config with specified thread count
    #[must_use]
    pub fn with_threads(num_threads: usize) -> Self {
        SmpConfig {
            num_threads: num_threads.max(1),
            ..Default::default()
        }
    }

    /// Set max depth
    #[must_use]
    pub fn depth(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set time limit
    #[must_use]
    pub fn time(mut self, time_limit_ms: u64) -> Self {
        self.time_limit_ms = time_limit_ms;
        self
    }

    /// Set node limit
    #[must_use]
    pub fn nodes(mut self, node_limit: u64) -> Self {
        self.node_limit = node_limit;
        self
    }

    /// Set info callback
    #[must_use]
    pub fn with_callback(mut self, callback: SearchInfoCallback) -> Self {
        self.info_callback = Some(callback);
        self
    }
}

/// Get depth offset for a worker thread.
///
/// Thread 0 (main): searches at target depth
/// Thread 1: searches at depth + 1 (populates TT with deeper entries)
/// Thread 2: searches at depth (different move order due to separate tables)
/// Thread 3: searches at depth + 1
/// etc.
fn worker_depth_offset(worker_id: usize) -> i32 {
    // Odd workers search deeper, even workers search at target depth
    #[allow(clippy::match_same_arms)]
    match worker_id % 4 {
        0 => 0, // Main worker: target depth
        1 => 1, // Search deeper
        2 => 0, // Same depth, different ordering
        3 => 1, // Search deeper
        _ => 0,
    }
}

/// Search thread stack size (32 MB to handle deep recursion)
const SEARCH_STACK_SIZE: usize = 32 * 1024 * 1024;

/// Run parallel search using Lazy SMP.
///
/// This spawns multiple worker threads that search the same position
/// independently. Workers share a transposition table but have separate
/// move ordering tables (killers, history, counter moves).
#[allow(clippy::needless_pass_by_value)] // Arc is cloned for thread sharing
pub fn smp_search(
    board: &Board,
    state: &mut SearchState,
    config: SmpConfig,
    stop: Arc<AtomicBool>,
) -> SearchResult {
    let num_threads = config.num_threads.max(1);

    // For single-threaded, use the existing path
    if num_threads == 1 {
        let mut board_clone = board.clone();
        let search_config = SearchConfig {
            max_depth: Some(config.max_depth),
            time_limit_ms: config.time_limit_ms,
            node_limit: config.node_limit,
            extract_ponder: true,
            info_callback: config.info_callback,
            multi_pv: 1, // SMP currently only supports single PV
        };
        return super::search(&mut board_clone, state, search_config, &stop);
    }

    // Increment generation for new search
    state.generation = state.generation.wrapping_add(1);
    state.stats.reset_search();

    // Create shared state with the TT, pawn hash, and NNUE from SearchState
    let shared = Arc::new(SharedSearchState::new(
        state.shared_tt(),
        state.shared_pawn_hash(),
        state.shared_nnue(),
        Arc::clone(&stop),
        state.generation,
    ));

    let start_time = Instant::now();
    let info_callback = config.info_callback.clone();
    let max_depth = config.max_depth;
    let time_limit_ms = config.time_limit_ms;
    let node_limit = config.node_limit;

    // Spawn worker threads
    let mut handles: Vec<JoinHandle<WorkerResult>> = Vec::with_capacity(num_threads);

    for worker_id in 0..num_threads {
        let board_clone = board.clone();
        let shared_clone = Arc::clone(&shared);
        let info_cb = if worker_id == 0 {
            info_callback.clone()
        } else {
            None // Only main worker reports info
        };

        let handle = thread::Builder::new()
            .name(format!("search-{worker_id}"))
            .stack_size(SEARCH_STACK_SIZE)
            .spawn(move || {
                run_worker(
                    worker_id,
                    board_clone,
                    shared_clone,
                    max_depth,
                    time_limit_ms,
                    node_limit,
                    info_cb,
                    start_time,
                )
            })
            .expect("failed to spawn search worker");

        handles.push(handle);
    }

    // Wait for all workers to complete
    let mut results: Vec<WorkerResult> = Vec::with_capacity(num_threads);
    for handle in handles {
        if let Ok(result) = handle.join() {
            results.push(result);
        }
    }

    // Update stats from shared counters
    state.stats.nodes = shared.total_nodes.load(Ordering::Relaxed);
    state.stats.seldepth = shared.max_seldepth.load(Ordering::Relaxed) as u32;

    // Select best result: prefer main worker (worker 0) as its search is most complete.
    // Only use helper results if main worker has no result.
    let main_result = results
        .iter()
        .find(|r| r.worker_id == 0 && r.best_move.is_some());
    let best_result = main_result.or_else(|| {
        results
            .iter()
            .filter(|r| r.best_move.is_some())
            .max_by_key(|r| r.depth)
    });

    let best_move = best_result.and_then(|r| r.best_move);

    // Extract ponder move from TT
    let ponder_move = best_move.and_then(|mv| {
        let mut temp_board = board.clone();
        let info = temp_board.make_move(mv);
        let ponder = shared.tt.probe(temp_board.hash).and_then(|entry| {
            entry.best_move().filter(|pmv| {
                let moves = temp_board.generate_moves();
                moves.iter().any(|m| m == pmv)
            })
        });
        temp_board.unmake_move(mv, info);
        ponder
    });

    SearchResult {
        best_move,
        ponder_move,
    }
}

/// Run a single worker thread
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn run_worker(
    worker_id: usize,
    mut board: Board,
    shared: Arc<SharedSearchState>,
    max_depth: u32,
    time_limit_ms: u64,
    node_limit: u64,
    info_callback: Option<SearchInfoCallback>,
    _start_time: Instant,
) -> WorkerResult {
    // Create local SearchState for this worker with shared TT, pawn hash, and NNUE
    let mut local_state = SearchState::with_shared_tables(
        Arc::clone(&shared.tt),
        Arc::clone(&shared.pawn_hash),
        shared.nnue.clone(),
        shared.generation,
    );
    local_state.params = shared.params.clone();

    // Reset local tables for this worker
    local_state.tables.history.decay();
    local_state.tables.killer_moves.reset();
    local_state.tables.counter_moves.reset();

    // Calculate this worker's depth offset
    // Helper threads search slightly deeper to populate TT for main thread
    let depth_offset = worker_depth_offset(worker_id);
    let search_depth = ((max_depth as i32) + depth_offset).max(1) as u32;

    // Run search with iterative deepening (handled internally by simple_search)
    // Each worker does full iterative deepening from depth 1 to search_depth
    let move_result = simple_search(
        &mut board,
        &mut local_state,
        search_depth,
        time_limit_ms,
        node_limit,
        &shared.stop,
        info_callback, // Main worker (id 0) reports info via callback
    );

    // Update shared stats
    shared.add_nodes(local_state.stats.nodes);
    shared.update_seldepth(local_state.stats.seldepth);

    // Get best move and score
    let best_move = move_result;
    let best_score = if let Some(entry) = shared.tt.probe(board.hash) {
        entry.score()
    } else {
        -30000i32
    };

    WorkerResult {
        worker_id,
        best_move,
        score: best_score,
        depth: search_depth,
        nodes: local_state.stats.total_nodes,
    }
}
