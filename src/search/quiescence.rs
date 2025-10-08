use crate::core::types::MoveList;
use crate::core::board::Board;
use crate::movegen::ordering::OrderingContext;
use crate::evaluation::eval;
use crate::movegen::ordering::order_moves;

/// Quiescence search to avoid horizon effect
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

    // Generate checks if in check or if stand pat is low (delta pruning)
    let in_check = board.is_in_check(board.current_color());
    if in_check || stand_pat_score + 200 >= alpha {
        let mut check_buf = MoveList::new();
        board.generate_moves_into(&mut check_buf);
        // Filter to only checking moves
        check_buf.retain(|m| {
            let info = board.make_move(m);
            let gives_check = board.is_in_check(board.current_color());
            board.unmake_move(m, info);
            gives_check
        });
        local_buf.extend(check_buf);
    }

    // Apply SEE-based pruning and ordering
    apply_see_pruning_and_ordering(board, &mut local_buf);
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