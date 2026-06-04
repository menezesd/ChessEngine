//! Piece quality evaluation.
//!
//! Implements:
//! - Active vs passive pieces (pieces with few safe moves)
//! - Trapped piece detection (pieces with 0-1 safe squares)
//! - Piece harmony (pieces supporting each other's activity)

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;

/// Passive piece penalty (fewer than 3 safe moves)
pub const PASSIVE_PIECE_MG: i32 = -5;
pub const PASSIVE_PIECE_EG: i32 = -3;

/// Trapped piece penalty (0-1 safe moves)
pub const TRAPPED_PIECE_MG: i32 = -20;
pub const TRAPPED_PIECE_EG: i32 = -15;

/// Active piece bonus (7+ safe moves)
pub const ACTIVE_PIECE_MG: i32 = 5;
pub const ACTIVE_PIECE_EG: i32 = 3;

/// Piece harmony bonus (pieces defending each other while both active)
pub const PIECE_HARMONY_MG: i32 = 4;

impl Board {
    /// Evaluate piece quality.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_piece_quality(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let (color_mg, color_eg) = self.eval_piece_quality_for_color(color, ctx);
            mg += sign * color_mg;
            eg += sign * color_eg;
        }

        (mg, eg)
    }

    fn eval_piece_quality_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let own_pieces = self.occupied_by(color);
        let pawn_attacks = self.pawn_attacks(color.opponent());

        // Evaluate each piece type
        for sq in self.pieces_of(color, Piece::Knight).iter() {
            Self::add_piece_activity_score(
                &mut mg,
                &mut eg,
                KNIGHT_ATTACKS[sq.index()],
                own_pieces,
                pawn_attacks,
            );
        }

        for sq in self.pieces_of(color, Piece::Bishop).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true);
            Self::add_piece_activity_score(&mut mg, &mut eg, attacks, own_pieces, pawn_attacks);
        }

        for sq in self.pieces_of(color, Piece::Rook).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, false);
            Self::add_piece_activity_score(&mut mg, &mut eg, attacks, own_pieces, pawn_attacks);
        }

        for sq in self.pieces_of(color, Piece::Queen).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true)
                | slider_attacks(sq.index(), self.all_occupied.0, false);
            Self::add_piece_activity_score(&mut mg, &mut eg, attacks, own_pieces, pawn_attacks);
        }

        // Piece harmony
        mg += self.eval_piece_harmony(color, ctx);

        (mg, eg)
    }

    fn add_piece_activity_score(
        mg: &mut i32,
        eg: &mut i32,
        attacks: u64,
        own_pieces: Bitboard,
        enemy_pawn_attacks: Bitboard,
    ) {
        let (piece_mg, piece_eg) =
            Self::eval_piece_activity(attacks, own_pieces, enemy_pawn_attacks);
        *mg += piece_mg;
        *eg += piece_eg;
    }

    /// Evaluate activity of a single piece.
    fn eval_piece_activity(
        attacks: u64,
        own_pieces: Bitboard,
        enemy_pawn_attacks: Bitboard,
    ) -> (i32, i32) {
        // Safe squares: not occupied by own pieces, not attacked by enemy pawns
        let safe_squares = attacks & !own_pieces.0 & !enemy_pawn_attacks.0;
        let safe_count = safe_squares.count_ones();

        if safe_count <= 1 {
            // Trapped piece
            return (TRAPPED_PIECE_MG, TRAPPED_PIECE_EG);
        } else if safe_count <= 2 {
            // Passive piece
            return (PASSIVE_PIECE_MG, PASSIVE_PIECE_EG);
        } else if safe_count >= 7 {
            // Very active piece
            return (ACTIVE_PIECE_MG, ACTIVE_PIECE_EG);
        }

        (0, 0)
    }

    /// Evaluate piece harmony (pieces supporting each other while both active).
    fn eval_piece_harmony(&self, color: Color, ctx: &AttackContext) -> i32 {
        let own_attacks = ctx.all_attacks(color);
        let enemy_pawn_attacks = self.pawn_attacks(color.opponent());
        let own_occupied = self.occupied_by(color).0;

        let mut harmony_count = 0;

        // Check minor pieces defending each other
        let knights = self.pieces_of(color, Piece::Knight);
        let bishops = self.pieces_of(color, Piece::Bishop);
        let minor_pieces = Bitboard(knights.0 | bishops.0);

        for sq in minor_pieces.iter() {
            let sq_bit = 1u64 << sq.index();

            // Is this piece defended by another of our pieces?
            if (own_attacks.0 & sq_bit) != 0 {
                // Check if this piece is also active
                let attacks = if (knights.0 & sq_bit) != 0 {
                    KNIGHT_ATTACKS[sq.index()]
                } else {
                    slider_attacks(sq.index(), self.all_occupied.0, true)
                };

                let safe_squares = attacks & !own_occupied & !enemy_pawn_attacks.0;
                if safe_squares.count_ones() >= 4 {
                    // Defended and active - harmony bonus
                    harmony_count += 1;
                }
            }
        }

        harmony_count * PIECE_HARMONY_MG
    }
}

#[cfg(test)]
mod tests;
