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
use std::time::Instant;

use crate::board::{draw::FIFTY_MOVE_RULE_HALFMOVES, Board};

use super::{
    constants::{DEFAULT_MAX_DEPTH, MATE_THRESHOLD},
    SearchConfig, SearchInfoCallback, SearchResult, SearchState, SEARCH_THREAD_STACK_SIZE,
};
use worker::{run_worker, WorkerOutcome};
pub use worker::{SharedSearchState, WorkerResult};

/// Configuration for SMP search
#[derive(Clone)]
pub struct SmpConfig {
    /// Number of worker threads
    pub num_threads: usize,
    /// Maximum depth to search
    pub max_depth: u32,
    /// Soft time limit in milliseconds (0 = unlimited)
    pub time_limit_ms: u64,
    /// Hard time limit in milliseconds (0 = none)
    pub hard_time_limit_ms: u64,
    /// Optional live clock shared with the controller (see `SearchConfig`)
    pub clock: Option<Arc<super::SearchClock>>,
    /// Node limit (0 = unlimited)
    pub node_limit: u64,
    /// Optional callback for iteration info
    pub info_callback: Option<SearchInfoCallback>,
    /// Whether this is a ponder search. A ponder search must leave the
    /// shared stop flag untouched on completion: the controller uses that
    /// flag to hold the result until `ponderhit` or `stop` arrives.
    pub ponder: bool,
    /// An infinite search also leaves publication to the controller until
    /// the caller explicitly stops it.
    pub infinite: bool,
}

impl Default for SmpConfig {
    fn default() -> Self {
        SmpConfig {
            num_threads: 1,
            max_depth: DEFAULT_MAX_DEPTH,
            time_limit_ms: 0,
            hard_time_limit_ms: 0,
            clock: None,
            node_limit: 0,
            info_callback: None,
            ponder: false,
            infinite: false,
        }
    }
}

