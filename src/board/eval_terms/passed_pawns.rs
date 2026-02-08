//! Passed pawn evaluation.
//!
//! Evaluates passed pawns with bonuses based on advancement and control of stop square.

use crate::board::masks::{
    fill_backward, relative_rank, FILES, PASSED_PAWN_BONUS_EG, PASSED_PAWN_BONUS_MG,
    PASSED_PAWN_MASK,
};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece, Square};

use super::helpers::AttackContext;
use super::tables::{ROOK_BEHIND_PASSER_EG, ROOK_BEHIND_PASSER_MG};

// Passed pawn multiplier constants
const PASSER_MULTIPLIER_BASE: i32 = 100;
const PASSER_CONTROL_BONUS: i32 = 33;
const PASSER_BLOCKED_PENALTY: i32 = 15;

impl Board {
    /// Check if a pawn at the given square is a passed pawn.
    /// A passed pawn has no enemy pawns ahead of it or on adjacent files.
    #[must_use]
    pub fn is_passed_pawn(&self, sq: Square, color: Color) -> bool {
        let enemy_pawns = self.opponent_pieces(color, Piece::Pawn);
        let pass_mask = PASSED_PAWN_MASK[color.index()][sq.as_index()];
        (pass_mask.0 & enemy_pawns.0) == 0
    }

    /// Evaluate passed pawns.
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_passed_pawns(&self) -> (i32, i32) {
        let ctx = self.compute_attack_context();
        self.eval_passed_pawns_with_context(&ctx)
    }

    /// Evaluate passed pawns using pre-computed attack context.
    pub(super) fn eval_passed_pawns_with_context(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let own_pawns = self.pieces_of(color, Piece::Pawn);
            let our_attacks = ctx.all_attacks(color);
            let their_attacks = ctx.all_attacks(color.opponent());

            for sq in own_pawns.iter() {
                if !self.is_passed_pawn(sq, color) {
                    continue;
                }

                let rank = sq.rank();
                let rel_rank = relative_rank(rank, color);
                let mut multiplier = PASSER_MULTIPLIER_BASE;

                // Calculate stop square (square immediately ahead of pawn)
                let stop_sq = match color {
                    Color::White if rank < 7 => Square::new(rank + 1, sq.file()),
                    Color::Black if rank > 0 => Square::new(rank - 1, sq.file()),
                    _ => sq,
                };
                let stop_bb = Bitboard::from_square(stop_sq);

                // Adjust multiplier based on stop square control
                if (stop_bb.0 & our_attacks.0) != 0 {
                    multiplier += PASSER_CONTROL_BONUS;
                }
                if (stop_bb.0 & their_attacks.0) != 0 {
                    multiplier -= PASSER_CONTROL_BONUS;
                }
                if (stop_bb.0 & self.all_occupied.0) != 0 {
                    multiplier -= PASSER_BLOCKED_PENALTY;
                }

                let base_mg = PASSED_PAWN_BONUS_MG[rel_rank];
                let base_eg = PASSED_PAWN_BONUS_EG[rel_rank];

                mg += sign * (base_mg * multiplier / PASSER_MULTIPLIER_BASE);
                eg += sign * (base_eg * multiplier / PASSER_MULTIPLIER_BASE);

                // Rook behind passed pawn bonus
                let file = sq.file();
                let file_mask = FILES[file];
                let our_rooks = self.pieces_of(color, Piece::Rook);
                let their_rooks = self.opponent_pieces(color, Piece::Rook);

                // Check if we have a rook behind (supporting) the passed pawn
                let behind_mask =
                    Bitboard(fill_backward(Bitboard::from_square(sq), color).0 & file_mask.0);

                if (our_rooks.0 & behind_mask.0) != 0 {
                    mg += sign * ROOK_BEHIND_PASSER_MG;
                    eg += sign * ROOK_BEHIND_PASSER_EG;
                }

                // Penalty if enemy rook is behind our passed pawn (blocking)
                if (their_rooks.0 & behind_mask.0) != 0 {
                    mg -= sign * (ROOK_BEHIND_PASSER_MG / 2);
                    eg -= sign * (ROOK_BEHIND_PASSER_EG / 2);
                }
            }
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_passed_pawn() {
        // White pawn on e5, no black pawns on d/e/f files ahead
        let board: Board = "8/8/8/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
        let e5 = Square::new(4, 4); // rank 4 (0-indexed), file e
        assert!(board.is_passed_pawn(e5, Color::White));
    }

    #[test]
    fn test_is_not_passed_blocked() {
        // White pawn on e5, black pawn on e6 blocks it
        let board: Board = "8/8/4p3/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
        let e5 = Square::new(4, 4);
        assert!(!board.is_passed_pawn(e5, Color::White));
    }

    #[test]
    fn test_is_not_passed_adjacent() {
        // White pawn on e5, black pawn on d6 guards promotion path
        let board: Board = "8/8/3p4/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
        let e5 = Square::new(4, 4);
        assert!(!board.is_passed_pawn(e5, Color::White));
    }

    #[test]
    fn test_passed_pawn_bonus() {
        // White has passed pawn on 6th rank (very advanced)
        let board: Board = "8/4P3/8/8/8/8/8/8 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_passed_pawns();
        // Advanced passed pawn should have significant bonus
        assert!(mg > 0, "passed pawn mg should be positive: {mg}");
        assert!(eg > 0, "passed pawn eg should be positive: {eg}");
    }

    #[test]
    fn test_no_passed_pawns() {
        // Starting position - no passed pawns
        let board = Board::new();
        let (mg, eg) = board.eval_passed_pawns();
        assert_eq!(mg, 0, "no passed pawns in starting position");
        assert_eq!(eg, 0, "no passed pawns in starting position");
    }

    #[test]
    fn test_rook_behind_passer() {
        // White passed pawn on e5 with rook behind on e1
        let board: Board = "8/8/8/4P3/8/8/8/4R3 w - - 0 1".parse().unwrap();
        let (mg1, eg1) = board.eval_passed_pawns();

        // Compare to pawn without rook support
        let board2: Board = "8/8/8/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
        let (mg2, eg2) = board2.eval_passed_pawns();

        // Rook behind passer should give bonus
        assert!(mg1 > mg2, "rook behind passer should add bonus");
        assert!(eg1 > eg2, "rook behind passer should add eg bonus");
    }
}
