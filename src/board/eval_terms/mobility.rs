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
                let safe = moves & !enemy_pawn_attacks.0 & !our_pieces;
                let (mob_mg, mob_eg) =
                    mobility_score(safe, KNIGHT_MOB_MAX, &KNIGHT_MOB_MG, &KNIGHT_MOB_EG);
                mg += sign * mob_mg;
                eg += sign * mob_eg;
            }

            // Bishop mobility
            for sq_idx in self.pieces_of(color, Piece::Bishop).iter() {
                let moves = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
                let safe = moves & !enemy_pawn_attacks.0 & !our_pieces;
                let (mob_mg, mob_eg) =
                    mobility_score(safe, BISHOP_MOB_MAX, &BISHOP_MOB_MG, &BISHOP_MOB_EG);
                mg += sign * mob_mg;
                eg += sign * mob_eg;
            }

            // Rook mobility
            for sq_idx in self.pieces_of(color, Piece::Rook).iter() {
                let moves = slider_attacks(sq_idx.index(), self.all_occupied.0, false);
                let safe = moves & !our_pieces;
                let (mob_mg, mob_eg) =
                    mobility_score(safe, ROOK_MOB_MAX, &ROOK_MOB_MG, &ROOK_MOB_EG);
                mg += sign * mob_mg;
                eg += sign * mob_eg;
            }

            // Queen mobility
            for sq_idx in self.pieces_of(color, Piece::Queen).iter() {
                let moves = queen_attacks(sq_idx.index(), self.all_occupied.0);
                let safe = moves & !enemy_pawn_attacks.0 & !our_pieces;
                let (mob_mg, mob_eg) =
                    mobility_score(safe, QUEEN_MOB_MAX, &QUEEN_MOB_MG, &QUEEN_MOB_EG);
                mg += sign * mob_mg;
                eg += sign * mob_eg;
            }
        }

        (mg, eg)
    }
}

fn mobility_score(safe_moves: u64, max: usize, mg_table: &[i32], eg_table: &[i32]) -> (i32, i32) {
    let count = (safe_moves.count_ones() as usize).min(max);
    (mg_table[count], eg_table[count])
}

#[cfg(test)]
mod tests;
