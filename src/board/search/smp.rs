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
    constants::DEFAULT_MAX_DEPTH, SearchConfig, SearchInfoCallback, SearchResult, SearchState,
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
        state.shared_static_nnue(),
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
    use super::{SmpConfig, DEFAULT_MAX_DEPTH};

    #[test]
    fn depth_clamps_to_default_max_depth() {
        assert_eq!(
            SmpConfig::with_threads(2).depth(u32::MAX).max_depth,
            DEFAULT_MAX_DEPTH
        );
    }
}
