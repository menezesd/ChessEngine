use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::{Move, MoveList};
use crate::core::board::Board;
use crate::movegen::ordering::OrderingContext;
use crate::evaluation::eval;
// use crate::core::constants;
use crate::core::config::evaluation::*;
use crate::movegen::ordering::order_moves;
use crate::search::pruning::*;
use crate::search::move_selector::{MoveSelector, TranspositionTableHelper};
use crate::search::extensions::*;
use crate::search::lmr::*;
use crate::search::quiescence;

/// Negamax search with alpha-beta pruning
#[allow(clippy::too_many_arguments)]
pub fn negamax(
    board: &mut Board,
    tt: &mut TranspositionTable,
    depth: u32,
    alpha: i32,
    beta: i32,
    moves_buf: &mut MoveList,
    ctx: &mut OrderingContext,
    ply: usize,
) -> i32 {
    // Mate distance pruning: if we're already mating, don't search for longer mates
    let (alpha, beta) = mate_distance_pruning(alpha, beta, ply);
    if alpha >= beta {
        return alpha;
    }

    let _original_alpha = alpha;
    let current_hash = board.hash;

    // TT probe using helper
    let (mut hash_move, alpha, beta, tt_result) = TranspositionTableHelper::probe_and_adjust_bounds(tt, current_hash, depth, alpha, beta);

    if let Some(score) = tt_result {
        return score;
    }

    // Internal Iterative Deepening (IID): if we have no TT move and depth is large,
    // do a shallow search (depth-2) to populate the TT with a good move for ordering.
    if hash_move.is_none() {
        hash_move = TranspositionTableHelper::internal_iterative_deepening(tt, board, current_hash, depth, alpha, beta, moves_buf, ctx, ply);
    }

    // Razoring: at shallow depths, if evaluation + margin is below beta, drop to quiescence
    if depth <= 3 && !board.is_in_check(board.current_color()) {
        let eval = crate::evaluation::eval::evaluate(board);
        let margin = 200 * depth as i32;
        if eval + margin < beta {
            let mut child_buf = std::mem::take(&mut ctx.child_buf);
            child_buf.clear();
            let score = quiescence::quiesce(board, alpha, beta, &mut child_buf, ctx);
            ctx.child_buf = child_buf;
            return score;
        }
    }

    // Singular extension: if we have a TT move and it's much better than alternatives,
    // extend the search for this move
    let singular_ext = singular_extension(board, tt, depth, hash_move, current_hash, moves_buf, ctx, ply);        if crate::search::control::should_stop() {
            return 0;
        }

        if board.is_draw() {
            return 0;
        }

        crate::search::control::node_visited();

        if depth == 0 {
            // Take the child buffer for quiesce
            let mut child_buf = std::mem::take(&mut ctx.child_buf);
            child_buf.clear();
            let score = quiescence::quiesce(board, alpha, beta, &mut child_buf, ctx);
            ctx.child_buf = child_buf;
            return score;
        }

        // Null-move pruning with verification search on cutoff.
        if let Some(null_prune_score) = null_move_pruning(board, tt, depth, beta, alpha, moves_buf, ctx, ply, current_hash) {
            return null_prune_score;
        }

        let (best_score, best_move_found) = search_moves(board, tt, depth, alpha, beta, hash_move, singular_ext, ctx, ply);

        let bound_type = if best_score <= _original_alpha {
            crate::transposition::transposition_table::BoundType::UpperBound
        } else if best_score >= beta {
            crate::transposition::transposition_table::BoundType::LowerBound
        } else {
            crate::transposition::transposition_table::BoundType::Exact
        };

        TranspositionTableHelper::store_result(tt, current_hash, depth, best_score, bound_type, best_move_found);

        best_score
    }

/// Search all moves from the current position and return the best score and move
fn search_moves(
    board: &mut Board,
    tt: &mut TranspositionTable,
    depth: u32,
    mut alpha: i32,
    beta: i32,
    hash_move: Option<Move>,
    singular_ext: u32,
    ctx: &mut OrderingContext,
    ply: usize,
) -> (i32, Option<Move>) {
    let mut move_selector = MoveSelector::new();
    move_selector.generate_and_order(board, ctx, depth, hash_move);

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
    let mut child_buf = std::mem::take(&mut ctx.child_buf);
    child_buf.clear();
    child_buf.reserve(move_selector.len().max(16));

    while let Some((move_idx, m)) = move_selector.next() {
        if crate::search::control::should_stop() {
            break;
        }

        // Capture attacker piece type before making the move (board state will change)
        let attacker_piece = board.piece_at(m.from).map(|(_c, p)| p);

        // Apply pruning techniques
        let is_quiet = m.captured_piece.is_none() && m.promotion.is_none();
        if should_futility_prune(board, depth, alpha, is_quiet) {
            continue;
        }
        if should_late_move_prune(depth, move_idx, is_quiet) {
            continue;
        }

        // Make the move and calculate extensions
        let info = board.make_move(m);
        let check_ext = check_extension(board, move_idx, depth);
        let total_extension = check_ext + singular_ext;

        // Search the move with potential LMR
        let score = if move_idx == 0 {
            // Principal variation move - full depth
            -negamax(board, tt, depth - 1 + total_extension, -beta, -alpha, &mut child_buf, ctx, ply + 1)
        } else {
            // Apply LMR for non-PV moves
            apply_lmr_and_research(
                board, tt, depth, total_extension, alpha, beta,
                move_idx, !is_quiet, m.promotion.is_some(),
                &mut child_buf, ctx, ply
            )
        };

        board.unmake_move(m, info);

        // Update best score and move
        if score > best_score {
            best_score = score;
            best_move_found = Some(*m);

            // Record history for non-capture moves
            if m.captured_piece.is_none() {
                if let Some(piece) = attacker_piece {
                    ctx.record_history(piece, m.from.0 as u8, m.to.0 as u8, 1);
                }
            }
        }

        alpha = alpha.max(best_score);

        // Beta cutoff - record killer and history, then break
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

    // Return the child buffer to the ordering context
    ctx.child_buf = child_buf;

    (best_score, best_move_found)
}
pub fn quiesce(
    board: &mut Board,
    mut alpha: i32,
    beta: i32,
    _moves_buf: &mut MoveList,  // Not used, kept for API compatibility
    ctx: &mut OrderingContext,
) -> i32 {
        let stand_pat_score = eval::evaluate(board);
        if board.is_draw() {
            return 0;
        }
        if crate::search::control::should_stop() {
            return stand_pat_score;
        }
        crate::search::control::node_visited();
        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut local_buf = MoveList::new();
        board.generate_tactical_moves_into(&mut local_buf);
        // Optionally disable SEE-based pruning/ordering via env var for diagnostics.
        if std::env::var_os("CHESS_DISABLE_SEE").is_none() {
            // Use SEE to prune obviously losing captures and order by SEE value
            local_buf.retain(|m| {
                let s = crate::see::see_capture(board, m);
                s >= 0
            });
            local_buf.sort_by_key(|m| -crate::see::see_capture(board, m));
        }
        order_moves(ctx, board, &mut local_buf[..], 0, None);

        let mut best_score = stand_pat_score;
        for m in &local_buf {
            if crate::search::control::should_stop() {
                break;
            }
            let info = board.make_move(m);
            let score = -quiesce(board, -beta, -alpha, &mut MoveList::new(), ctx);
            board.unmake_move(m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);
            if alpha >= beta {
                break;
            }
        }

        alpha
    }