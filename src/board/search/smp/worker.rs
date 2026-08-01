use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::board::nnue::NnueNetwork;
use crate::board::{Board, Move};
use crate::tt::TranspositionTable;

use super::super::constants::SCORE_INFINITE;
use super::super::simple::simple_search;
use super::super::{HceOptions, SearchInfoCallback, SearchParams, SearchState, StaticEvalOptions};

/// Shared state across all worker threads.
pub struct SharedSearchState {
    /// Thread-safe transposition table
    pub tt: Arc<TranspositionTable>,
    /// Thread-safe pawn hash table
    pub pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
    /// Shared NNUE network (optional)
    pub nnue: Option<Arc<NnueNetwork>>,
    /// Shared static-eval NNUE network (optional)
    pub static_nnue: Option<Arc<NnueNetwork>>,
    /// Stop flag checked by all workers
    pub stop: Arc<AtomicBool>,
    /// Global node counter (sum of all workers)
    pub total_nodes: Arc<AtomicU64>,
    /// Maximum selective depth seen
    pub max_seldepth: Arc<AtomicU64>,
    /// TT generation for aging
    pub generation: u16,
    /// Explicit hard deadline inherited from the parent search state.
    pub hard_stop_at: Option<Instant>,
    /// Search parameters
    pub params: SearchParams,
    /// Main NNUE scale and HCE blend configuration.
    pub nnue_eval_scale: i32,
    pub nnue_hce_blend: i32,
    /// HCE configuration for full and static evaluation.
    pub hce_options: HceOptions,
    pub static_eval_options: StaticEvalOptions,
    /// Static-evaluation NNUE configuration.
    pub nnue_static_eval_scale: i32,
    pub nnue_static_blend: i32,
    /// Whether static-evaluation tracing is enabled.
    pub trace: bool,
}

impl SharedSearchState {
    /// Create with a specific TT, pawn hash table, and optional NNUE network.
    pub fn new(state: &SearchState, stop: Arc<AtomicBool>, generation: u16) -> Self {
        SharedSearchState {
            tt: state.shared_tt(),
            pawn_hash: state.shared_pawn_hash(),
            nnue: state.shared_nnue(),
            static_nnue: state.shared_static_nnue(),
            stop,
            total_nodes: Arc::new(AtomicU64::new(0)),
            max_seldepth: Arc::new(AtomicU64::new(0)),
            generation,
            hard_stop_at: state.hard_stop_at,
            params: state.params.clone(),
            nnue_eval_scale: state.nnue_eval_scale,
            nnue_hce_blend: state.nnue_hce_blend,
            hce_options: state.hce_options,
            static_eval_options: state.static_eval_options,
            nnue_static_eval_scale: state.nnue_static_eval_scale,
            nnue_static_blend: state.nnue_static_blend,
            trace: state.trace,
        }
    }

    /// Update seldepth if this value is higher.
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

