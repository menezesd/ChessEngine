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

mod worker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::board::Board;

use super::{
    constants::DEFAULT_MAX_DEPTH,
    simple::{immediate_root_result, RootSearchResolution},
    SearchConfig, SearchInfoCallback, SearchResult, SearchState,
};
use worker::{run_worker, WorkerSearchConfig};
pub use worker::{SharedSearchState, WorkerResult};

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
            max_depth: DEFAULT_MAX_DEPTH,
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

    /// Extract worker search config from SMP config
    fn to_worker_config(&self) -> WorkerSearchConfig {
        WorkerSearchConfig {
            max_depth: self.max_depth,
            time_limit_ms: self.time_limit_ms,
            node_limit: self.node_limit,
            info_callback: self.info_callback.clone(),
        }
    }

    /// Set max depth
    #[must_use]
    pub fn depth(mut self, max_depth: u32) -> Self {
        self.max_depth = max_depth.min(DEFAULT_MAX_DEPTH);
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

/// Return the number of workers that can receive a non-zero node budget.
fn active_worker_count(requested_threads: usize, node_limit: u64) -> usize {
    let requested_threads = requested_threads.max(1);
    if node_limit == 0 {
        return requested_threads;
    }

    usize::try_from(node_limit).map_or(requested_threads, |nodes| {
        requested_threads.min(nodes.max(1))
    })
}

/// Split a global node budget between workers without exceeding it in total.
fn worker_node_limit(node_limit: u64, worker_id: usize, worker_count: usize) -> u64 {
    if node_limit == 0 {
        return 0;
    }

    let worker_count = u64::try_from(worker_count).unwrap_or(u64::MAX).max(1);
    let base = node_limit / worker_count;
    let remainder = node_limit % worker_count;
    let receives_remainder = u64::try_from(worker_id).is_ok_and(|id| id < remainder);

    base + u64::from(receives_remainder)
}

/// Search thread stack size (32 MB to handle deep recursion)
const SEARCH_STACK_SIZE: usize = 32 * 1024 * 1024;

fn best_worker_move(results: &[WorkerResult]) -> Option<crate::board::Move> {
    let main_result = results
        .iter()
        .find(|r| r.worker_id == 0 && r.best_move.is_some());
    let best_result = main_result.or_else(|| {
        results
            .iter()
            .filter(|r| r.best_move.is_some())
            .max_by_key(|r| r.depth)
    });

    best_result.and_then(|r| r.best_move)
}

fn extract_smp_ponder_move(
    board: &Board,
    shared: &SharedSearchState,
    best_move: Option<crate::board::Move>,
) -> Option<crate::board::Move> {
    best_move.and_then(|mv| {
        let mut temp_board = board.clone();
        let info = temp_board.make_move(mv);
        let legal_moves = temp_board.generate_moves();
        let ponder = shared
            .tt
            .probe(temp_board.hash)
            .and_then(|entry| entry.best_move())
            .filter(|pmv| legal_moves.iter().any(|legal| legal == pmv));
        temp_board.unmake_move(mv, info);
        ponder
    })
}

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
    let num_threads = active_worker_count(config.num_threads, config.node_limit);

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

    // Avoid spawning worker threads for terminal, proven-draw, and exact
    // K+NN-versus-K roots. The single-thread path performs this same check
    // internally; SMP needs it here to avoid thread setup entirely.
    let mut root_board = board.clone();
    if let RootSearchResolution::Resolved(best_move) = immediate_root_result(&mut root_board) {
        state.generation = state.generation.wrapping_add(1);
        state.stats.reset_search();
        return SearchResult {
            best_move,
            ponder_move: None,
        };
    }

    // Increment generation for new search
    state.generation = state.generation.wrapping_add(1);
    state.stats.reset_search();

    // Create shared state with the TT, pawn hash, and NNUE from SearchState
    let shared = Arc::new(SharedSearchState::new(
        state,
        Arc::clone(&stop),
        state.generation,
    ));

    let worker_config = config.to_worker_config();

    // Spawn worker threads
    let mut handles: Vec<JoinHandle<WorkerResult>> = Vec::with_capacity(num_threads);

    for worker_id in 0..num_threads {
        let board_clone = board.clone();
        let shared_clone = Arc::clone(&shared);
        let mut worker_cfg = worker_config.clone();
        worker_cfg.node_limit = worker_node_limit(config.node_limit, worker_id, num_threads);
        // Only main worker reports info
        if worker_id != 0 {
            worker_cfg.info_callback = None;
        }

        let handle = thread::Builder::new()
            .name(format!("search-{worker_id}"))
            .stack_size(SEARCH_STACK_SIZE)
            .spawn(move || run_worker(worker_id, board_clone, shared_clone, worker_cfg))
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
    state.stats.total_nodes = state.stats.nodes;
    state.stats.seldepth = shared.max_seldepth.load(Ordering::Relaxed) as u32;

    let best_move = best_worker_move(&results);
    let ponder_move = extract_smp_ponder_move(board, &shared, best_move);

    SearchResult {
        best_move,
        ponder_move,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Instant;

    use crate::board::{Board, SearchState};

    use super::{active_worker_count, smp_search, worker_node_limit, SmpConfig, DEFAULT_MAX_DEPTH};

    #[test]
    fn depth_clamps_to_default_max_depth() {
        assert_eq!(
            SmpConfig::with_threads(2).depth(u32::MAX).max_depth,
            DEFAULT_MAX_DEPTH
        );
    }

    #[test]
    fn node_limited_search_uses_no_more_workers_than_nodes() {
        assert_eq!(active_worker_count(4, 0), 4);
        assert_eq!(active_worker_count(4, 1), 1);
        assert_eq!(active_worker_count(4, 3), 3);
        assert_eq!(active_worker_count(4, 10), 4);
    }

    #[test]
    fn worker_node_limits_partition_the_global_budget() {
        let limits: Vec<_> = (0..3)
            .map(|worker| worker_node_limit(10, worker, 3))
            .collect();

        assert_eq!(limits, vec![4, 3, 3]);
        assert_eq!(limits.iter().sum::<u64>(), 10);
        assert_eq!(worker_node_limit(0, 0, 3), 0);
    }

    #[test]
    fn smp_search_respects_the_global_node_limit() {
        let board = Board::default();
        let mut state = SearchState::new(1);
        let config = SmpConfig::with_threads(2).depth(6).nodes(100);

        let _ = smp_search(&board, &mut state, config, Arc::new(AtomicBool::new(false)));

        assert!(
            state.stats.nodes <= 100,
            "searched {} nodes with a 100-node budget",
            state.stats.nodes
        );
        assert_eq!(state.stats.total_nodes, state.stats.nodes);
    }

    #[test]
    fn smp_search_with_single_node_limit_never_overshoots() {
        let mut board = Board::default();
        let mut state = SearchState::new(1);
        let config = SmpConfig::with_threads(4).depth(20).nodes(1);

        let result = smp_search(&board, &mut state, config, Arc::new(AtomicBool::new(false)));

        assert!(
            result.best_move.is_some_and(|mv| board.is_legal_move(mv)),
            "an immediately limited SMP search should retain a legal root fallback"
        );
        assert!(
            state.stats.nodes <= 1,
            "SMP search exceeded its one-node budget: {}",
            state.stats.nodes
        );
        assert_eq!(state.stats.total_nodes, state.stats.nodes);
    }

    #[test]
    fn smp_search_respects_state_hard_deadline() {
        let board = Board::default();
        let mut state = SearchState::new(1);
        state.set_hard_stop_at(Some(Instant::now()));
        let config = SmpConfig::with_threads(2).depth(6);

        let result = smp_search(&board, &mut state, config, Arc::new(AtomicBool::new(false)));

        assert!(result.best_move.is_some());
        assert_eq!(state.stats.nodes, 0);
    }

    #[test]
    fn smp_search_resolves_two_knights_vs_bare_king_at_the_root() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/4N1N1/K7 w - - 0 1");
        let mut state = SearchState::new(1);
        let config = SmpConfig::with_threads(2).depth(12);

        let result = smp_search(&board, &mut state, config, Arc::new(AtomicBool::new(false)));

        assert!(result.best_move.is_some());
        assert!(board.is_legal_move(result.best_move.unwrap()));
        assert_eq!(state.stats.nodes, 0);
    }

    #[test]
    fn smp_search_keeps_two_knights_mate_in_one_at_the_root() {
        let board = Board::from_fen("8/8/8/8/8/2N5/8/k1K1N3 w - - 0 1");
        let mut state = SearchState::new(1);
        let config = SmpConfig::with_threads(2).depth(12);

        let result = smp_search(&board, &mut state, config, Arc::new(AtomicBool::new(false)));

        assert_eq!(
            result.best_move.map(|mv| mv.to_string()),
            Some("e1c2".to_string())
        );
        assert_eq!(state.stats.nodes, 0);
    }
}
