use crate::transposition_table::TranspositionTable;
use crate::types::{format_square, Move, MoveList};
use crate::board::Board;
use crate::ordering::{OrderingContext, order_moves};
// Null-move pruning removed.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::sync::mpsc::Sender;
use crate::uci_info;
use crate::transposition_table::BoundType;
use crate::see::see_capture;
use std::env;


pub fn negamax(
    board: &mut Board,
    tt: &mut TranspositionTable,
    depth: u32,
    mut alpha: i32,
    mut beta: i32,
    moves_buf: &mut MoveList,
    ctx: &mut OrderingContext,
) -> i32 {
    let original_alpha = alpha;
    let current_hash = board.hash;

    // TT probe
    let mut hash_move: Option<Move> = None;
    if let Some(entry) = tt.probe(current_hash) {
        if entry.depth >= depth {
            match entry.bound_type {
                BoundType::Exact => return entry.score,
                BoundType::LowerBound => alpha = alpha.max(entry.score),
                BoundType::UpperBound => beta = beta.min(entry.score),
            }
            if alpha >= beta {
                return entry.score;
            }
        }
        hash_move = entry.best_move;
    }

    // Internal Iterative Deepening (IID): if we have no TT move and depth is large,
    // do a shallow search (depth-2) to populate the TT with a good move for ordering.
    if hash_move.is_none() && depth >= 3 {
        // perform a reduced-depth search to get a usable TT hint
        let _ = negamax(board, tt, depth - 2, alpha, beta, moves_buf, ctx);
        if let Some(entry) = tt.probe(current_hash) {
            hash_move = entry.best_move;
        }
    }

    if crate::search_control::should_stop() {
        return 0;
    }

    if board.is_draw() {
        return 0;
    }

    crate::search_control::node_visited();

    if depth == 0 {
        return quiesce(board, alpha, beta, moves_buf, ctx);
    }

    // Null-move pruning (Publius-style) with verification search on cutoff.
    // Only apply when depth is sufficiently large and not in check.
    if depth >= 3 && !board.is_in_check(board.current_color()) {
        // reduction R = 2 (default); allow simple depth-adaptive tweak in future
        let r = 2u32;
        let null_info = board.make_null_move();
        let null_score = -negamax(board, tt, depth - 1 - r, -beta, -beta + 1, moves_buf, ctx);
        board.unmake_null_move(null_info);
        if null_score >= beta {
            // Verification search: do a cautious full (or full-window) search at reduced depth
            // to avoid zugzwang false positives. If verification confirms >= beta, store and return.
            // Use depth-1 for verification (safe since depth >= 3 here).
            let verify_score = -negamax(board, tt, depth - 1, -beta, -alpha, moves_buf, ctx);
            if verify_score >= beta {
                tt.store(current_hash, depth, verify_score, BoundType::LowerBound, None);
                return verify_score;
            }
            // Otherwise, fall through and search normally (no premature cutoff).
        }
    }

    moves_buf.clear();
    board.generate_moves_into(moves_buf);
    // Use ordering heuristics (TT move already extracted above)
    order_moves(ctx, board, &mut moves_buf[..], depth as usize, hash_move);

    if moves_buf.is_empty() {
        let current_color = board.current_color();
        return if board.is_in_check(current_color) {
            -(crate::constants::MATE_SCORE - (100 - depth as i32))
        } else {
            0
        };
    }

    if let Some(hm) = &hash_move {
        if let Some(pos) = moves_buf.iter().position(|m| m == hm) {
            moves_buf.swap(0, pos);
        }
    }

    let mut best_score: i32 = -crate::constants::MATE_SCORE * 2;
    let mut best_move_found: Option<Move> = None;

    // Take the reusable child buffer out of the ordering context so we can
    // borrow it independently without causing multiple mutable borrows of ctx.
    let mut child_buf = std::mem::take(&mut ctx.child_buf);
    child_buf.clear();
    child_buf.reserve(moves_buf.len().max(16));
    for (i, m) in moves_buf.iter().enumerate() {
        if crate::search_control::should_stop() {
            break;
        }
        // Capture attacker piece type before making the move (board state will change)
        let attacker_piece = board.piece_at(m.from).map(|(_c, p)| p);
        // Futility pruning: at very shallow depths (near leaf), skip quiet moves
        // that are unlikely to raise alpha. This is a conservative heuristic.
        let is_quiet = m.captured_piece.is_none() && m.promotion.is_none();
        if is_quiet && depth <= 2 {
            // small margin per ply (centipawns)
            let margin = if depth == 1 { 150 } else { 80 };
            let stand_pat = board.evaluate();
            if stand_pat + margin <= alpha {
                // treat as a non-improving move: continue to next move
                continue;
            }
        }

    // Prepare to make the move and search. We'll apply Late Move Reductions (LMR)
    // for non-captures/non-promotions that appear late in the move ordering.
    let info = board.make_move(m);

        let mut score: i32;
        if i == 0 {
            // principal variation move - full depth
            score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
        } else {
            let mut did_lmr = false;
            let mut reduced_score = -crate::constants::MATE_SCORE * 2;

            // Determine if LMR is applicable: non-capture, no promotion, and depth >= 3
            if m.captured_piece.is_none() && m.promotion.is_none() && depth >= 3 {
                // apply reductions only for sufficiently late moves
                if i >= 4 {
                    // basic reduction formula: 1 + (i / 6), capped to depth-2
                    let mut red = 1u32 + (i as u32 / 6);
                    if red > depth.saturating_sub(2) {
                        red = depth.saturating_sub(2);
                    }
                    if red > 0 {
                        did_lmr = true;
                        let reduced_depth = depth - 1 - red;
                        reduced_score = -negamax(board, tt, reduced_depth, -alpha - 1, -alpha, &mut child_buf, ctx);
                    }
                }
            }

            if did_lmr {
                score = reduced_score;
                // If reduced search suggests the move might be interesting, do full null-window then full-window re-search
                if score > alpha && score < beta {
                    score = -negamax(board, tt, depth - 1, -alpha - 1, -alpha, &mut child_buf, ctx);
                    if score > alpha && score < beta {
                        score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
                    }
                }
            } else {
                // No reduction: PVS-style search
                score = -negamax(board, tt, depth - 1, -alpha - 1, -alpha, &mut child_buf, ctx);
                if score > alpha && score < beta {
                    score = -negamax(board, tt, depth - 1, -beta, -alpha, &mut child_buf, ctx);
                }
            }
        }

        board.unmake_move(m, info);

        // If this move improved best_score, record history for non-capture moves
        if score > best_score {
            best_score = score;
            best_move_found = Some(*m);
            if m.captured_piece.is_none() {
                if let Some(piece) = attacker_piece {
                    // small increment for history
                    ctx.record_history(piece, m.from.0 as u8, m.to.0 as u8, 1);
                }
            }
        }

        alpha = alpha.max(best_score);
        // On beta cutoff, record killer for non-captures and boost history
        if alpha >= beta {
            if m.captured_piece.is_none() {
                ctx.record_killer(depth as usize, *m);
                if let Some(piece) = attacker_piece {
                    ctx.record_history(piece, m.from.0 as u8, m.to.0 as u8, 32);
                }
            }
            break;
        }
    }

    let bound_type = if best_score <= original_alpha {
        BoundType::UpperBound
    } else if best_score >= beta {
        BoundType::LowerBound
    } else {
        BoundType::Exact
    };

    // Return the child buffer to the ordering context before storing TT and returning
    ctx.child_buf = child_buf;

    tt.store(current_hash, depth, best_score, bound_type, best_move_found);

    best_score
}

