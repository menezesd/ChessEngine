//! Search module implementing alpha-beta with iterative deepening.
//!
//! Features:
//! - Iterative deepening with aspiration windows
//! - Alpha-beta search with null move pruning and LMR
//! - Quiescence search with stand-pat
//! - Move ordering (TT move, killers, MVV-LVA, history)
//! - Transposition table for move ordering and cutoffs
//! - Lazy SMP parallel search support

mod config;
mod constants;
mod move_order;
mod params;
mod simple;
pub mod smp;
mod state;
mod tables;

use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub use config::{
    SearchClock, SearchConfig, SearchInfoCallback, SearchIterationInfo, SearchLimits, SearchResult,
};
pub(crate) use constants::DEFAULT_MAX_DEPTH;
pub use params::SearchParams;
pub use state::{HceOptions, SearchState, SearchStats, StaticEvalOptions};
pub use tables::{CaptureHistory, CounterMoveTable, HistoryTable, KillerTable, SearchTables};

use super::{Board, Move, MAX_PLY};

/// Default transposition table size in MB
pub const DEFAULT_TT_MB: usize = 1024;

/// Mate score constant
pub(crate) const MATE_SCORE: i32 = constants::MATE_THRESHOLD + MAX_PLY as i32;

/// Extract ponder move by making best move and probing TT
fn extract_ponder_move(board: &mut Board, state: &SearchState, best_move: Move) -> Option<Move> {
    let info = board.make_move(best_move);

    let legal_moves = board.generate_moves();
    let ponder = state
        .tables
        .tt
        .probe(board.hash)
        .and_then(|entry| entry.best_move())
        .filter(|mv| legal_moves.iter().any(|legal| legal == mv));

    board.unmake_move(best_move, info);

    ponder
}

/// Unified search function that accepts a configuration.
///
/// This is the preferred API for running searches. It consolidates
/// all the `find_best_move_*` variants into a single function.
///
/// # Example
/// ```ignore
/// let config = SearchConfig::depth(10).with_ponder(true);
/// let result = search(board, state, config, &stop);
/// ```
#[allow(clippy::needless_pass_by_value)] // Config is intentionally consumed
pub fn search(
    board: &mut Board,
    state: &mut SearchState,
    config: SearchConfig,
    stop: &AtomicBool,
) -> SearchResult {
    let max_depth = config.max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
    let info_callback = config.info_callback.clone();
    let multi_pv = config.multi_pv.max(1);

    if multi_pv == 1 {
        let best_move = simple::simple_search(
            board,
            state,
            max_depth,
            config.time_limit_ms,
            config.node_limit,
            stop,
            info_callback,
        );

        let ponder_move = if config.extract_ponder {
            best_move.and_then(|mv| extract_ponder_move(board, state, mv))
        } else {
            None
        };

        return SearchResult {
            best_move,
            ponder_move,
        };
    }

    let mut excluded_moves: Vec<Move> = Vec::new();
    let mut first_best_move: Option<Move> = None;
    let starting_total_nodes = state.stats.total_nodes;
    let search_start = Instant::now();

    for pv_index in 1..=multi_pv {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        let node_limit = if config.node_limit == 0 {
            0
        } else {
            let consumed = state.stats.total_nodes.saturating_sub(starting_total_nodes);
            let remaining = config.node_limit.saturating_sub(consumed);
            if remaining == 0 {
                break;
            }
            remaining
        };
        let time_limit_ms = if config.time_limit_ms == 0 {
            0
        } else {
            let elapsed_ms = u64::try_from(search_start.elapsed().as_millis()).unwrap_or(u64::MAX);
            let remaining = config.time_limit_ms.saturating_sub(elapsed_ms);
            if remaining == 0 {
                break;
            }
            remaining
        };

        let best_move = simple::simple_search_multipv(
            board,
            state,
            max_depth,
            time_limit_ms,
            node_limit,
            stop,
            info_callback.clone(),
            &excluded_moves,
            pv_index,
        );

        if let Some(mv) = best_move {
            if pv_index == 1 {
                first_best_move = Some(mv);
            }
            excluded_moves.push(mv);
        } else {
            break;
        }
    }

    let ponder_move = if config.extract_ponder {
        first_best_move.and_then(|mv| extract_ponder_move(board, state, mv))
    } else {
        None
    };

    SearchResult {
        best_move: first_best_move,
        ponder_move,
    }
}

/// Find best move with fixed depth limit
pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> Option<Move> {
    simple::simple_search(board, state, max_depth, 0, 0, stop, None)
}

/// Find best move with fixed depth limit, returning ponder move too
pub fn find_best_move_with_ponder(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> SearchResult {
    search(board, state, SearchConfig::depth(max_depth), stop)
}

/// Find best move with time control
pub fn find_best_move_with_time(
    board: &mut Board,
    state: &mut SearchState,
    limits: &SearchLimits,
) -> Option<Move> {
    let config = SearchConfig::from_limits(limits).with_ponder(false);
    search(board, state, config, &limits.stop).best_move
}

/// Find best move with time control, returning ponder move too
pub fn find_best_move_with_time_and_ponder(
    board: &mut Board,
    state: &mut SearchState,
    limits: &SearchLimits,
) -> SearchResult {
    let config = SearchConfig::from_limits(limits);
    search(board, state, config, &limits.stop)
}
