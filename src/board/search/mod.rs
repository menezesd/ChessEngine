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
    let best_move = if config.multi_pv <= 1 {
        simple::simple_search_multipv(board, state, &config, stop, &[], 1)
    } else {
        search_multiple_pv(board, state, &config, stop)
    };
    let ponder_move = if config.extract_ponder {
        best_move.and_then(|mv| extract_ponder_move(board, state, mv))
    } else {
        None
    };

    SearchResult {
        best_move,
        ponder_move,
    }
}

fn search_multiple_pv(
    board: &mut Board,
    state: &mut SearchState,
    config: &SearchConfig,
    stop: &AtomicBool,
) -> Option<Move> {
    let eligible_root_moves = board
        .generate_moves()
        .iter()
        .filter(|mv| config.allows_root_move(**mv))
        .count();
    let multi_pv = effective_multi_pv(config, eligible_root_moves);
    let mut excluded_moves: Vec<Move> = Vec::new();
    let mut first_best_move: Option<Move> = None;
    let starting_total_nodes = state.stats.total_nodes;
    let search_start = Instant::now();

    // Split the soft budget evenly across PV lines. Giving each line the
    // full remaining budget would let line 1 consume everything and leave
    // nothing to report for the other lines under a clock.
    let per_line_soft_ms = if config.time_limit_ms == 0 {
        0
    } else {
        (config.time_limit_ms / u64::from(multi_pv)).max(1)
    };

    for pv_index in 1..=multi_pv {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        if pv_index > 1
            && config.clock.as_ref().is_some_and(|clock| {
                let (_, soft, hard) = clock.snapshot();
                let now = Instant::now();
                soft.is_some_and(|deadline| now >= deadline)
                    || hard.is_some_and(|deadline| now >= deadline)
            })
        {
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
        let elapsed_ms = u64::try_from(search_start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let time_limit_ms = if config.time_limit_ms == 0 || config.clock.is_some() {
            config.time_limit_ms
        } else {
            let remaining = config.time_limit_ms.saturating_sub(elapsed_ms);
            if remaining == 0 {
                break;
            }
            remaining.min(per_line_soft_ms)
        };
        let hard_time_limit_ms = if config.hard_time_limit_ms == 0 || config.clock.is_some() {
            config.hard_time_limit_ms
        } else {
            let remaining = config.hard_time_limit_ms.saturating_sub(elapsed_ms);
            if remaining == 0 {
                break;
            }
            remaining
        };

        // Static budgets account for earlier PV lines here. Live clocks
        // remain shared so resets also reach a search already in progress;
        // each context derives its own share of the live soft deadline.
        let line_config = SearchConfig {
            time_limit_ms,
            hard_time_limit_ms,
            node_limit,
            multi_pv,
            ..config.clone()
        };
        let best_move = simple::simple_search_multipv(
            board,
            state,
            &line_config,
            stop,
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

    // Stop or a tiny static budget can expire before PV1 starts. Preserve
    // the same legal fallback that single-PV iterative deepening provides.
    first_best_move.or_else(|| {
        board
            .generate_moves()
            .iter()
            .copied()
            .find(|mv| config.allows_root_move(*mv))
    })
}

fn effective_multi_pv(config: &SearchConfig, eligible_root_moves: usize) -> u32 {
    let eligible = u32::try_from(eligible_root_moves).unwrap_or(u32::MAX);
    config.multi_pv.min(eligible).max(1)
}

/// Find best move with fixed depth limit
pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> Option<Move> {
    simple::simple_search(board, state, max_depth, 0, 0, None, 0, stop, None)
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

#[cfg(test)]
mod tests {
    use super::{effective_multi_pv, Board, SearchConfig};

    #[test]
    fn multipv_count_is_capped_by_eligible_root_moves() {
        let mut board = Board::new();
        let only = board.parse_move("e2e4").unwrap();
        let config = SearchConfig::depth(1)
            .with_multi_pv(256)
            .with_root_moves(vec![only]);
        let eligible = board
            .generate_moves()
            .iter()
            .filter(|mv| config.allows_root_move(**mv))
            .count();

        assert_eq!(eligible, 1);
        assert_eq!(effective_multi_pv(&config, eligible), 1);
    }
}
