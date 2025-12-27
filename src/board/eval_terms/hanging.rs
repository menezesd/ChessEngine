//! Hanging pieces evaluation.
//!
//! Evaluates pieces that are attacked but undefended, and minor piece attacks.

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::tables::{HANGING_PENALTY, MINOR_ON_MINOR};

impl Board {
    /// Evaluate hanging pieces (attacked and undefended).
    /// Returns score from white's perspective.
    #[must_use]
    pub fn eval_hanging(&self) -> i32 {
        let white_attacks = self.all_attacks(Color::White);
        let black_attacks = self.all_attacks(Color::Black);
        self.eval_hanging_with_attacks(white_attacks, black_attacks)
    }

    /// Evaluate hanging pieces using pre-computed attacks (avoids recomputation).
    pub(super) fn eval_hanging_with_attacks(
        &self,
        white_attacks: Bitboard,
        black_attacks: Bitboard,
    ) -> i32 {
        let mut score = 0;

        for color_idx in 0..2 {
            let sign = if color_idx == 0 { 1 } else { -1 };
            let our_attacks = if color_idx == 0 {
                white_attacks
            } else {
                black_attacks
            };
            let their_attacks = if color_idx == 0 {
                black_attacks
            } else {
                white_attacks
            };

            let their_pawn_attacks = self.pawn_attacks(if color_idx == 0 {
                Color::Black
            } else {
                Color::White
            });

            for piece in [
                Piece::Pawn,
                Piece::Knight,
                Piece::Bishop,
                Piece::Rook,
                Piece::Queen,
            ] {
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
        let white_knights = self.pieces[0][Piece::Knight.index()];
        let white_bishops = self.pieces[0][Piece::Bishop.index()];
        let black_knights = self.pieces[1][Piece::Knight.index()];
        let black_bishops = self.pieces[1][Piece::Bishop.index()];

        for sq_idx in white_bishops.iter() {
            let attacks = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
            if (attacks & black_knights.0) != 0 {
                score += MINOR_ON_MINOR;
            }
        }

        for sq_idx in white_knights.iter() {
            let attacks = KNIGHT_ATTACKS[sq_idx.index()];
            if (attacks & black_bishops.0) != 0 {
                score += MINOR_ON_MINOR;
            }
        }

        for sq_idx in black_bishops.iter() {
            let attacks = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
            if (attacks & white_knights.0) != 0 {
                score -= MINOR_ON_MINOR;
            }
        }

        for sq_idx in black_knights.iter() {
            let attacks = KNIGHT_ATTACKS[sq_idx.index()];
            if (attacks & white_bishops.0) != 0 {
                score -= MINOR_ON_MINOR;
            }
        }

        score
    }
}