impl SmpConfig {
    fn to_search_config(&self) -> SearchConfig {
        SearchConfig {
            max_depth: Some(self.max_depth),
            time_limit_ms: self.time_limit_ms,
            hard_time_limit_ms: self.hard_time_limit_ms,
            clock: self.clock.clone(),
            node_limit: self.node_limit,
            extract_ponder: true,
            info_callback: self.info_callback.clone(),
            multi_pv: 1,
            root_moves: None,
        }
    }

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

/// Each worker searches its own tree. Small node limits work better when a
/// single worker can finish a useful iteration.
const MIN_SMP_NODES_PER_WORKER: u64 = 8_000;

fn active_worker_count(requested_threads: usize, node_limit: u64) -> usize {
    let requested_threads = requested_threads.max(1);
    if node_limit == 0 {
        return requested_threads;
    }

    let affordable = (node_limit / MIN_SMP_NODES_PER_WORKER).max(1);
    requested_threads.min(usize::try_from(affordable).unwrap_or(usize::MAX))
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

fn best_worker(results: &[WorkerOutcome]) -> Option<&WorkerOutcome> {
    // A completed forced win is stronger than a centipawn score. Keep the
    // shortest available mate even if another worker found it first.
    let forced_mate = results
        .iter()
        .filter(|outcome| {
            let r = &outcome.result;
            r.best_move.is_some() && r.depth > 0 && r.score >= MATE_THRESHOLD
        })
        .max_by_key(|outcome| (outcome.result.score, outcome.result.worker_id == 0));
    forced_mate.or_else(|| {
        results
            .iter()
            .filter(|outcome| outcome.result.best_move.is_some())
            .max_by_key(|outcome| {
                let r = &outcome.result;
                (r.worker_id == 0 && r.depth > 0, r.depth, r.worker_id == 0)
            })
    })
}

fn extract_smp_ponder_move(
    board: &Board,
    shared: &SharedSearchState,
    best_move: Option<crate::board::Move>,
) -> Option<crate::board::Move> {
    best_move.and_then(|mv| {
        let mut temp_board = board.clone();
        super::extract_ponder_move(&mut temp_board, &shared.tt, mv)
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
    let mut root_board = board.clone();

    // Tablebase roots need a single ranking pass.
    if num_threads == 1 || super::endgame::probe(board).is_some() {
        return super::search(&mut root_board, state, config.to_search_config(), &stop);
    }

    // These roots can be handled by the single-thread path without starting
    // helpers or generating a second root move list for ordinary positions.
    if board.halfmove_clock() >= FIFTY_MOVE_RULE_HALFMOVES - 1
        || board.is_theoretical_draw()
        || !board.has_multiple_legal_moves()
    {
        state.stats.reset_search();
        let mut root_config = config.to_search_config();
        root_config.extract_ponder = root_config.info_callback.is_some();
        return super::search(&mut root_board, state, root_config, &stop);
    }

    #[cfg(feature = "syzygy")]
    if let Some(result) = try_syzygy(&mut root_board, state, &config, &stop) {
        return result;
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

    let start = Instant::now();
    let handles = spawn_workers(
        board,
        &shared,
        &config,
        num_threads,
        #[cfg(feature = "syzygy")]
        state.syzygy.as_ref(),
    );

    if handles.is_empty() {
        state.generation = state.generation.wrapping_sub(1);
        return super::search(&mut root_board, state, config.to_search_config(), &stop);
    }

    // Join the main worker first: it defines search completion. Helpers
    // search one ply deeper, so without a completion signal a fixed-depth
    // search would block here until the slowest helper finishes its deeper
    // tree. The stop flag is created fresh for each search, so setting it
    // cannot leak into a later search. Ponder and infinite searches must
    // leave it untouched: the controller waits for `ponderhit` or `stop`.
    let mut results: Vec<WorkerOutcome> = Vec::with_capacity(handles.len());
    let mut handles = handles.into_iter();
    if let Some(main_handle) = handles.next() {
        if let Ok(result) = main_handle.join() {
            results.push(result);
        }
        if !config.ponder && !config.infinite {
            stop.store(true, Ordering::Relaxed);
        }
    }
    for handle in handles {
        if let Ok(result) = handle.join() {
            results.push(result);
        }
    }

    // Update stats from shared counters
    state.stats.nodes = shared.total_nodes.load(Ordering::Relaxed);
    state.stats.total_nodes = state.stats.nodes;
    state.stats.seldepth = shared.max_seldepth.load(Ordering::Relaxed) as u32;
    state.stats.tt_hits = results
        .iter()
        .fold(0u64, |sum, outcome| sum.saturating_add(outcome.tt_hits));

    let selected = best_worker(&results);
    let best_move = selected.and_then(|outcome| outcome.result.best_move);
    if let (Some(callback), Some(outcome)) = (&config.info_callback, selected) {
        if outcome.result.worker_id != 0 {
            if let Some(mut info) = outcome.last_iteration.clone() {
                info.nodes = state.stats.nodes;
                info.seldepth = state.stats.seldepth;
                info.tt_hits = state.stats.tt_hits;
                info.time_ms = start.elapsed().as_millis() as u64;
                info.nps = info
                    .nodes
                    .saturating_mul(1000)
                    .checked_div(info.time_ms)
                    .unwrap_or(0);
                callback(&info);
            }
        }
    }
    let ponder_move = extract_smp_ponder_move(board, &shared, best_move);

    SearchResult {
        best_move,
        ponder_move,
    }
}

fn spawn_workers(
    board: &Board,
    shared: &Arc<SharedSearchState>,
    config: &SmpConfig,
    num_threads: usize,
    #[cfg(feature = "syzygy")] syzygy: Option<&Arc<super::syzygy::Tablebases>>,
) -> Vec<JoinHandle<WorkerOutcome>> {
    let worker_config = config.to_search_config();

    let mut handles = Vec::new();

    for worker_id in 0..num_threads {
        let board_clone = board.clone();
        let shared_clone = Arc::clone(shared);
        let mut worker_cfg = worker_config.clone();
        worker_cfg.node_limit = worker_node_limit(config.node_limit, worker_id, num_threads);
        // Only main worker reports info
        if worker_id != 0 {
            worker_cfg.info_callback = None;
        }

        #[cfg(feature = "syzygy")]
        let syzygy = syzygy.cloned();
        let handle = thread::Builder::new()
            .name(format!("search-{worker_id}"))
            .stack_size(SEARCH_THREAD_STACK_SIZE)
            .spawn(move || {
                run_worker(
                    worker_id,
                    board_clone,
                    shared_clone,
                    worker_cfg,
                    #[cfg(feature = "syzygy")]
                    syzygy,
                )
            });

        let Ok(handle) = handle else {
            // If the system cannot create another worker, keep the ones
            // already running. Their assigned budgets still fit the limit.
            break;
        };
        handles.push(handle);
    }
    handles
}

#[cfg(feature = "syzygy")]
fn try_syzygy(
    board: &mut Board,
    state: &mut SearchState,
    config: &SmpConfig,
    stop: &AtomicBool,
) -> Option<SearchResult> {
    if !state.syzygy.as_ref()?.may_cover(board) {
        return None;
    }
    let moves = board.generate_moves();
    let best_move = super::simple::try_syzygy_root(
        board,
        state,
        &config.to_search_config(),
        moves.as_slice(),
        stop,
    )?;
    state.generation = state.generation.wrapping_add(1);
    state.stats.reset_search();
    Some(SearchResult {
        best_move: Some(best_move),
        ponder_move: None,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Instant;

    use crate::board::{Board, SearchState};

    use super::{active_worker_count, smp_search, worker_node_limit, SmpConfig, DEFAULT_MAX_DEPTH};

    #[test]
    fn completed_helper_result_takes_precedence_over_an_unsearched_fallback() {
        let mut board = Board::new();
        let fallback = board.parse_move("a2a3").unwrap();
        let searched = board.parse_move("e2e4").unwrap();
        let results = [
            super::WorkerResult {
                worker_id: 0,
                best_move: Some(fallback),
                score: 0,
                depth: 0,
                nodes: 0,
            },
            super::WorkerResult {
                worker_id: 1,
                best_move: Some(searched),
                score: 20,
                depth: 2,
                nodes: 100,
            },
        ]
        .map(|result| super::WorkerOutcome {
            result,
            last_iteration: None,
            tt_hits: 0,
        });

        assert_eq!(
            super::best_worker(&results).unwrap().result.best_move,
            Some(searched)
        );
    }

    #[test]
    fn helper_with_shorter_proven_mate_supplies_the_result() {
        use super::super::MATE_SCORE;
        let mut board = Board::new();
        let moves = ["a2a3", "e2e4", "d2d4"].map(|uci| board.parse_move(uci).unwrap());
        let results = [MATE_SCORE - 7, MATE_SCORE - 5, 1000]
            .into_iter()
            .enumerate()
            .map(|(worker_id, score)| super::WorkerOutcome {
                result: super::WorkerResult {
                    worker_id,
                    best_move: Some(moves[worker_id]),
                    score,
                    depth: 6 + worker_id as u32,
                    nodes: 1_000,
                },
                last_iteration: None,
                tt_hits: 0,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            super::best_worker(&results).unwrap().result.best_move,
            Some(moves[1])
        );
    }

    #[test]
    fn depth_clamps_to_default_max_depth() {
        assert_eq!(
            SmpConfig::with_threads(2).depth(u32::MAX).max_depth,
            DEFAULT_MAX_DEPTH
        );
    }

    #[test]
    fn workers_store_entries_in_the_search_generation() {
        let board = Board::new();
        let mut state = SearchState::new(1);
        state.generation = 62;
        let result = smp_search(
            &board,
            &mut state,
            SmpConfig::with_threads(2).depth(2),
            Arc::new(AtomicBool::new(false)),
        );

        assert!(result.best_move.is_some());
        assert_eq!(state.generation, 63);
        assert_eq!(state.tables.tt.probe(board.hash()).unwrap().generation, 63);
    }

    #[test]
    fn node_limited_search_keeps_enough_nodes_per_worker() {
        assert_eq!(active_worker_count(4, 0), 4);
        assert_eq!(active_worker_count(4, 1), 1);
        assert_eq!(active_worker_count(4, 15_999), 1);
        assert_eq!(active_worker_count(4, 16_000), 2);
        assert_eq!(active_worker_count(4, 24_000), 3);
        assert_eq!(active_worker_count(4, 80_000), 4);
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