// Null-move helpers removed.
/// Quiescence search extracted from Board::quiesce
pub fn quiesce(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    moves_buf: &mut MoveList,
    ctx: &mut OrderingContext,
) -> i32 {
    let stand_pat_score = board.evaluate();
    if board.is_draw() {
        return 0;
    }
    if crate::search_control::should_stop() {
        return stand_pat_score;
    }
    crate::search_control::node_visited();
    if stand_pat_score >= beta {
        return beta;
    }
    alpha = alpha.max(stand_pat_score);

    moves_buf.clear();
    board.generate_tactical_moves_into(moves_buf);
    // Optionally disable SEE-based pruning/ordering via env var for diagnostics.
    if env::var_os("CHESS_DISABLE_SEE").is_none() {
        // Use SEE to prune obviously losing captures and order by SEE value
        moves_buf.retain(|m| {
            let s = see_capture(board, m);
            s >= 0
        });
        moves_buf.sort_by_key(|m| -see_capture(board, m));
    }
    order_moves(ctx, board, &mut moves_buf[..], 0, None);

    let mut best_score = stand_pat_score;
    let tactical_moves = moves_buf.clone();
    for m in tactical_moves {
        if crate::search_control::should_stop() {
            break;
        }
    let info = board.make_move(&m);
    let score = -quiesce(board, -beta, -alpha, moves_buf, ctx);
    board.unmake_move(&m, info);

        best_score = best_score.max(score);
        alpha = alpha.max(best_score);
        if alpha >= beta {
            break;
        }
    }

    alpha
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
    let mut best_move: Option<Move> = None;

    let mut legal_moves: MoveList = MoveList::new();
    board.generate_moves_into(&mut legal_moves);
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves[0]);
    }
    let mut root_moves = legal_moves;

    let search_start = Instant::now();
    let mut prev_score: Option<i32> = None;
    for depth in 1..=max_depth {
        // Bump TT generation so entries written at this depth are preferred
        tt.new_generation();
        // Progressive aspiration: if we have a previous score and depth is big enough,
        // try small windows around prev_score and widen (10,20,40,... up to 500 cp)
        let mut mv_opt: Option<Move> = None;
        let mut score: i32 = 0;
        let mut completed = false;

        if let Some(ps) = prev_score {
            if depth > 2 && ps.abs() < (crate::constants::MATE_SCORE / 2) {
                let mut margin = 10i32;
                while margin <= 500 {
                    if crate::search_control::should_stop() {
                        break;
                    }
                    let a = ps.saturating_sub(margin);
                    let b = ps.saturating_add(margin);
                    let (mv_try, sc_try, completed_try) = board.run_root_search(
                        tt,
                        depth,
                        &mut root_moves[..],
                        crate::search_control::should_stop,
                        Some((a, b)),
                    );
                    if completed_try && sc_try > a && sc_try < b {
                        mv_opt = mv_try;
                        score = sc_try;
                        completed = true;
                        break;
                    }
                    if !completed_try && crate::search_control::should_stop() {
                        break;
                    }
                    margin = margin.saturating_mul(2);
                }
            }
        }

        // Fallback to full-window search if aspiration did not produce a completed result
        if !completed && !crate::search_control::should_stop() {
            let (mv_full, sc_full, completed_full) = board.run_root_search(
                tt,
                depth,
                &mut root_moves[..],
                crate::search_control::should_stop,
                None,
            );
            mv_opt = mv_full;
            score = sc_full;
            completed = completed_full;
        }

        if completed {
            if let Some(mv) = mv_opt {
                best_move = Some(mv);

                if let Some(ref s) = sink {
                    let mut lock = match s.lock() {
                        Ok(g) => g,
                        Err(poisoned) => {
                            eprintln!("warning: sink mutex poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    *lock = best_move;
                }

                if let Some(ref sender) = info_sender {
                    let nodes_total = crate::search_control::get_node_count();
                    let elapsed_ms = search_start.elapsed().as_millis();
                    let nps = if elapsed_ms > 0 {
                        Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
                    } else {
                        None
                    };
                    let pv = build_pv_from_tt(tt, board.hash);
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
                    if score.abs() > (crate::constants::MATE_SCORE / 2) {
                        let mate_in = (crate::constants::MATE_SCORE - score.abs() + 1) / 2;
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

                if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                    root_moves.swap(0, pos);
                }
            // record previous score for next depth's aspiration window
            prev_score = Some(score);
            }
        }
    }

    best_move
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
    let mut best_move: Option<Move> = None;
    let mut depth = 1u32;
    let mut last_depth_time = Duration::from_millis(1);

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;

    let mut prev_score: Option<i32> = None;
    while start_time.elapsed() + SAFETY_MARGIN < max_time {
        // Bump generation each iterative step so TT replacement prefers newer entries
        tt.new_generation();
        let elapsed = start_time.elapsed();
        let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();

        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if estimated_next_time + SAFETY_MARGIN > time_remaining {
            break;
        }

        let depth_start = Instant::now();

        let mut legal_moves: MoveList = MoveList::new();
        board.generate_moves_into(&mut legal_moves);
        if legal_moves.is_empty() {
            return None;
        }
        if legal_moves.len() == 1 {
            return Some(legal_moves[0]);
        }

        legal_moves.sort_by_key(|m| -crate::board::mvv_lva_score(m, board));
        apply_tt_move_hint(&mut legal_moves[..], tt, board.hash);

        // Progressive aspiration similar to iterative driver
        let mut this_best_move: Option<Move> = None;
        let mut this_best_score: i32 = 0;
        let mut completed = false;
        if let Some(ps) = prev_score {
            if depth > 2 && ps.abs() < (crate::constants::MATE_SCORE / 2) {
                let mut margin = 10i32;
                while margin <= 500 {
                    if start_time.elapsed() + SAFETY_MARGIN >= max_time {
                        break;
                    }
                    let a = ps.saturating_sub(margin);
                    let b = ps.saturating_add(margin);
                    let (mv_try, sc_try, completed_try) = board.run_root_search(
                        tt,
                        depth,
                        &mut legal_moves[..],
                        || start_time.elapsed() + SAFETY_MARGIN >= max_time,
                        Some((a, b)),
                    );
                    if completed_try && sc_try > a && sc_try < b {
                        this_best_move = mv_try;
                        this_best_score = sc_try;
                        completed = true;
                        break;
                    }
                    if !completed_try && start_time.elapsed() + SAFETY_MARGIN >= max_time {
                        break;
                    }
                    margin = margin.saturating_mul(2);
                }
            }
        }

        if !completed {
            let (mv_full, sc_full, completed_full) = board.run_root_search(
                tt,
                depth,
                &mut legal_moves[..],
                || start_time.elapsed() + SAFETY_MARGIN >= max_time,
                None,
            );
            if completed_full {
                this_best_move = mv_full;
                this_best_score = sc_full;
                completed = true;
            }
        }

        if start_time.elapsed() + SAFETY_MARGIN < max_time {
            if completed {
                best_move = this_best_move;
                if let Some(ref s) = sink {
                    let mut lock = match s.lock() {
                        Ok(g) => g,
                        Err(poisoned) => {
                            eprintln!("warning: sink mutex poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    *lock = best_move;
                }

                if let Some(ref sender) = info_sender {
                    let nodes_total = crate::search_control::get_node_count();
                    let elapsed_ms = start_time.elapsed().as_millis();
                    let nps = if elapsed_ms > 0 {
                        Some(((nodes_total as u128 * 1000) / elapsed_ms) as u64)
                    } else {
                        None
                    };
                    let pv = build_pv_from_tt(tt, board.hash);
                    let mut info = uci_info::Info {
                        depth: Some(depth),
                        nodes: Some(nodes_total),
                        nps,
                        time_ms: Some(elapsed_ms),
                        score_cp: None,
                        score_mate: None,
                        pv: Some(pv),
                        seldepth: None,
                        ponder: None,
                    };
                    if this_best_score.abs() > (crate::constants::MATE_SCORE / 2) {
                        let mate_in = (crate::constants::MATE_SCORE - this_best_score.abs() + 1) / 2;
                        info.score_mate = Some(mate_in);
                    } else {
                        info.score_cp = Some(this_best_score);
                    }
                    if is_ponder {
                        if let Some(bm) = best_move {
                            info.ponder = Some(format!("{}{}", format_square(bm.from), format_square(bm.to)));
                        }
                    }
                    let _ = sender.send(info);
                }

                // rotate best move to front in root moves
                if let Some(bm) = best_move {
                    if let Some(pos) = legal_moves.iter().position(|m| *m == bm) {
                        legal_moves.swap(0, pos);
                    }
                }
                // record prev_score for next depth's aspiration window
                prev_score = Some(this_best_score);
            }

            last_depth_time = depth_start.elapsed();
            depth += 1;
        } else {
            break;
        }
    }

    best_move
}

/// Build a principal variation (PV) string by following TT best-move entries starting
/// from `start_hash`. Used to include a PV in UCI info messages.
pub fn build_pv_from_tt(tt: &TranspositionTable, start_hash: u64) -> String {
    let mut pv = Vec::new();
    if let Some(entry) = tt.probe(start_hash) {
        if let Some(mv) = entry.best_move {
            pv.push(mv);
        }
    }
    let pv_strs: Vec<String> = pv
        .iter()
        .map(|m| format!("{}{}", format_square(m.from), format_square(m.to)))
        .collect();
    pv_strs.join(" ")
}

/// If the transposition table contains a best-move hint for `hash`, and that move
/// is present in `moves`, swap it to index 0 so it will be searched first.
pub fn apply_tt_move_hint(moves: &mut [Move], tt: &TranspositionTable, hash: u64) {
    if let Some(entry) = tt.probe(hash) {
        if let Some(hm) = &entry.best_move {
            if let Some(pos) = moves.iter().position(|m| m == hm) {
                moves.swap(0, pos);
            }
        }
    }
}
