use std::sync::atomic::Ordering;

use crate::board::{Move, Piece, MAX_PLY};
use crate::tt::BoundType;

use super::super::constants::{PAWN_EXTENSION_RANK_BLACK, PAWN_EXTENSION_RANK_WHITE};
use super::SimpleSearchContext;

const CHECK_EXTENSION: u32 = 1;
const PAWN_NEAR_PROMOTION_EXTENSION: u32 = 1;
const FUTILITY_PRUNING_MAX_DEPTH: u32 = 6;
const MIN_MOVES_BEFORE_FUTILITY: usize = 1;
const STOP_CHECK_NODE_INTERVAL_LOG2: u32 = 10;
const REPETITION_THRESHOLD: u32 = 1;
const IMPROVING_LOOKBACK_PLY: usize = 2;

fn futility_pruning_score(static_eval: i32, futility_margin: i32, depth: u32) -> i32 {
    let margin = i64::from(futility_margin) * i64::from(depth);
    let score = i64::from(static_eval) + margin;
    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn lmp_threshold(move_limit: usize, depth: u32) -> usize {
    let depth = depth as usize;
    move_limit.saturating_add(depth.saturating_mul(depth))
}

#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct NodeContext {
    pub(super) ply: usize,
    pub(super) is_pv: bool,
    pub(super) in_check: bool,
    pub(super) improving: bool,
    pub(super) excluded_move: Move,
    pub(super) tt_move: Move,
    pub(super) tt_score: i32,
    pub(super) tt_bound: BoundType,
    /// Extension for the TT move (from singular extension search).
    pub(super) singular_extension: u32,
}

/// Context for a single move being searched.
pub(crate) struct MoveContext {
    pub(super) m: Move,
    pub(super) move_score: i32,
    pub(super) is_quiet: bool,
    pub(super) gives_check: bool,
    pub(super) moving_piece: Option<Piece>,
}

impl MoveContext {
    pub(super) fn new(
        m: Move,
        move_score: i32,
        gives_check: bool,
        moving_piece: Option<Piece>,
    ) -> Self {
        Self {
            m,
            move_score,
            is_quiet: !m.is_capture() && !m.is_promotion(),
            gives_check,
            moving_piece,
        }
    }
}

/// Result of trying TT move before full move generation.
pub(crate) struct StagedMoveResult {
    /// Score from searching the TT move.
    pub(super) score: i32,
    /// Whether TT move raised alpha (but didn't cause cutoff).
    pub(super) raised_alpha: bool,
}

impl SimpleSearchContext<'_> {
    /// Compute extensions for a move.
    pub(super) fn compute_extensions(ctx: &MoveContext, node: &NodeContext) -> u32 {
        let mut extension = 0u32;

        if ctx.gives_check {
            extension += CHECK_EXTENSION;
        }

        if ctx.m == node.tt_move && node.singular_extension > 0 {
            extension += node.singular_extension;
        }

        if extension == 0 && !ctx.m.is_promotion() {
            if let Some(Piece::Pawn) = ctx.moving_piece {
                let to_rank = ctx.m.to().rank();
                if to_rank == PAWN_EXTENSION_RANK_WHITE || to_rank == PAWN_EXTENSION_RANK_BLACK {
                    extension += PAWN_NEAR_PROMOTION_EXTENSION;
                }
            }
        }

        extension
    }

    /// Check if a quiet move should be pruned (futility pruning or LMP).
    pub(super) fn should_prune_quiet(
        &self,
        ctx: &MoveContext,
        node: &NodeContext,
        depth: u32,
        moves_tried: usize,
        alpha: i32,
    ) -> bool {
        if !ctx.is_quiet || node.in_check || ctx.gives_check || node.is_pv {
            return false;
        }

        if depth <= FUTILITY_PRUNING_MAX_DEPTH && moves_tried > MIN_MOVES_BEFORE_FUTILITY {
            let static_eval = if node.ply < MAX_PLY {
                self.static_eval[node.ply]
            } else {
                0
            };
            if futility_pruning_score(static_eval, self.state.params.futility_margin, depth)
                <= alpha
            {
                return true;
            }
        }

        if depth <= self.state.params.lmp_min_depth
            && moves_tried > lmp_threshold(self.state.params.lmp_move_limit, depth)
        {
            return true;
        }

        false
    }

    /// Check if we should stop searching.
    #[inline]
    pub(super) fn should_stop(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        if self.node_limit > 0 && self.nodes >= self.node_limit {
            return true;
        }
        if self.time_limit_ms > 0 && self.nodes.trailing_zeros() >= STOP_CHECK_NODE_INTERVAL_LOG2 {
            let elapsed = self.elapsed_ms();
            if elapsed >= self.time_limit_ms {
                return true;
            }
        }

        false
    }

    /// Check for repetition.
    #[inline]
    pub(super) fn is_repetition(&self) -> bool {
        self.board.repetition_counts.get(self.board.hash) > REPETITION_THRESHOLD
    }

    /// Check if the position is improving (eval better than 2 plies ago).
    #[inline]
    pub(super) fn is_improving(&self, ply: usize, eval: i32) -> bool {
        if (IMPROVING_LOOKBACK_PLY..MAX_PLY).contains(&ply) {
            eval > self.static_eval[ply - IMPROVING_LOOKBACK_PLY]
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{futility_pruning_score, lmp_threshold};

    #[test]
    fn futility_pruning_score_saturates_extreme_inputs() {
        assert_eq!(
            futility_pruning_score(i32::MAX, i32::MAX, u32::MAX),
            i32::MAX
        );
        assert_eq!(
            futility_pruning_score(i32::MIN, i32::MIN, u32::MAX),
            i32::MIN
        );
    }

    #[test]
    fn lmp_threshold_saturates_extreme_inputs() {
        assert_eq!(lmp_threshold(usize::MAX, u32::MAX), usize::MAX);
    }
}
