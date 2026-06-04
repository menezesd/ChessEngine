use crate::board::{Move, Piece, EMPTY_MOVE, MAX_PLY};
use crate::tt::BoundType;

use super::super::constants::TT_MOVE_SCORE;
use super::{MoveContext, NodeContext, SimpleSearchContext, StagedMoveResult};

fn store_bound(raised_alpha: bool) -> BoundType {
    if raised_alpha {
        BoundType::Exact
    } else {
        BoundType::UpperBound
    }
}

fn tt_cutoff_score(
    bound: BoundType,
    score: i32,
    alpha: i32,
    beta: i32,
    is_pv: bool,
) -> Option<i32> {
    match bound {
        BoundType::Exact => {
            if !is_pv || (score > alpha && score < beta) {
                Some(score)
            } else {
                None
            }
        }
        BoundType::LowerBound => {
            if score >= beta {
                Some(score)
            } else {
                None
            }
        }
        BoundType::UpperBound => {
            if score <= alpha {
                Some(score)
            } else {
                None
            }
        }
    }
}

impl SimpleSearchContext<'_> {
    /// Store position in transposition table.
    pub(super) fn store_tt(&mut self, depth: u32, score: i32, raised_alpha: bool, best_move: Move) {
        if self.should_stop() || best_move == EMPTY_MOVE {
            return;
        }
        self.state.tables.tt.store(
            self.board.hash,
            depth,
            score,
            store_bound(raised_alpha),
            Some(best_move),
            self.state.generation,
        );
    }

    /// Probe TT and check for cutoff.
    /// Returns (`tt_move`, `tt_score`, `tt_bound`, `Option<cutoff_score>`).
    pub(super) fn probe_tt_for_cutoff(
        &self,
        depth: u32,
        alpha: i32,
        beta: i32,
        is_pv: bool,
        excluded_move_active: bool,
    ) -> (Move, i32, BoundType, Option<i32>) {
        let Some(entry) = self.state.tables.tt.probe(self.board.hash) else {
            return (EMPTY_MOVE, 0, BoundType::Exact, None);
        };

        let tt_move = entry.best_move().unwrap_or(EMPTY_MOVE);
        let tt_score = entry.score();
        let tt_bound = entry.bound_type();

        if !excluded_move_active && entry.depth() >= depth && !self.is_repetition() {
            let cutoff = tt_cutoff_score(tt_bound, tt_score, alpha, beta, is_pv);
            return (tt_move, tt_score, tt_bound, cutoff);
        }

        (tt_move, tt_score, tt_bound, None)
    }

    /// Try the TT move before generating all moves.
    ///
    /// Returns `Some` if TT move was legal and searched. If the score is at
    /// least beta, caller should return immediately with a beta cutoff.
    pub(super) fn try_tt_move_first(
        &mut self,
        tt_move: Move,
        node: &NodeContext,
        depth: u32,
        alpha: i32,
        beta: i32,
    ) -> Option<StagedMoveResult> {
        if !self.board.is_legal_move(tt_move) {
            return None;
        }

        let ply = node.ply;
        let moving_piece = self.board.piece_at(tt_move.from()).map(|(_, p)| p);

        if let Some(piece) = moving_piece {
            self.update_accumulator_for_move(ply, tt_move, piece, self.board.side_to_move());
        }

        let info = self.board.make_move(tt_move);
        if moving_piece == Some(Piece::King) {
            self.init_accumulator(ply + 1);
        }

        self.state.tables.tt.prefetch(self.board.hash);
        let gives_check = self.board.is_in_check(self.board.side_to_move());

        if ply < MAX_PLY {
            self.previous_move[ply] = tt_move;
            self.previous_piece[ply] = moving_piece;
        }

        let move_ctx = MoveContext::new(tt_move, TT_MOVE_SCORE, gives_check, moving_piece);
        let extension = Self::compute_extensions(&move_ctx, node);
        let new_depth = depth.saturating_sub(1) + extension;

        let score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);

        self.board.unmake_move(tt_move, info);

        if self.should_stop() {
            return None;
        }

        if score >= beta {
            self.handle_beta_cutoff(tt_move, ply, depth, score, tt_move);
            return Some(StagedMoveResult {
                score,
                raised_alpha: true,
            });
        }

        Some(StagedMoveResult {
            score,
            raised_alpha: score > alpha,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{store_bound, tt_cutoff_score};
    use crate::tt::BoundType;

    #[test]
    fn store_bound_tracks_alpha_raise() {
        assert_eq!(store_bound(true), BoundType::Exact);
        assert_eq!(store_bound(false), BoundType::UpperBound);
    }

    #[test]
    fn exact_bound_cuts_non_pv_regardless_window() {
        assert_eq!(
            tt_cutoff_score(BoundType::Exact, 50, 100, 200, false),
            Some(50)
        );
    }

    #[test]
    fn exact_bound_cuts_pv_only_inside_window() {
        assert_eq!(
            tt_cutoff_score(BoundType::Exact, 150, 100, 200, true),
            Some(150)
        );
        assert_eq!(tt_cutoff_score(BoundType::Exact, 50, 100, 200, true), None);
        assert_eq!(tt_cutoff_score(BoundType::Exact, 250, 100, 200, true), None);
    }

    #[test]
    fn lower_and_upper_bounds_cut_on_window_edges() {
        assert_eq!(
            tt_cutoff_score(BoundType::LowerBound, 200, 100, 200, true),
            Some(200)
        );
        assert_eq!(
            tt_cutoff_score(BoundType::UpperBound, 100, 100, 200, true),
            Some(100)
        );
        assert_eq!(
            tt_cutoff_score(BoundType::LowerBound, 199, 100, 200, true),
            None
        );
        assert_eq!(
            tt_cutoff_score(BoundType::UpperBound, 101, 100, 200, true),
            None
        );
    }
}
