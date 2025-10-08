use crate::transposition::transposition_table::{TranspositionTable, BoundType};
use crate::core::types::MoveList;
use crate::core::board::Board;
use crate::movegen::ordering::OrderingContext;
use crate::core::config::evaluation::MATE_SCORE;

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
    board: &Board,
    depth: u32,
    alpha: i32,
    is_quiet: bool,
) -> bool {
    if !is_quiet || depth > 2 {
        return false;
    }

    // Futility pruning: at very shallow depths (near leaf), skip quiet moves
    // that are unlikely to raise alpha. This is a conservative heuristic.
    let margin = if depth == 1 { 150 } else { 80 };
    let stand_pat = crate::evaluation::eval::evaluate(board);
    stand_pat + margin <= alpha
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
    tt: &mut TranspositionTable,
    depth: u32,
    beta: i32,
    alpha: i32,
    moves_buf: &mut MoveList,
    ctx: &mut OrderingContext,
    ply: usize,
    current_hash: u64,
) -> Option<i32> {
    // Only apply when depth is sufficiently large, not in check, not zugzwang, and not near mate
    if depth < 3 || board.is_in_check(board.current_color())
       || is_zugzwang_position(board)
       || beta.abs() >= (MATE_SCORE - 100) {
        return None;
    }

    //  adaptive reduction: 4 + depth/6 + eval-beta bonus
    let eval = crate::evaluation::eval::evaluate(board);
    let eval_beta_bonus = if eval >= beta { 1 } else { 0 };
    let r = 4u32.saturating_add(depth / 6).saturating_add(eval_beta_bonus);
    let r = r.min(depth.saturating_sub(1)); // Ensure we don't reduce below 1 ply

    let null_info = board.make_null_move();
    let null_score = -crate::search::algorithms::negamax(board, tt, depth - 1 - r, -beta, -beta + 1, moves_buf, ctx, ply);
    board.unmake_null_move(null_info);

    if null_score >= beta {
        // Verification search at the same reduced depth to confirm cutoff
        let verify_score = -crate::search::algorithms::negamax(board, tt, depth - 1 - r, -beta, -alpha, moves_buf, ctx, ply);
        if verify_score >= beta {
            tt.store(current_hash, depth, verify_score, BoundType::LowerBound, None);
            return Some(verify_score);
        }
        // Otherwise, fall through and search normally (no premature cutoff).
    }

    None
}