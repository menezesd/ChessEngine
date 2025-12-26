//! Mobility evaluation.
//!
//! Evaluates piece mobility (number of safe squares available).

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Piece};

use super::helpers::AttackContext;
use super::tables::{
    BISHOP_MOB_EG, BISHOP_MOB_MG, KNIGHT_MOB_EG, KNIGHT_MOB_MG, QUEEN_MOB_EG, QUEEN_MOB_MG,
    ROOK_MOB_EG, ROOK_MOB_MG,
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

        for color_idx in 0..2 {
            let sign = if color_idx == 0 { 1 } else { -1 };
            let enemy_pawn_attacks = if color_idx == 0 {
                black_pawn_attacks
            } else {
                white_pawn_attacks
            };

            // Knight mobility
            for sq_idx in self.pieces[color_idx][Piece::Knight.index()].iter() {
                let moves = KNIGHT_ATTACKS[sq_idx.as_usize()];
                // Count safe squares (not attacked by enemy pawns, not blocked by own pieces)
                let safe = moves & !enemy_pawn_attacks.0 & !self.occupied[color_idx].0;
                let count = safe.count_ones() as usize;
                mg += sign * KNIGHT_MOB_MG[count.min(8)];
                eg += sign * KNIGHT_MOB_EG[count.min(8)];
            }

            // Bishop mobility
            for sq_idx in self.pieces[color_idx][Piece::Bishop.index()].iter() {
                let moves = slider_attacks(sq_idx.as_usize(), self.all_occupied.0, true);
                let safe = moves & !enemy_pawn_attacks.0 & !self.occupied[color_idx].0;
                let count = safe.count_ones() as usize;
                mg += sign * BISHOP_MOB_MG[count.min(13)];
                eg += sign * BISHOP_MOB_EG[count.min(13)];
            }

            // Rook mobility
            for sq_idx in self.pieces[color_idx][Piece::Rook.index()].iter() {
                let moves = slider_attacks(sq_idx.as_usize(), self.all_occupied.0, false);
                let safe = moves & !self.occupied[color_idx].0;
                let count = safe.count_ones() as usize;
                mg += sign * ROOK_MOB_MG[count.min(14)];
                eg += sign * ROOK_MOB_EG[count.min(14)];
            }

            // Queen mobility
            for sq_idx in self.pieces[color_idx][Piece::Queen.index()].iter() {
                let diag = slider_attacks(sq_idx.as_usize(), self.all_occupied.0, true);
                let straight = slider_attacks(sq_idx.as_usize(), self.all_occupied.0, false);
                let moves = diag | straight;
                let safe = moves & !enemy_pawn_attacks.0 & !self.occupied[color_idx].0;
                let count = safe.count_ones() as usize;
                mg += sign * QUEEN_MOB_MG[count.min(27)];
                eg += sign * QUEEN_MOB_EG[count.min(27)];
            }
        }

        (mg, eg)
    }
}
