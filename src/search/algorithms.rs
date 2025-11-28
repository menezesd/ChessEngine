use crate::search::search_context::SearchContext;
use crate::core::types::Move;
use crate::core::board::Board;
// use crate::core::constants;
use crate::core::config::evaluation::*;
use crate::search::pruning::*;
use crate::search::move_selector::{MoveSelector, TranspositionTableHelper};
use crate::search::extensions::*;
use crate::search::lmr::*;
use crate::search::quiescence;

/// Negamax search with alpha-beta pruning
pub fn negamax(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    depth: u32,
    alpha: i32,
    beta: i32,
) -> i32 {

    const MAX_PLY: usize = 32; // tightened for debugging
    crate::search::control::update_max_ply(s_ctx.ply);
    if cfg!(debug_assertions) {
        
    }
    if s_ctx.ply >= MAX_PLY {
        if cfg!(debug_assertions) {
            
        }
        return crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table);
    }
    if cfg!(debug_assertions) && depth == 0 {
        
    }
    if depth == 0 {
        return crate::search::quiescence::quiesce(board, s_ctx, alpha, beta);
    }
    if crate::search::control::should_stop() {
        return crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table);
    }
    // Mate distance pruning: if we're already mating, don't search for longer mates
    let (alpha, beta) = mate_distance_pruning(alpha, beta, s_ctx.ply);
    if alpha >= beta {
        return alpha;
    }
    if cfg!(debug_assertions) && s_ctx.ply > 20 {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("trace.txt")
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "negamax ply {} depth {}", s_ctx.ply, depth)
            });
    }

    let _original_alpha = alpha;
    let current_hash = board.hash;

    // TT probe using helper
    let (mut hash_move, alpha, beta, tt_cutoff_score, tt_stored_score) = TranspositionTableHelper::probe_and_adjust_bounds(s_ctx.tt, current_hash, depth, alpha, beta);

    if let Some(score) = tt_cutoff_score {
        return score;
    }

    // Reverse Futility Pruning (RFP)
    // If the static evaluation of the position plus a margin is already
    // greater than or equal to beta, we can prune this branch.
    // This is applied at shallow depths only and not in check.
    if depth <= 3 && !board.is_in_check(board.current_color()) {
        let static_eval = tt_stored_score.unwrap_or_else(|| crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table));
        if static_eval - RFP_MARGIN >= beta {
            return static_eval;
        }
    }

    // Internal Iterative Deepening (IID): if we have no TT move and depth is large,
    // do a shallow search (depth-2) to populate the TT with a good move for ordering.
    if hash_move.is_none() {
        hash_move = TranspositionTableHelper::internal_iterative_deepening(board, s_ctx, current_hash, depth, alpha, beta);
    }

    // Razoring: at shallow depths, if evaluation + margin is below beta, drop to quiescence
    if depth <= 3 && !board.is_in_check(board.current_color()) {
        let eval = tt_stored_score.unwrap_or_else(|| crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table));
        let margin = 200 * depth as i32;
        if eval + margin < beta {
            let mut child_buf = std::mem::take(&mut s_ctx.ordering_ctx.child_buf);
            child_buf.clear();
            let score = quiescence::quiesce(board, s_ctx, alpha, beta);
            s_ctx.ordering_ctx.child_buf = child_buf;
            return score;
        }
    }

    // Null-move pruning with verification search on cutoff.
    if let Some(null_prune_score) = null_move_pruning(board, s_ctx, depth, beta, alpha, current_hash) {
        return null_prune_score;
    }

    let mut singular_ext = 0;
    // Singular extension: if hash move is quiet and causes a beta cutoff even at reduced depth
    // it suggests it's a "singular" move. This extends its search depth.
    if depth >= crate::core::config::search::SINGULAR_EXTENSION_MIN_DEPTH
        && hash_move.is_some()
        && !board.is_in_check(board.current_color()) {

                    let hm = hash_move.unwrap();
                    // Only consider singular extension for quiet moves
                    if hm.captured_piece.is_none() && hm.promotion.is_none() {
                        let mut temp_move_list = crate::core::types::MoveList::new();
                        let mut temp_ordering_ctx = crate::movegen::ordering::OrderingContext::new(MAX_PLY);
        
                        let mut singular_s_ctx = SearchContext {
                            tt: s_ctx.tt,
                            moves_buf: &mut temp_move_list,
                            ordering_ctx: &mut temp_ordering_ctx,
                            ply: s_ctx.ply + 1,
                            pawn_hash_table: s_ctx.pawn_hash_table,
                        };
                        let info = board.make_move(&hm);
                        let r_depth = depth - crate::core::config::search::SINGULAR_EXTENSION_VERIFICATION_REDUCTION;
                        let singular_beta = alpha + crate::core::config::search::SINGULAR_EXTENSION_MARGIN;
                        let singular_score = -negamax(board, &mut singular_s_ctx, r_depth, -singular_beta, -alpha);
                        board.unmake_move(&hm, info);
        
                        if singular_score >= singular_beta {
                            // The move is singular, apply an extension
                            singular_ext = 1;
                            // If it's a strong singular move, apply a double extension
                            if singular_score >= beta {
                                singular_ext = 2;
                            }
                        }
                    }    }
    if crate::search::control::should_stop() {
        return 0;
    }

    if board.is_draw() {
        return 0;
    }

    crate::search::control::node_visited();

    if depth == 0 {
        // Take the child buffer for quiesce
        let mut child_buf = std::mem::take(&mut s_ctx.ordering_ctx.child_buf);
        child_buf.clear();
        let score = quiescence::quiesce(board, s_ctx, alpha, beta);
        s_ctx.ordering_ctx.child_buf = child_buf;
        return score;
    }

    // Null-move pruning with verification search on cutoff.
    if let Some(null_prune_score) = null_move_pruning(board, s_ctx, depth, beta, alpha, current_hash) {
        return null_prune_score;
    }

    // Multi-cut pruning disabled for stability.

    // Parent static eval used for improving/WASP and correction updates.
    let parent_static_eval = tt_stored_score.unwrap_or_else(|| crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table));

    let (best_score, best_move_found) = search_moves(
        board,
        s_ctx,
        depth,
        alpha,
        beta,
        hash_move,
        singular_ext,
        tt_stored_score,
        parent_static_eval,
    );

    let bound_type = if best_score <= _original_alpha {
        crate::transposition::transposition_table::BoundType::UpperBound
    } else if best_score >= beta {
        crate::transposition::transposition_table::BoundType::LowerBound
    } else {
        crate::transposition::transposition_table::BoundType::Exact
    };

    TranspositionTableHelper::store_result(s_ctx.tt, current_hash, depth, best_score, bound_type, best_move_found);

    best_score
}

