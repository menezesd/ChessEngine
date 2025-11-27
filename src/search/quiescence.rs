use crate::core::types::MoveList;
use crate::core::board::Board;
use crate::evaluation::eval;
use crate::movegen::ordering::order_moves;
use crate::search::search_context::SearchContext;

/// Quiescence search to avoid horizon effect
pub fn quiesce(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    mut alpha: i32,
    beta: i32,
) -> i32 {
    let in_check = board.is_in_check(board.current_color());

    // In check: generate all moves to escape check
    if in_check {
        let mut all_moves = MoveList::new();
        board.generate_moves_into(&mut all_moves); // This generates all legal moves

        let mut best_score = -i32::MAX;
        for m in &all_moves {
            if crate::search::control::should_stop() {
                break;
            }
            let info = board.make_move(m);
            let score = -quiesce(board, s_ctx, -beta, -alpha);
            board.unmake_move(m, info);

            best_score = best_score.max(score);
            alpha = alpha.max(best_score);
            if alpha >= beta {
                return beta; // Fail hard beta-cutoff
            }
        }
        return best_score; // Return the best score found to escape check
    }


    let stand_pat_score = eval::evaluate(board, s_ctx.pawn_hash_table);
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

    // Apply SEE-based pruning and ordering
    apply_see_pruning_and_ordering(board, &mut local_buf);
    order_moves(s_ctx.ordering_ctx, board, &mut local_buf[..], 0, None);

    let mut best_score = stand_pat_score;
    for m in &local_buf {
        if crate::search::control::should_stop() {
            break;
        }
        let info = board.make_move(m);
        let mut child_s_ctx = SearchContext {
            tt: s_ctx.tt,
            moves_buf: &mut MoveList::new(),
            ordering_ctx: s_ctx.ordering_ctx,
            ply: s_ctx.ply + 1,
            pawn_hash_table: s_ctx.pawn_hash_table,
        };
        let score = -quiesce(board, &mut child_s_ctx, -beta, -alpha);
        board.unmake_move(m, info);

        best_score = best_score.max(score);
        alpha = alpha.max(best_score);
        if alpha >= beta {
            break;
        }
    }

    alpha
}

/// Apply SEE-based pruning and ordering to tactical moves
fn apply_see_pruning_and_ordering(board: &Board, moves: &mut MoveList) {
    // Optionally disable SEE-based pruning/ordering via env var for diagnostics.
    if std::env::var_os("CHESS_DISABLE_SEE").is_none() {
        // Use SEE to prune obviously losing captures and order by SEE value
        moves.retain(|m| {
            let s = crate::see::see_capture(board, m);
            s >= 0
        });
        moves.sort_by_key(|m| -crate::see::see_capture(board, m));
    }
}