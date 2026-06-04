use super::super::constants::SCORE_NEAR_MATE;
use super::{NodeContext, SimpleSearchContext};
use crate::board::Piece;

const NULL_MOVE_MIN_DEPTH: u32 = 3;
const NULL_MOVE_EVAL_MARGIN: i32 = 20;
const NULL_MOVE_DEPTH_DIVISOR: u32 = 3;
const PROBCUT_MIN_DEPTH: u32 = 8;
const PROBCUT_MARGIN: i32 = 350;
const PROBCUT_DEPTH_REDUCTION: u32 = 5;
const REVERSE_FUTILITY_MAX_DEPTH: u32 = 7;

fn null_move_reduction(depth: u32) -> u32 {
    super::super::constants::NULL_MOVE_BASE_REDUCTION + (depth + 1) / NULL_MOVE_DEPTH_DIVISOR
}

fn null_window_beta(beta: i32) -> i32 {
    beta.saturating_sub(1)
}

fn probcut_beta(beta: i32) -> i32 {
    beta.saturating_add(PROBCUT_MARGIN)
}

fn reverse_futility_score(eval: i32, margin: i32, depth: u32) -> i32 {
    let margin = i64::from(margin) * i64::from(depth);
    let score = i64::from(eval) - margin;
    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl SimpleSearchContext<'_> {
    /// Try null move pruning with verification
    pub(super) fn try_null_move_pruning(
        &mut self,
        depth: u32,
        beta: i32,
        eval: i32,
        node: &NodeContext,
    ) -> Option<i32> {
        let dominated_phase = self.board.game_phase[self.board.side_to_move().index()];

        // Don't do null move in check, with no pieces, at root, or when eval is too low
        // Allow null move when eval is slightly below beta (more aggressive)
        if node.in_check
            || dominated_phase == 0
            || depth < NULL_MOVE_MIN_DEPTH
            || depth >= self.initial_depth
            || node.ply == 0
            || eval < beta - NULL_MOVE_EVAL_MARGIN
        {
            return None;
        }

        let r = null_move_reduction(depth);
        let reduced_depth = depth.saturating_sub(r);

        self.copy_accumulator_for_null_move(node.ply);
        let info = self.board.make_null_move();
        let score = -self.alphabeta(
            reduced_depth,
            -beta,
            null_window_beta(-beta),
            false,
            node.ply + 1,
            crate::board::EMPTY_MOVE,
        );
        self.board.unmake_null_move(info);

        if self.should_stop() {
            return None;
        }

        if score >= beta {
            return Some(beta);
        }

        None
    }

    /// `ProbCut`: If a shallow search on good captures suggests we'll beat beta
    /// by a large margin, prune this node. Based on the idea that if a capture
    /// refutes the position at reduced depth, it will likely refute at full depth.
    pub(super) fn try_probcut(&mut self, depth: u32, beta: i32, node: &NodeContext) -> Option<i32> {
        // Very conservative: only at high depths, not in check
        // High margin to avoid pruning tactical positions
        if depth < PROBCUT_MIN_DEPTH || node.in_check || beta.abs() > SCORE_NEAR_MATE {
            return None;
        }

        let probcut_beta = probcut_beta(beta);
        let probcut_depth = depth.saturating_sub(PROBCUT_DEPTH_REDUCTION);

        // Generate captures and promotions
        let captures = self.board.generate_tactical_moves();

        for m in &captures {
            // Only consider good captures (positive SEE)
            if self.board.see(m.from(), m.to()) < 0 {
                continue;
            }

            // Update NNUE accumulator before make_move
            let moving_piece = self.board.piece_at(m.from()).map(|(_, piece)| piece);
            if let Some(piece) = moving_piece {
                self.update_accumulator_for_move(node.ply, *m, piece, self.board.side_to_move());
            }

            let info = self.board.make_move(*m);
            if moving_piece == Some(Piece::King) {
                self.init_accumulator(node.ply + 1);
            }

            // Do a reduced search at probcut_beta
            let score = -self.alphabeta(
                probcut_depth,
                -probcut_beta,
                null_window_beta(-probcut_beta),
                false,
                node.ply + 1,
                crate::board::EMPTY_MOVE,
            );

            self.board.unmake_move(*m, info);

            if self.should_stop() {
                return None;
            }

            if score >= probcut_beta {
                return Some(score);
            }
        }

        None
    }

    /// Reverse futility pruning (RFP) / Static null move pruning.
    /// If static eval is significantly better than beta, we assume this node
    /// will fail high and we can prune it.
    pub(super) fn try_reverse_futility_pruning(
        &self,
        depth: u32,
        beta: i32,
        eval: i32,
    ) -> Option<i32> {
        if depth > REVERSE_FUTILITY_MAX_DEPTH {
            return None;
        }

        if reverse_futility_score(eval, self.state.params.rfp_margin, depth) >= beta {
            return Some(beta);
        }

        None
    }

    /// Run static/null-move pruning that can exit before generating moves.
    pub(super) fn prune_before_move_loop(
        &mut self,
        depth: u32,
        _alpha: i32,
        beta: i32,
        eval: i32,
        node: &NodeContext,
        allow_null: bool,
    ) -> Option<i32> {
        if node.is_pv || node.in_check || node.excluded_move != crate::board::EMPTY_MOVE {
            return None;
        }

        // Reverse futility pruning (static null move)
        if let Some(score) = self.try_reverse_futility_pruning(depth, beta, eval) {
            return Some(score);
        }

        // Null move pruning
        if allow_null {
            if let Some(score) = self.try_null_move_pruning(depth, beta, eval, node) {
                return Some(score);
            }
        }

        // ProbCut: reduced search on good captures (conservative settings)
        if let Some(score) = self.try_probcut(depth, beta, node) {
            return Some(score);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::{null_move_reduction, null_window_beta, probcut_beta, reverse_futility_score};
    use crate::board::search::constants::NULL_MOVE_BASE_REDUCTION;

    #[test]
    fn null_move_reduction_scales_by_depth() {
        assert_eq!(null_move_reduction(3), NULL_MOVE_BASE_REDUCTION + 1);
        assert_eq!(null_move_reduction(8), NULL_MOVE_BASE_REDUCTION + 3);
    }

    #[test]
    fn null_window_beta_saturates_at_i32_min() {
        assert_eq!(null_window_beta(i32::MIN), i32::MIN);
        assert_eq!(null_window_beta(10), 9);
    }

    #[test]
    fn probcut_beta_saturates_at_i32_max() {
        assert_eq!(probcut_beta(i32::MAX), i32::MAX);
        assert_eq!(probcut_beta(10), 360);
    }

    #[test]
    fn reverse_futility_score_saturates_extreme_inputs() {
        assert_eq!(
            reverse_futility_score(i32::MIN, i32::MAX, u32::MAX),
            i32::MIN
        );
        assert_eq!(
            reverse_futility_score(i32::MAX, i32::MIN, u32::MAX),
            i32::MAX
        );
    }
}
