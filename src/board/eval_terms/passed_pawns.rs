//! Passed pawn evaluation.
//!
//! Evaluates passed pawns with bonuses based on advancement and control of stop square.

use crate::board::masks::{
    fill_north, fill_south, relative_rank, FILES, PASSED_PAWN_BONUS_EG, PASSED_PAWN_BONUS_MG,
    PASSED_PAWN_MASK,
};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece, Square};

use super::helpers::AttackContext;
use super::tables::{ROOK_BEHIND_PASSER_EG, ROOK_BEHIND_PASSER_MG};

impl Board {
    /// Check if a pawn at the given square is a passed pawn.
    /// A passed pawn has no enemy pawns ahead of it or on adjacent files.
    #[must_use]
    pub fn is_passed_pawn(&self, sq: Square, color: Color) -> bool {
        let color_idx = color.index();
        let enemy_pawns = self.pieces[color.opponent().index()][Piece::Pawn.index()];
        let pass_mask = PASSED_PAWN_MASK[color_idx][sq.as_index()];
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
            let color_idx = color.index();
            let own_pawns = self.pieces[color_idx][Piece::Pawn.index()];
            let enemy_pawns = self.pieces[color.opponent().index()][Piece::Pawn.index()];
            let our_attacks = ctx.all_attacks(color);
            let their_attacks = ctx.all_attacks(color.opponent());

            for sq_idx in own_pawns.iter() {
                let sq = sq_idx;
                let rank = sq.rank();
                let rel_rank = relative_rank(rank, color);

                let pass_mask = PASSED_PAWN_MASK[color_idx][sq.as_index()];
                if (pass_mask.0 & enemy_pawns.0) == 0 {
                    let mut multiplier = 100i32;

                    let stop_sq = match color {
                        Color::White => {
                            if rank < 7 {
                                Square::new(rank + 1, sq.file())
                            } else {
                                sq
                            }
                        }
                        Color::Black => {
                            if rank > 0 {
                                Square::new(rank - 1, sq.file())
                            } else {
                                sq
                            }
                        }
                    };
                    let stop_bb = Bitboard::from_square(stop_sq);

                    if (stop_bb.0 & our_attacks.0) != 0 {
                        multiplier += 33;
                    }
                    if (stop_bb.0 & their_attacks.0) != 0 {
                        multiplier -= 33;
                    }
                    if (stop_bb.0 & self.all_occupied.0) != 0 {
                        multiplier -= 15;
                    }

                    let base_mg = PASSED_PAWN_BONUS_MG[rel_rank];
                    let base_eg = PASSED_PAWN_BONUS_EG[rel_rank];

                    mg += sign * (base_mg * multiplier / 100);
                    eg += sign * (base_eg * multiplier / 100);

                    // Rook behind passed pawn bonus
                    let file = sq.file();
                    let file_mask = FILES[file];
                    let our_rooks = self.pieces[color_idx][Piece::Rook.index()];
                    let their_rooks = self.pieces[color.opponent().index()][Piece::Rook.index()];

                    // Check if we have a rook behind (supporting) the passed pawn
                    let behind_mask = match color {
                        Color::White => {
                            Bitboard(fill_south(Bitboard::from_square(sq).0) & file_mask.0)
                        }
                        Color::Black => {
                            Bitboard(fill_north(Bitboard::from_square(sq).0) & file_mask.0)
                        }
                    };

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
        }

        (mg, eg)
    }
}
