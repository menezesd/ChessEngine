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
mod endgame;
mod move_order;
mod params;
mod simple;
pub mod smp;
mod state;
#[cfg(feature = "syzygy")]
mod syzygy;
mod tables;

use std::sync::atomic::AtomicBool;

pub use config::{
    SearchClock, SearchConfig, SearchInfoCallback, SearchIterationInfo, SearchLimits, SearchResult,
};
pub(crate) use constants::{DEFAULT_MAX_DEPTH, SEARCH_THREAD_STACK_SIZE};
pub use params::SearchParams;
pub use state::{HceOptions, SearchState, SearchStats, StaticEvalOptions};
pub use tables::{CaptureHistory, CounterMoveTable, HistoryTable, KillerTable, SearchTables};

use super::{Board, Move, MAX_PLY};

/// Default transposition table size in MB
pub const DEFAULT_TT_MB: usize = 1024;

/// Mate score constant
pub(crate) const MATE_SCORE: i32 = constants::MATE_THRESHOLD + MAX_PLY as i32;

/// Extract the predicted reply from an endgame table or the TT.
fn extract_ponder_move(
    board: &mut Board,
    tt: &crate::tt::TranspositionTable,
    best_move: Move,
) -> Option<Move> {
    let info = board.make_move(best_move);

    let ponder = endgame::best_reply(board).or_else(|| {
        tt.probe(board.hash)
            .and_then(|entry| entry.best_move())
            .filter(|mv| board.is_legal_move(*mv))
    });

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
    state.generation = state.generation.wrapping_add(1);
    let best_move = simple::simple_search_multipv(board, state, &config, stop, 0);
    let ponder_move = if config.extract_ponder {
        best_move.and_then(|mv| extract_ponder_move(board, &state.tables.tt, mv))
    } else {
        None
    };

    SearchResult {
        best_move,
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
    state.generation = state.generation.wrapping_add(1);
    simple::simple_search_multipv(board, state, &SearchConfig::depth(max_depth), stop, 0)
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
