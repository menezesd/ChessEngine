use super::super::constants::{MATE_THRESHOLD, SCORE_NEAR_MATE};
use super::{NodeContext, SimpleSearchContext, FIFTY_MOVE_RULE_HALFMOVES};
use crate::board::{MoveList, EMPTY_MOVE};

const RFP_MIN_DEPTH: u32 = 4;
const PROBCUT_MIN_DEPTH: u32 = 8;

#[inline]
fn null_move_reduction(configured_reduction: u32, depth: u32) -> u32 {
    configured_reduction.saturating_add((depth + 1) / 3)
}

impl SimpleSearchContext<'_> {
    /// Try null move pruning with a reduced search after a synthetic pass.
    fn try_null_move_pruning(&mut self, reduced_depth: u32, beta: i32, ply: usize) -> Option<i32> {
        self.copy_accumulator_for_null_move(ply);
        self.previous_move[ply] = EMPTY_MOVE;
        self.previous_piece[ply] = None;
        let previous_floor = self.line.repetition_floor;
        // The pass and its ancestors are outside the legal line searched
        // after it. Repetition can resume once real moves have been played.
        self.line.repetition_floor = ply + 2;
        let info = self.board.make_null_move();
        let score = -self.alphabeta(reduced_depth, -beta, -beta + 1, false, ply + 1, EMPTY_MOVE);
        self.board.unmake_null_move(info);
        self.line.repetition_floor = previous_floor;

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
    fn try_probcut(&mut self, depth: u32, beta: i32, node: &NodeContext) -> Option<i32> {
        let probcut_beta = beta + 350;
        let probcut_depth = depth.saturating_sub(5);

        // Generate captures and promotions
        let mut captures = MoveList::new();
        self.board.generate_tactical_moves_into(&mut captures);

        for m in &captures {
            // The coordinate-based SEE assumes a queen promotion. Keep all
            // promotion choices, including quiet promotions and underpromotions.
            if !m.is_promotion() && self.board.see(m.from(), m.to()) < 0 {
                continue;
            }

            let moving_piece = self.board.piece_at(m.from()).map(|(_, piece)| piece);
            self.previous_move[node.ply] = *m;
            self.previous_piece[node.ply] = moving_piece;
            let info = self.board.make_search_move(*m);
            if let Some(piece) = moving_piece {
                self.update_accumulator_for_move(node.ply, *m, piece, info.captured_piece_info);
            }

            // Do a reduced search at probcut_beta
            let score = -self.alphabeta(
                probcut_depth,
                -probcut_beta,
                -probcut_beta + 1,
                false,
                node.ply + 1,
                EMPTY_MOVE,
            );

            self.board.unmake_move(*m, info);

            if self.should_stop() {
                return None;
            }

            if score >= probcut_beta {
                // A speculative cutoff only establishes this threshold
                // for the full-depth search, even if the probe saw a mate.
                return Some(if score >= MATE_THRESHOLD {
                    probcut_beta
                } else {
                    score
                });
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
        // Very shallow static evals are too noisy for a hard cutoff. In
        // particular, they can hide short forcing mates in a null-window
        // search and prevent the later root move from receiving its PVS
        // re-search.
        if !(RFP_MIN_DEPTH..PROBCUT_MIN_DEPTH).contains(&depth) {
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
        beta: i32,
        eval: i32,
        node: &NodeContext,
        allow_null: bool,
    ) -> Option<i32> {
        if node.is_pv
            || node.in_check
            || node.excluded_move != EMPTY_MOVE
            || node.ply == 1
            || beta.abs() >= SCORE_NEAR_MATE
            || self.board.halfmove_clock().saturating_add(depth) >= FIFTY_MOVE_RULE_HALFMOVES
        {
            // A null-window search directly below the root decides whether a
            // later root move gets a full-window PVS re-search.  A speculative
            // fail-high here can therefore hide the best root move entirely.
            // Search those nodes normally; the deeper tree still gets the
            // usual RFP/null-move/ProbCut pruning.
            // Material estimates cannot prove a mate or an escape from it.
            // Once the horizon reaches the fifty-move limit, static and
            // null-move estimates can hide a forced draw. Search real moves
            // so captures/pawn moves and mating exceptions are considered.
            return None;
        }

        let reverse_futility = self.try_reverse_futility_pruning(depth, beta, eval);
        let null_candidate = allow_null
            && depth >= self.state.params.null_min_depth
            && depth < self.initial_depth
            && node.ply != 0
            && eval >= beta - 20
            && self.board.eval.phase[self.board.side_to_move().index()] != 0;
        let probcut_candidate = depth >= PROBCUT_MIN_DEPTH;
        // Verify a candidate cutoff can neither hide stalemate nor skip the
        // sole-reply extension. Skip move generation when no cutoff applies.
        if !(reverse_futility.is_some() || null_candidate || probcut_candidate)
            || !self.board.has_multiple_legal_moves()
        {
            return None;
        }

        if reverse_futility.is_some() {
            return reverse_futility;
        }
        if null_candidate {
            let reduction = null_move_reduction(self.state.params.null_reduction, depth);
            if let Some(score) =
                self.try_null_move_pruning(depth.saturating_sub(reduction), beta, node.ply)
            {
                return Some(score);
            }
        }

        probcut_candidate
            .then(|| self.try_probcut(depth, beta, node))
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use crate::board::{Board, SearchState, EMPTY_MOVE};

    use super::{null_move_reduction, NodeContext};

    #[test]
    fn null_move_reduction_uses_configured_base() {
        assert_eq!(null_move_reduction(1, 3), 2);
        assert_eq!(null_move_reduction(4, 9), 7);
    }

    #[test]
    fn probcut_returns_a_bound_instead_of_a_mate_distance() {
        let mut board = Board::from_fen("7k/5K1p/6p1/4N3/8/8/8/7R w - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        let node = NodeContext {
            ply: 2,
            raw_eval: 0,
            is_pv: false,
            in_check: false,
            improving: false,
            excluded_move: EMPTY_MOVE,
            tt_move: EMPTY_MOVE,
            singular_extension: 0,
        };
        assert_eq!(ctx.try_probcut(8, 5_000, &node), Some(5_350));
    }
}
