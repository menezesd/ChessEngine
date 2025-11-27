use crate::core::board::Board;
use crate::search::search_context::SearchContext;

/// Late Move Reduction logic
pub struct LmrResult {
    pub should_reduce: bool,
    pub reduction: u32,
    pub reduced_score: i32,
}

impl LmrResult {
    pub fn no_reduction() -> Self {
        Self {
            should_reduce: false,
            reduction: 0,
            reduced_score: -i32::MAX,
        }
    }
}

/// Check if LMR should be applied to a move
pub fn should_apply_lmr(
    depth: u32,
    move_index: usize,
    is_capture: bool,
    is_promotion: bool,
) -> Option<u32> {
    // Determine if LMR is applicable: non-capture, no promotion, and depth >= 3
    if is_capture || is_promotion || depth < 3 {
        return None;
    }

    // Apply reductions only for sufficiently late moves
    if move_index >= 4 {
        // Log-based reduction formula 
        let log_reduction = ((depth as f32).ln() * (move_index as f32).ln()) as u32;
        let mut red = 1u32.saturating_add(log_reduction);
        if red > depth.saturating_sub(2) {
            red = depth.saturating_sub(2);
        }
        Some(red)
    } else {
        None
    }
}

/// Apply LMR and handle re-searches if needed
pub fn apply_lmr_and_research(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    depth: u32,
    extension: u32,
    alpha: i32,
    beta: i32,
    move_index: usize,
    is_capture: bool,
    is_promotion: bool,
) -> i32 {
    use crate::search::algorithms::negamax;

    let lmr_reduction = match should_apply_lmr(depth, move_index, is_capture, is_promotion) {
        Some(reduction) => reduction,
        None => return -negamax(board, s_ctx, depth - 1 + extension, -alpha - 1, -alpha),
    };

    // Apply LMR
    let reduced_depth = depth - 1 - lmr_reduction + extension;
    let reduced_score = -negamax(board, s_ctx, reduced_depth, -alpha - 1, -alpha);

    // If reduced search suggests the move might be interesting, do full null-window re-search
    if reduced_score > alpha {
        let null_window_score = -negamax(board, s_ctx, depth - 1 + extension, -alpha - 1, -alpha);
        if null_window_score > alpha && null_window_score < beta {
            // Full window re-search
            -negamax(board, s_ctx, depth - 1 + extension, -beta, -alpha)
        } else {
            null_window_score
        }
    } else {
        reduced_score
    }
}