/// Search all moves from the current position and return the best score and move
fn search_moves(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    depth: u32,
    mut alpha: i32,
    beta: i32,
    hash_move: Option<Move>,
    singular_ext: u32,
    tt_stored_score: Option<i32>,
    parent_static_eval: i32,
) -> (i32, Option<Move>) {
    let mut move_selector = MoveSelector::new();
    move_selector.generate_and_order(board, s_ctx.ordering_ctx, depth, hash_move);

    let in_check = board.is_in_check(board.current_color());
    // Cache the static evaluation once per node for futility pruning and correction history.
    let mut static_eval_cache = tt_stored_score;
    let base_eval = static_eval_cache.unwrap_or_else(|| {
        let e = crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table);
        static_eval_cache = Some(e);
        e
    });
    let prev_move_to = board.last_move_made.map(|m| m.to.0 as u8);

    if move_selector.is_empty() {
        let current_color = board.current_color();
        let score = if board.is_in_check(current_color) {
            -(MATE_SCORE - (100 - depth as i32))
        } else {
            0
        };
        return (score, None);
    }

    let mut best_score = -MATE_SCORE * 2;
    let mut best_move_found = None;

    // Take the reusable child buffer out of the ordering context
    let mut child_buf = std::mem::take(&mut s_ctx.ordering_ctx.child_buf);
    child_buf.clear();
    child_buf.reserve(move_selector.len().max(16));

    while let Some((move_idx, m)) = move_selector.next() {
        if crate::search::control::should_stop() {
            break;
        }

        // Capture attacker piece type before making the move (board state will change)
        let attacker_piece = board.piece_at(m.from).map(|(_c, p)| p);

        // Guarded SEE pruning for shallow non-PV moves
        if depth <= 3 && move_idx > 0 {
            if let Some(_cap) = m.captured_piece {
                let see = crate::movegen::see::see_capture(board, m);
                // Allow good sacs but prune very bad captures with margin.
                if see < -100 {
                    continue;
                }
            }
        }

        // Apply pruning techniques
        let is_quiet = m.captured_piece.is_none() && m.promotion.is_none();
        if !in_check && is_quiet {
            // Late Move Pruning (LMP)
            if depth <= crate::core::config::search::LMP_DEPTH_THRESHOLD
                && move_idx >= crate::core::config::search::LMP_MOVE_INDEX_THRESHOLD
            {
                continue;
            }
            // History Pruning
            if depth < 5 { // Only apply at shallower depths
                if let Some(piece) = attacker_piece {
                    let hist_score = s_ctx.ordering_ctx.history_score(piece, m.from.0 as u8, m.to.0 as u8);
                    if hist_score < crate::core::config::search::HISTORY_PRUNING_THRESHOLD {
                        continue;
                    }
                }
            }
        if let Some(eval) = static_eval_cache.or_else(|| Some(base_eval)) {
            if should_futility_prune(in_check, depth, alpha, is_quiet, eval) {
                continue;
            }
        }
        }
        if should_late_move_prune(depth, move_idx, is_quiet) {
            continue;
        }

        // Make the move and calculate extensions
        let info = board.make_move(m);
        let check_ext = check_extension(board, move_idx, depth);
        let recapture_ext = recapture_extension(board, m);
        let mut total_extension = check_ext + singular_ext + recapture_ext;
        if total_extension > 2 {
            total_extension = 2;
        }

        let is_pv_move = move_idx == 0;
        let hist_score = {
            let ctx_ref: &crate::movegen::ordering::OrderingContext = s_ctx.ordering_ctx;
            crate::movegen::ordering::order_moves_score(ctx_ref, board, m)
        };
        let see_score = if m.captured_piece.is_some() {
            crate::movegen::see::see_capture(board, m)
        } else {
            0
        };

        let mut child_s_ctx = SearchContext {
            tt: s_ctx.tt,
            moves_buf: &mut child_buf,
            ordering_ctx: s_ctx.ordering_ctx,
            ply: s_ctx.ply + 1,
            pawn_hash_table: s_ctx.pawn_hash_table,
        };

        let score = if is_pv_move {
            -negamax(board, &mut child_s_ctx, depth - 1 + total_extension, -beta, -alpha)
        } else {
            apply_lmr_and_research(
                board,
                &mut child_s_ctx,
                depth,
                total_extension,
                is_pv_move,
                parent_static_eval,
                alpha,
                beta,
                hist_score,
                see_score,
                move_idx,
                !is_quiet,
                m.promotion.is_some(),
            )
        };

        board.unmake_move(m, info);

        // Update best score and move
        let is_better = score > best_score;
        if is_better {
            best_score = score;
            best_move_found = Some(*m);

            // Record history heuristics
            if let Some(piece) = attacker_piece {
                if m.captured_piece.is_none() {
                    s_ctx.ordering_ctx.record_history(piece, m.from.0 as u8, m.to.0 as u8, 1);
                    if let Some(prev) = prev_move_to {
                        s_ctx.ordering_ctx.record_continuation(prev, m.to.0 as u8, 2);
                    }
                } else {
                    s_ctx.ordering_ctx.record_capture_history(piece, m.to.0 as u8, 2);
                }
            }
        }

        alpha = alpha.max(best_score);

        // Beta cutoff - record killer/history/correction, then break
        if alpha >= beta {
            if let Some(piece) = attacker_piece {
                let delta = (score - base_eval).clamp(-64, 64);
                if delta.abs() > 8 {
                    s_ctx
                        .ordering_ctx
                        .record_correction(piece, m.to.0 as u8, delta / 4);
                }
                if m.captured_piece.is_none() {
                    s_ctx.ordering_ctx.record_killer(depth as usize, *m);
                    s_ctx
                        .ordering_ctx
                        .record_history(piece, m.from.0 as u8, m.to.0 as u8, 32);
                    if let Some(prev) = prev_move_to {
                        s_ctx
                            .ordering_ctx
                            .record_continuation(prev, m.to.0 as u8, 16);
                    }
                } else {
                    s_ctx
                        .ordering_ctx
                        .record_capture_history(piece, m.to.0 as u8, 8);
                }
            }
            break;
        }
    }

    // Return the child buffer to the ordering context
    s_ctx.ordering_ctx.child_buf = child_buf;

    (best_score, best_move_found)
}
