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
            let color_idx = color.index();
            let our_attacks = ctx.all_attacks(color);
            let their_attacks = ctx.all_attacks(color.opponent());
            let their_pawn_attacks = ctx.pawn_attacks(color.opponent());

            for piece in Piece::NON_KING {
                let our_pieces = self.pieces[color_idx][piece.index()];

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
            let our_idx = color.index();
            let their_idx = color.opponent().index();

            let our_bishops = self.pieces[our_idx][Piece::Bishop.index()];
            let our_knights = self.pieces[our_idx][Piece::Knight.index()];
            let their_bishops = self.pieces[their_idx][Piece::Bishop.index()];
            let their_knights = self.pieces[their_idx][Piece::Knight.index()];

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