    /// Add nodes to global counter.
    pub fn add_nodes(&self, nodes: u64) {
        let mut current = self.total_nodes.load(Ordering::Relaxed);
        loop {
            let next = current.saturating_add(nodes);
            match self.total_nodes.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }
}

/// Result from a single worker thread.
#[derive(Debug, Clone)]
pub struct WorkerResult {
    pub worker_id: usize,
    pub best_move: Option<Move>,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
}

/// Configuration passed to each worker thread.
#[derive(Clone)]
pub(super) struct WorkerSearchConfig {
    pub(super) max_depth: u32,
    pub(super) time_limit_ms: u64,
    pub(super) node_limit: u64,
    pub(super) info_callback: Option<SearchInfoCallback>,
}

/// Get depth offset for a worker thread.
///
/// Thread 0 (main): searches at target depth.
/// Thread 1: searches at depth + 1 (populates TT with deeper entries).
/// Thread 2: searches at depth (different move order due to separate tables).
/// Thread 3: searches at depth + 1.
fn worker_depth_offset(worker_id: usize) -> u32 {
    u32::from(!worker_id.is_multiple_of(2))
}

fn worker_search_depth(max_depth: u32, worker_id: usize) -> u32 {
    max_depth
        .max(1)
        .saturating_add(worker_depth_offset(worker_id))
}

fn new_worker_state(shared: &SharedSearchState) -> SearchState {
    let mut local_state = SearchState::with_shared_tables(
        Arc::clone(&shared.tt),
        Arc::clone(&shared.pawn_hash),
        shared.nnue.clone(),
        shared.static_nnue.clone(),
        shared.generation,
    );
    local_state.params = shared.params.clone();
    local_state.hard_stop_at = shared.hard_stop_at;
    local_state.nnue_eval_scale = shared.nnue_eval_scale;
    local_state.nnue_hce_blend = shared.nnue_hce_blend;
    local_state.hce_options = shared.hce_options;
    local_state.static_eval_options = shared.static_eval_options;
    local_state.nnue_static_eval_scale = shared.nnue_static_eval_scale;
    local_state.nnue_static_blend = shared.nnue_static_blend;
    local_state.trace = shared.trace;
    local_state
}

/// Run a single worker thread.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn run_worker(
    worker_id: usize,
    mut board: Board,
    shared: Arc<SharedSearchState>,
    config: WorkerSearchConfig,
) -> WorkerResult {
    let mut local_state = new_worker_state(&shared);

    local_state.tables.history.decay();
    local_state.tables.killer_moves.reset();
    local_state.tables.counter_moves.reset();

    let search_depth = worker_search_depth(config.max_depth, worker_id);

    let move_result = simple_search(
        &mut board,
        &mut local_state,
        search_depth,
        config.time_limit_ms,
        config.node_limit,
        &shared.stop,
        config.info_callback,
    );

    shared.add_nodes(local_state.stats.nodes);
    shared.update_seldepth(local_state.stats.seldepth);

    let best_move = move_result;
    let best_score = if let Some(entry) = shared.tt.probe(board.hash) {
        entry.score()
    } else {
        -SCORE_INFINITE
    };

    WorkerResult {
        worker_id,
        best_move,
        score: best_score,
        depth: search_depth,
        nodes: local_state.stats.total_nodes,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::board::SearchState;

    use super::{new_worker_state, worker_search_depth, SharedSearchState};

    #[test]
    fn worker_search_depth_alternates_helper_depth() {
        assert_eq!(worker_search_depth(6, 0), 6);
        assert_eq!(worker_search_depth(6, 1), 7);
        assert_eq!(worker_search_depth(6, 2), 6);
        assert_eq!(worker_search_depth(6, 3), 7);
    }

    #[test]
    fn worker_search_depth_clamps_zero_and_saturates_max() {
        assert_eq!(worker_search_depth(0, 0), 1);
        assert_eq!(worker_search_depth(0, 1), 2);
        assert_eq!(worker_search_depth(u32::MAX, 1), u32::MAX);
    }

    #[test]
    fn add_nodes_saturates_global_counter() {
        let state = SearchState::new(1);
        let shared = SharedSearchState::new(&state, Arc::new(AtomicBool::new(false)), 0);
        shared.total_nodes.store(u64::MAX - 1, Ordering::Relaxed);

        shared.add_nodes(10);

        assert_eq!(shared.total_nodes.load(Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn worker_state_inherits_search_and_evaluation_options() {
        let mut state = SearchState::new(1);
        let hard_stop_at = Instant::now() + Duration::from_secs(1);
        state.set_hard_stop_at(Some(hard_stop_at));
        state.params.futility_margin = 321;
        state.nnue_eval_scale = 123;
        state.nnue_hce_blend = 45;
        state.hce_options.use_full = false;
        state.hce_options.use_tuned = false;
        state.static_eval_options.nnue_pure = true;
        state.static_eval_options.use_full_hce = true;
        state.nnue_static_eval_scale = 87;
        state.nnue_static_blend = 65;
        state.trace = true;

        let shared = SharedSearchState::new(&state, Arc::new(AtomicBool::new(false)), 7);
        let worker_state = new_worker_state(&shared);

        assert_eq!(worker_state.generation, 7);
        assert_eq!(worker_state.hard_stop_at, Some(hard_stop_at));
        assert_eq!(worker_state.params.futility_margin, 321);
        assert_eq!(worker_state.nnue_eval_scale, 123);
        assert_eq!(worker_state.nnue_hce_blend, 45);
        assert!(!worker_state.hce_options.use_full);
        assert!(!worker_state.hce_options.use_tuned);
        assert!(worker_state.static_eval_options.nnue_pure);
        assert!(worker_state.static_eval_options.use_full_hce);
        assert_eq!(worker_state.nnue_static_eval_scale, 87);
        assert_eq!(worker_state.nnue_static_blend, 65);
        assert!(worker_state.trace);
    }
}
