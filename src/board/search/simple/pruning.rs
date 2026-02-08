use super::super::constants::SCORE_NEAR_MATE;
use super::{NodeContext, SimpleSearchContext};

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
            || depth <= 2
            || depth >= self.initial_depth
            || node.ply == 0
            || eval < beta - 20
        {
            return None;
        }

        let r = super::super::constants::NULL_MOVE_BASE_REDUCTION + (depth + 1) / 3;
        let reduced_depth = depth.saturating_sub(r);

        let info = self.board.make_null_move();
        let score = -self.alphabeta(
            reduced_depth,
            -beta,
            -beta + 1,
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
        if depth < 8 || node.in_check || beta.abs() > SCORE_NEAR_MATE {
            return None;
        }

        let probcut_beta = beta + 350;
        let probcut_depth = depth.saturating_sub(5);

        // Generate captures and promotions
        let captures = self.board.generate_tactical_moves();

        for m in &captures {
            // Only consider good captures (positive SEE)
            if self.board.see(m.from(), m.to()) < 0 {
                continue;
            }

            let info = self.board.make_move(*m);

            // Do a reduced search at probcut_beta
            let score = -self.alphabeta(
                probcut_depth,
                -probcut_beta,
                -probcut_beta + 1,
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
        if depth >= 8 {
            return None;
        }

        let margin = self.state.params.rfp_margin * depth as i32;
        if eval - margin >= beta {
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
