use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::{format_square, Move, MoveList};
use crate::core::board::Board;
use crate::uci::info as uci_info;
use crate::search::control as search_control;
use crate::core::config::search::*;
use crate::core::config::evaluation::*;
use crate::evaluation::pawn_hash::PawnHashTable;
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::time::{Duration, Instant};

pub enum SearchLimits {
    Depth(u32),
    Time(Duration),
    Infinite,
}

pub struct SearchConfig<'a> {
    limits: SearchLimits,
    board: &'a mut Board,
    tt: &'a mut TranspositionTable,
    sink: Option<Arc<Mutex<Option<Move>>>>,
    info_sender: Option<Sender<uci_info::Info>>,
    is_ponder: bool,
    start_time: Instant,
}

/// Iterative deepening search driver which publishes `uci_info::Info` messages to `info_sender`
/// and updates an optional sink with intermediate best moves.
pub fn iterative_deepening_with_sink(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_depth: u32,
    sink: Option<Arc<Mutex<Option<Move>>>>,
    info_sender: Option<Sender<uci_info::Info>>,
    is_ponder: bool,
) -> Option<Move> {
    let config = SearchConfig {
        limits: SearchLimits::Depth(max_depth),
        board,
        tt,
        sink,
        info_sender,
        is_ponder,
        start_time: Instant::now(),
    };
    let mut dummy_pawn_hash_table = crate::evaluation::pawn_hash::PawnHashTable::new();
    search_internal(config, &mut dummy_pawn_hash_table)
}

/// Time-limited iterative deepening driver.
pub fn time_limited_search_with_sink(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
    sink: Option<Arc<Mutex<Option<Move>>>>,
    info_sender: Option<Sender<uci_info::Info>>,
    is_ponder: bool,
) -> Option<Move> {
    let config = SearchConfig {
        limits: SearchLimits::Time(max_time),
        board,
        tt,
        sink,
        info_sender,
        is_ponder,
        start_time,
    };
    let mut dummy_pawn_hash_table = crate::evaluation::pawn_hash::PawnHashTable::new();
    search_internal(config, &mut dummy_pawn_hash_table)
}

fn search_internal(config: SearchConfig, pawn_hash_table: &mut PawnHashTable) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut prev_score: Option<i32> = None;
    let mut last_depth_time = Duration::from_millis(1);

    let mut root_moves: MoveList = MoveList::new();
    config.board.generate_moves_into(&mut root_moves);
    if root_moves.is_empty() {
        return None;
    }
    if root_moves.len() == 1 {
        return Some(root_moves[0]);
    }

    // Create the initial SearchContext
    let mut ordering_ctx = crate::movegen::ordering::OrderingContext::new(255); // Max depth is 255
    let mut s_ctx = crate::search::search_context::SearchContext {
        tt: config.tt,
        moves_buf: &mut MoveList::new(),
        ordering_ctx: &mut ordering_ctx,
        ply: 0,
        pawn_hash_table,
    };

    for depth in 1..=255 {
        if should_stop(&config.limits, depth, &config.start_time, last_depth_time) {
            break;
        }

        s_ctx.tt.new_generation();
        let depth_start = Instant::now();

        let mut mv_opt: Option<Move> = None;
        let mut score: i32 = 0;
        let mut completed = false;

        // Reset the ply for the root search for each iteration
        s_ctx.ply = 0;

        if let Some(ps) = prev_score {
            if depth > 2 && ps.abs() < (MATE_SCORE / 2) {
                let mut alpha = ps - 10;
                let mut beta = ps + 10;
                let mut margin = 10;

                loop {
                    if search_control::should_stop() {
                        break;
                    }

                    let stop_check = || {
                        should_stop(&config.limits, depth, &config.start_time, last_depth_time)
                            || search_control::should_stop()
                    };
                    let (mv_try, sc_try, completed_try) = crate::search::run_root_search(
                        config.board,
                        &mut s_ctx,
                        depth,
                        &mut root_moves[..],
                        stop_check,
                        Some((alpha, beta)),
                    );

                    if !completed_try {
                        break; // Aborted, fall back to full window
                    }

                    if sc_try <= alpha {
                        alpha = ps - margin;
                    } else if sc_try >= beta {
                        beta = ps + margin;
                    } else {
                        mv_opt = mv_try;
                        score = sc_try;
                        completed = true;
                        break;
                    }

                    margin = margin.saturating_mul(2);
                    if margin > 500 {
                        break; // Give up, fall back to full window
                    }
                }
            }
        }

        if !completed && !search_control::should_stop() {
            let stop_check = || {
                should_stop(&config.limits, depth, &config.start_time, last_depth_time)
                    || search_control::should_stop()
            };
            let (mv_full, sc_full, completed_full) = crate::search::run_root_search(
                config.board,
                &mut s_ctx,
                depth,
                &mut root_moves[..],
                stop_check,
                None,
            );
            mv_opt = mv_full;
            score = sc_full;
            completed = completed_full;
        }

        if completed {
            if let Some(mv) = mv_opt {
                best_move = Some(mv);
                prev_score = Some(score);

                if let Some(ref s) = config.sink {
                    let mut lock = match s.lock() {
                        Ok(g) => g,
                        Err(poisoned) => {
                            eprintln!("warning: sink mutex poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    *lock = best_move;
                }

                if let Some(ref sender) = config.info_sender {
                    send_uci_info(
                        sender,
                        depth,
                        score,
                        best_move,
                        s_ctx.tt,
                        config.board.hash,
                        &config.start_time,
                        config.is_ponder,
                    );
                }

                if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                    root_moves.swap(0, pos);
                }
            }
        }
        last_depth_time = depth_start.elapsed();
    }

    best_move
}

fn should_stop(limits: &SearchLimits, depth: u32, start_time: &Instant, last_depth_time: Duration) -> bool {
    match limits {
        SearchLimits::Depth(max_depth) => depth > *max_depth,
        SearchLimits::Time(max_time) => {
            let elapsed = start_time.elapsed();
            if elapsed + SAFETY_MARGIN >= *max_time {
                return true;
            }
            let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();
            let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
            estimated_next_time + SAFETY_MARGIN > time_remaining
        }
        SearchLimits::Infinite => false,
    }
}

fn send_uci_info(
    sender: &Sender<uci_info::Info>,
    depth: u32,
    score: i32,
    best_move: Option<Move>,
    tt: &TranspositionTable,
    board_hash: u64,
    start_time: &Instant,
    is_ponder: bool,
) {
    let nodes_total = search_control::get_node_count();
    let elapsed_ms = start_time.elapsed().as_millis();
    let nps = if elapsed_ms > 0 {
        Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
    } else {
        None
    };
    let pv = crate::search::build_pv_from_tt(tt, board_hash);
    let mut info = uci_info::Info {
        depth: Some(depth),
        nodes: Some(nodes_total),
        nps,
        time_ms: Some(elapsed_ms),
        score_cp: None,
        score_mate: None,
        pv: Some(pv),
        seldepth: Some(depth),
        ponder: None,
    };
    if score.abs() > (MATE_SCORE / 2) {
        let mate_in = (MATE_SCORE - score.abs() + 1) / 2;
        info.score_mate = Some(mate_in);
    } else {
        info.score_cp = Some(score);
    }
    if is_ponder {
        if let Some(bm) = best_move {
            info.ponder = Some(format!("{}{}", format_square(bm.from), format_square(bm.to)));
        }
    }
    let _ = sender.send(info);
}


