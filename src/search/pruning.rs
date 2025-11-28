use crate::transposition::transposition_table::BoundType;
use crate::core::board::Board;
use crate::core::config::evaluation::MATE_SCORE;
use crate::search::search_context::SearchContext;

/// Check if the current position is likely zugzwang (only king and pawns)
/// In such positions, null move pruning is unsafe
pub fn is_zugzwang_position(board: &Board) -> bool {
    let color_idx = if board.white_to_move { 0 } else { 1 };
    let opponent_idx = 1 - color_idx;

    // Count pieces for the side to move
    let mut piece_count = 0;
    for piece_type in 0..6 { // Pawn, Knight, Bishop, Rook, Queen, King
        piece_count += board.bitboards[color_idx][piece_type].count_ones();
    }

    // If we have very few pieces (king + pawns only), likely zugzwang
    // Also check if opponent has many pieces (we might need to move to defend)
    let opponent_piece_count = (0..6).map(|pt| board.bitboards[opponent_idx][pt].count_ones()).sum::<u32>();

    piece_count <= 4 && opponent_piece_count > 3
}

/// Apply mate distance pruning to alpha and beta bounds
pub fn mate_distance_pruning(mut alpha: i32, mut beta: i32, ply: usize) -> (i32, i32) {
    alpha = alpha.max(-(MATE_SCORE - (ply as i32)));
    beta = beta.min(MATE_SCORE - (ply as i32));
    (alpha, beta)
}

/// Check if futility pruning should skip this quiet move
pub fn should_futility_prune(
    in_check: bool,
    depth: u32,
    alpha: i32,
    is_quiet: bool,
    static_eval: i32,
) -> bool {
    // Only apply if not in check, and it's a quiet move (not a capture or promotion)
    if in_check || !is_quiet {
        return false;
    }

    // Futility pruning: if the static evaluation is already significantly
    // worse than alpha, it's unlikely a quiet move will improve the score.
    // Apply more aggressively at deeper depths.
    if depth == 1 && static_eval + 200 < alpha {
        true
    } else if depth == 2 && static_eval + 300 < alpha {
        true
    } else if depth == 3 && static_eval + 500 < alpha {
        true
    } else if depth == 4 && static_eval + 700 < alpha {
        true
    } else {
        false
    }
}

/// Check if late move pruning should skip this move
pub fn should_late_move_prune(
    depth: u32,
    move_index: usize,
    is_quiet: bool,
) -> bool {
    // Late Move Pruning (LMP): at low depths, prune late quiet moves
    is_quiet && depth <= 6 && move_index >= ((3 + depth * depth) / 2) as usize
}

/// Apply null move pruning with verification search
pub fn null_move_pruning(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    depth: u32,
    beta: i32,
    alpha: i32,
    current_hash: u64,
) -> Option<i32> {
    // Respect ply cap from caller.
    const MAX_PLY: usize = 64;
    if s_ctx.ply >= MAX_PLY {
        return None;
    }
    // Only apply when depth is sufficiently large, not in check, not zugzwang, and not near mate
    if depth < 3 || board.is_in_check(board.current_color())
       || is_zugzwang_position(board)
       || beta.abs() >= (MATE_SCORE - 100) {
        return None;
    }

    //  adaptive reduction: 4 + depth/6 + eval-beta bonus
    let eval = crate::evaluation::eval::evaluate(board, s_ctx.pawn_hash_table);
    let eval_beta_bonus = if eval >= beta { 1 } else { 0 };
    let r = 4u32.saturating_add(depth / 6).saturating_add(eval_beta_bonus);
    let r = r.min(depth.saturating_sub(1)); // Ensure we don't reduce below 1 ply

    let null_info = board.make_null_move();
    // Use a child context with incremented ply to respect ply caps.
    let mut child_ctx = SearchContext {
        tt: s_ctx.tt,
        moves_buf: s_ctx.moves_buf,
        ordering_ctx: s_ctx.ordering_ctx,
        ply: s_ctx.ply + 1,
        pawn_hash_table: s_ctx.pawn_hash_table,
    };
    if crate::search::control::should_stop() {
        board.unmake_null_move(null_info);
        return None;
    }
    let null_score = -crate::search::algorithms::negamax(board, &mut child_ctx, depth - 1 - r, -beta, -beta + 1);
    board.unmake_null_move(null_info);

    if null_score >= beta {
        // Verification search at the same reduced depth to confirm cutoff
        let verify_score = -crate::search::algorithms::negamax(board, s_ctx, depth - 1 - r, -beta, -alpha);
        if verify_score >= beta {
            s_ctx.tt.store(current_hash, depth, verify_score, BoundType::LowerBound, None);
            return Some(verify_score);
        }
        // Otherwise, fall through and search normally (no premature cutoff).
    }

    None
}
