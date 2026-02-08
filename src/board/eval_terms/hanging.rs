//! Hanging pieces evaluation.
//!
//! Evaluates pieces that are attacked but undefended, and minor piece attacks.

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Color, Piece};

use super::helpers::AttackContext;
use super::tables::{HANGING_PENALTY, MINOR_ON_MINOR};

impl Board {
    /// Evaluate hanging pieces (attacked and undefended).
    /// Returns score from white's perspective.
    #[must_use]
    pub fn eval_hanging(&self) -> i32 {
        let ctx = self.compute_attack_context();
        self.eval_hanging_with_context(&ctx)
    }

    /// Evaluate hanging pieces using pre-computed attack context.
    pub(super) fn eval_hanging_with_context(&self, ctx: &AttackContext) -> i32 {
        let mut score = 0;
        for color in Color::BOTH {
            let sign = color.sign();
            let our_attacks = ctx.all_attacks(color);
            let their_attacks = ctx.all_attacks(color.opponent());
            let their_pawn_attacks = ctx.pawn_attacks(color.opponent());

            for piece in Piece::NON_KING {
                let our_pieces = self.pieces_of(color, piece);

                for sq_idx in our_pieces.iter() {
                    let sq_bb = 1u64 << sq_idx.index();

                    let attacked_undefended =
                        (sq_bb & their_attacks.0) != 0 && (sq_bb & our_attacks.0) == 0;
                    let attacked_by_pawn = (sq_bb & their_pawn_attacks.0) != 0;

                    if attacked_undefended || attacked_by_pawn {
                        score -= sign * HANGING_PENALTY[piece.index()];
                    }
                }
            }
        }

        // Minor piece attacking minor piece
        for color in Color::BOTH {
            let sign = color.sign();

            let our_bishops = self.pieces_of(color, Piece::Bishop);
            let our_knights = self.pieces_of(color, Piece::Knight);
            let their_bishops = self.opponent_pieces(color, Piece::Bishop);
            let their_knights = self.opponent_pieces(color, Piece::Knight);

            // Our bishops attacking their knights
            for sq_idx in our_bishops.iter() {
                let attacks = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
                if (attacks & their_knights.0) != 0 {
                    score += sign * MINOR_ON_MINOR;
                }
            }

            // Our knights attacking their bishops
            for sq_idx in our_knights.iter() {
                let attacks = KNIGHT_ATTACKS[sq_idx.index()];
                if (attacks & their_bishops.0) != 0 {
                    score += sign * MINOR_ON_MINOR;
                }
            }
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_hanging_pieces() {
        // Starting position - all pieces defended
        let board = Board::new();
        let score = board.eval_hanging();
        // Should be roughly balanced
        assert!(score.abs() < 20, "no hanging pieces in start: {score}");
    }

    #[test]
    fn test_hanging_knight() {
        // White knight on e4 attacked by black bishop on b7
        let board: Board = "8/1b6/8/8/4N3/8/8/8 w - - 0 1".parse().unwrap();
        let score = board.eval_hanging();
        // Hanging knight should give penalty (negative for white)
        assert!(score < 0, "hanging knight should be penalized: {score}");
    }

    #[test]
    fn test_defended_piece_not_hanging() {
        // White knight on e4 defended by white pawn on d3
        let board: Board = "8/1b6/8/8/4N3/3P4/8/8 w - - 0 1".parse().unwrap();
        let score = board.eval_hanging();
        // Defended piece shouldn't have full hanging penalty
        // Score may still be slightly negative due to pawn attack
        assert!(
            score > -50,
            "defended piece should have less penalty: {score}"
        );
    }

    #[test]
    fn test_minor_attacking_minor() {
        // White bishop attacks black knight
        let board: Board = "8/8/4n3/8/8/8/6B1/8 w - - 0 1".parse().unwrap();
        let score = board.eval_hanging();
        // Minor attacking minor should give slight bonus
        assert!(score >= 0, "minor on minor should be non-negative: {score}");
    }

    #[test]
    fn test_pawn_attack_on_minor() {
        // Black pawn attacks white knight
        let board: Board = "8/8/3p4/4N3/8/8/8/8 w - - 0 1".parse().unwrap();
        let score = board.eval_hanging();
        // Pawn attacking knight is bad for knight's side
        assert!(score < 0, "pawn attacking knight should penalize: {score}");
    }
}
