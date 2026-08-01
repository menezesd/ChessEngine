//! Passed pawn evaluation.
//!
//! Evaluates passed pawns with bonuses based on advancement and control of stop square.

use crate::board::attack_tables::slider_attacks;
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

                // A rook behind the pawn helps only when the file between
                // them is clear. Merely sharing the file can be blocked by a
                // friendly piece, in which case it provides no support.
                let behind_mask =
                    Bitboard(fill_backward(Bitboard::from_square(sq), color).0 & file_mask.0);
                let clear_behind = Bitboard(
                    slider_attacks(sq.index(), self.all_occupied.0, false) & behind_mask.0,
                );

                if (our_rooks.0 & clear_behind.0) != 0 {
                    mg += sign * ROOK_BEHIND_PASSER_MG;
                    eg += sign * ROOK_BEHIND_PASSER_EG;
                }

                // Penalty if enemy rook is behind our passed pawn (blocking)
                if (their_rooks.0 & clear_behind.0) != 0 {
                    mg -= sign * (ROOK_BEHIND_PASSER_MG / 2);
                    eg -= sign * (ROOK_BEHIND_PASSER_EG / 2);
                }
            }
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests;
