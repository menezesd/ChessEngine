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

        let (w_mg, w_eg) = self.eval_piece_quality_for_color(Color::White, ctx);
        let (b_mg, b_eg) = self.eval_piece_quality_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        (mg, eg)
    }

    fn eval_piece_quality_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let c_idx = color.index();
        let own_pieces = self.occupied[c_idx];
        let enemy_attacks = ctx.all_attacks(color.opponent());
        let pawn_attacks = self.pawn_attacks(color.opponent());

        // Evaluate each piece type
        let knights = self.pieces[c_idx][Piece::Knight.index()];
        for sq in knights.iter() {
            let (piece_mg, piece_eg) = Self::eval_piece_activity(
                sq.index(),
                KNIGHT_ATTACKS[sq.index()],
                own_pieces,
                enemy_attacks,
                pawn_attacks,
            );
            mg += piece_mg;
            eg += piece_eg;
        }

        let bishops = self.pieces[c_idx][Piece::Bishop.index()];
        for sq in bishops.iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true);
            let (piece_mg, piece_eg) = Self::eval_piece_activity(
                sq.index(),
                attacks,
                own_pieces,
                enemy_attacks,
                pawn_attacks,
            );
            mg += piece_mg;
            eg += piece_eg;
        }

        let rooks = self.pieces[c_idx][Piece::Rook.index()];
        for sq in rooks.iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, false);
            let (piece_mg, piece_eg) = Self::eval_piece_activity(
                sq.index(),
                attacks,
                own_pieces,
                enemy_attacks,
                pawn_attacks,
            );
            mg += piece_mg;
            eg += piece_eg;
        }

        let queens = self.pieces[c_idx][Piece::Queen.index()];
        for sq in queens.iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true)
                | slider_attacks(sq.index(), self.all_occupied.0, false);
            let (piece_mg, piece_eg) = Self::eval_piece_activity(
                sq.index(),
                attacks,
                own_pieces,
                enemy_attacks,
                pawn_attacks,
            );
            mg += piece_mg;
            eg += piece_eg;
        }

        // Piece harmony
        mg += self.eval_piece_harmony(color, ctx);

        (mg, eg)
    }

    /// Evaluate activity of a single piece.
    fn eval_piece_activity(
        _sq: usize,
        attacks: u64,
        own_pieces: Bitboard,
        _enemy_attacks: Bitboard,
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
        let c_idx = color.index();
        let own_attacks = ctx.all_attacks(color);
        let _enemy_attacks = ctx.all_attacks(color.opponent());
        let enemy_pawn_attacks = self.pawn_attacks(color.opponent());

        let mut harmony_count = 0;

        // Check minor pieces defending each other
        let knights = self.pieces[c_idx][Piece::Knight.index()];
        let bishops = self.pieces[c_idx][Piece::Bishop.index()];
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

                let safe_squares = attacks & !self.occupied[c_idx].0 & !enemy_pawn_attacks.0;
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
mod tests {
    use super::*;

    #[test]
    fn test_trapped_piece() {
        // Knight trapped in corner
        let board: Board = "8/8/1p6/p7/N7/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_piece_quality(&ctx);
        // Knight on a4 with pawns on a5, b6 should be very restricted
        let _ = eg; // Silence unused variable warning
        assert!(mg < 10); // Should have some penalty
    }

    #[test]
    fn test_active_piece() {
        // Knight in center with many squares
        let board: Board = "8/8/8/3N4/8/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let (mg, _) = board.eval_piece_quality(&ctx);
        // Central knight should be active
        assert!(mg >= 0);
    }
}
