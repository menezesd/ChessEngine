//! Mobility evaluation.
//!
//! Evaluates piece mobility (number of safe squares available).

use crate::board::attack_tables::{queen_attacks, slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;
use super::tables::{
    BISHOP_MOB_EG, BISHOP_MOB_MAX, BISHOP_MOB_MG, KNIGHT_MOB_EG, KNIGHT_MOB_MAX, KNIGHT_MOB_MG,
    QUEEN_MOB_EG, QUEEN_MOB_MAX, QUEEN_MOB_MG, ROOK_MOB_EG, ROOK_MOB_MAX, ROOK_MOB_MG,
};

impl Board {
    /// Evaluate mobility for all pieces.
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_mobility(&self) -> (i32, i32) {
        let ctx = self.compute_attack_context();
        self.eval_mobility_with_context(&ctx)
    }

    /// Evaluate mobility using pre-computed attack context.
    #[must_use]
    pub fn eval_mobility_with_context(&self, ctx: &AttackContext) -> (i32, i32) {
        self.eval_mobility_inner(ctx.white_pawn_attacks, ctx.black_pawn_attacks)
    }

    /// Internal mobility evaluation with pawn attacks.
    fn eval_mobility_inner(
        &self,
        white_pawn_attacks: Bitboard,
        black_pawn_attacks: Bitboard,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let pawn_attacks = [white_pawn_attacks, black_pawn_attacks];
        for color in Color::BOTH {
            let sign = color.sign();
            let enemy_pawn_attacks = pawn_attacks[color.opponent().index()];
            let our_pieces = self.occupied_by(color).0;

            // Knight mobility
            for sq_idx in self.pieces_of(color, Piece::Knight).iter() {
                let moves = KNIGHT_ATTACKS[sq_idx.index()];
                // Count safe squares (not attacked by enemy pawns, not blocked by own pieces)
                let safe = moves & !enemy_pawn_attacks.0 & !our_pieces;
                let count = safe.count_ones() as usize;
                mg += sign * KNIGHT_MOB_MG[count.min(KNIGHT_MOB_MAX)];
                eg += sign * KNIGHT_MOB_EG[count.min(KNIGHT_MOB_MAX)];
            }

            // Bishop mobility
            for sq_idx in self.pieces_of(color, Piece::Bishop).iter() {
                let moves = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
                let safe = moves & !enemy_pawn_attacks.0 & !our_pieces;
                let count = safe.count_ones() as usize;
                mg += sign * BISHOP_MOB_MG[count.min(BISHOP_MOB_MAX)];
                eg += sign * BISHOP_MOB_EG[count.min(BISHOP_MOB_MAX)];
            }

            // Rook mobility
            for sq_idx in self.pieces_of(color, Piece::Rook).iter() {
                let moves = slider_attacks(sq_idx.index(), self.all_occupied.0, false);
                let safe = moves & !our_pieces;
                let count = safe.count_ones() as usize;
                mg += sign * ROOK_MOB_MG[count.min(ROOK_MOB_MAX)];
                eg += sign * ROOK_MOB_EG[count.min(ROOK_MOB_MAX)];
            }

            // Queen mobility
            for sq_idx in self.pieces_of(color, Piece::Queen).iter() {
                let moves = queen_attacks(sq_idx.index(), self.all_occupied.0);
                let safe = moves & !enemy_pawn_attacks.0 & !our_pieces;
                let count = safe.count_ones() as usize;
                mg += sign * QUEEN_MOB_MG[count.min(QUEEN_MOB_MAX)];
                eg += sign * QUEEN_MOB_EG[count.min(QUEEN_MOB_MAX)];
            }
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobility_starting_position() {
        let board = Board::new();
        let (mg, eg) = board.eval_mobility();
        // Starting position should be roughly equal
        assert!(mg.abs() < 20, "mobility mg should be near 0: {mg}");
        assert!(eg.abs() < 20, "mobility eg should be near 0: {eg}");
    }

    #[test]
    fn test_knight_center_more_mobile() {
        // Knight in center (e4) has more squares
        let board1: Board = "8/8/8/8/4N3/8/8/8 w - - 0 1".parse().unwrap();
        let (mg1, _) = board1.eval_mobility();

        // Knight in corner (a1) has fewer squares
        let board2: Board = "8/8/8/8/8/8/8/N7 w - - 0 1".parse().unwrap();
        let (mg2, _) = board2.eval_mobility();

        assert!(mg1 > mg2, "center knight should have more mobility");
    }

    #[test]
    fn test_bishop_on_open_diagonal() {
        // Bishop on long diagonal with few blockers
        let board: Board = "8/8/8/8/8/8/8/B7 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_mobility();
        // Open bishop should have positive mobility
        assert!(mg > 0, "open bishop should have positive mobility: {mg}");
        assert!(eg > 0, "open bishop should have positive eg mobility: {eg}");
    }

    #[test]
    fn test_rook_more_mobile_on_open_board() {
        // Rook on empty board
        let board1: Board = "8/8/8/8/4R3/8/8/8 w - - 0 1".parse().unwrap();
        let (mg1, _) = board1.eval_mobility();

        // Rook blocked by pawns
        let board2: Board = "8/8/8/4P3/4R3/4P3/8/8 w - - 0 1".parse().unwrap();
        let (mg2, _) = board2.eval_mobility();

        assert!(mg1 > mg2, "rook on open board should have more mobility");
    }

    #[test]
    fn test_queen_mobility() {
        // Queen in center with open lines
        let board: Board = "8/8/8/8/4Q3/8/8/8 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_mobility();
        // Queen should have high mobility score
        assert!(mg > 0, "queen should have positive mobility: {mg}");
        assert!(eg > 0, "queen should have positive eg mobility: {eg}");
    }
}
