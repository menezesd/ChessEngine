use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::board::nnue::NnueNetwork;
use crate::board::{Board, Move, SearchIterationInfo};
use crate::tt::TranspositionTable;

use super::super::simple::simple_search_multipv;
use super::super::{
    HceOptions, SearchConfig, SearchParams, SearchState, StaticEvalOptions, DEFAULT_MAX_DEPTH,
};

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
        self.max_seldepth
            .fetch_max(u64::from(seldepth), Ordering::Relaxed);
    }

    /// Add nodes to global counter.
    pub fn add_nodes(&self, nodes: u64) {
        let mut current = self.total_nodes.load(Ordering::Relaxed);
        while let Err(observed) = self.total_nodes.compare_exchange_weak(
            current,
            current.saturating_add(nodes),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            current = observed;
        }
    }
}

/// Result from a single worker thread.
#[derive(Debug, Clone)]
pub struct WorkerResult {
    pub worker_id: usize,
    /// The best playable move, including a verified improvement after the
    /// last completed iteration.
    pub best_move: Option<Move>,
    /// Score of the last completed iteration, or zero for an unsearched fallback.
    pub score: i32,
    /// Last completed depth, or zero if no iteration finished.
    pub depth: u32,
    pub nodes: u64,
}

pub(super) struct WorkerOutcome {
    pub(super) result: WorkerResult,
    pub(super) last_iteration: Option<SearchIterationInfo>,
    pub(super) tt_hits: u64,
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
    mut config: SearchConfig,
    #[cfg(feature = "syzygy")] syzygy: Option<Arc<super::super::syzygy::Tablebases>>,
) -> WorkerOutcome {
    let mut local_state = new_worker_state(&shared);
    #[cfg(feature = "syzygy")]
    {
        local_state.syzygy = syzygy;
    }

    let search_depth =
        worker_search_depth(config.max_depth.unwrap_or(DEFAULT_MAX_DEPTH), worker_id);
    let completed = Arc::new(Mutex::new(None));
    let last_iteration = Arc::clone(&completed);
    let callback = config.info_callback.take();
    let record_iteration = Arc::new(move |info: &crate::board::SearchIterationInfo| {
        *last_iteration.lock() = Some(info.clone());
        if let Some(callback) = &callback {
            callback(info);
        }
    });

    config.max_depth = Some(search_depth);
    config.info_callback = Some(record_iteration);
    let move_result = simple_search_multipv(
        &mut board,
        &mut local_state,
        &config,
        &shared.stop,
        worker_id,
    );

    shared.add_nodes(local_state.stats.nodes);
    shared.update_seldepth(local_state.stats.seldepth);

    // Another worker can replace the root TT entry at any time. Its score
    // and requested depth do not describe this worker's selected move.
    // Partial recovery is disabled after a completed mate score, so worker
    // selection can use that score to compare forced mates.
    let last_iteration = completed.lock().take();
    let (depth, score) = last_iteration
        .as_ref()
        .map_or((0, 0), |info| (info.depth, info.score));

    WorkerOutcome {
        result: WorkerResult {
            worker_id,
            best_move: move_result,
            score,
            depth,
            nodes: local_state.stats.total_nodes,
        },
        last_iteration,
        tt_hits: local_state.stats.tt_hits,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::board::SearchState;

    use super::{
        new_worker_state, run_worker, worker_search_depth, SearchConfig, SharedSearchState,
    };

    #[test]
    fn interrupted_worker_reports_only_completed_search_metadata() {
        let board = crate::board::Board::new();
        let state = SearchState::new(1);
        state.tables.tt.store(
            board.hash(),
            8,
            1234,
            crate::tt::BoundType::LowerBound,
            None,
            0,
        );
        let shared = Arc::new(SharedSearchState::new(
            &state,
            Arc::new(AtomicBool::new(true)),
            0,
        ));
        let config = SearchConfig::depth(20);

        let result = run_worker(
            1,
            board,
            shared,
            config,
            #[cfg(feature = "syzygy")]
            None,
        )
        .result;

        assert!(result.best_move.is_some());
        assert_eq!(result.nodes, 0);
        assert_eq!(result.depth, 0);
        assert_eq!(result.score, 0);
    }

    #[test]
    fn worker_result_matches_its_last_completed_iteration() {
        let board = crate::board::Board::new();
        let state = SearchState::new(1);
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(SharedSearchState::new(&state, Arc::clone(&stop), 0));
        let reported = Arc::new(parking_lot::Mutex::new(None));
        let received = Arc::clone(&reported);
        let config = SearchConfig {
            info_callback: Some(Arc::new(move |info| {
                *received.lock() = Some(info.clone());
                if info.depth == 2 {
                    stop.store(true, Ordering::Relaxed);
                }
            })),
            ..SearchConfig::depth(20)
        };

        let result = run_worker(
            0,
            board,
            shared,
            config,
            #[cfg(feature = "syzygy")]
            None,
        )
        .result;

        let reported = reported.lock();
        let reported = reported.as_ref().unwrap();
        assert_eq!(result.depth, 2);
        assert_eq!(result.score, reported.score);
        assert_eq!(
            result.best_move.unwrap().to_string(),
            reported.pv.split_whitespace().next().unwrap()
        );
    }

    #[test]
    fn worker_search_depth_alternates_helper_depth() {
        assert_eq!(worker_search_depth(6, 0), 6);
        assert_eq!(worker_search_depth(6, 1), 7);
        assert_eq!(worker_search_depth(6, 2), 6);
        assert_eq!(worker_search_depth(6, 3), 7);
    }

    #[test]
    fn helper_can_start_at_a_different_depth_from_the_main_worker() {
        let board = crate::board::Board::new();
        let state = SearchState::new(1);
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(SharedSearchState::new(&state, Arc::clone(&stop), 0));
        let callback_stop = Arc::clone(&stop);
        let config = SearchConfig {
            info_callback: Some(Arc::new(move |_| {
                callback_stop.store(true, Ordering::Relaxed);
            })),
            ..SearchConfig::depth(3)
        };

        let result = run_worker(
            1,
            board,
            shared,
            config,
            #[cfg(feature = "syzygy")]
            None,
        )
        .result;
        assert_eq!(result.depth, 2);
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
