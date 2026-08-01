//! Initiative evaluation.
//!
//! Implements:
//! - Tempo threats (forcing moves available)
//! - Attack momentum (building pressure)
//! - Development advantage (in opening/middlegame)

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;

/// Tempo threat bonus (attacking undefended pieces)
pub const TEMPO_THREAT_MG: i32 = 8;

/// Attack momentum (multiple pieces attacking same area)
pub const ATTACK_MOMENTUM_MG: i32 = 3;

/// Development bonus per developed piece
pub const DEVELOPMENT_BONUS_MG: i32 = 5;

/// Undeveloped penalty (pieces on starting squares)
pub const UNDEVELOPED_PENALTY_MG: i32 = -8;

/// Castling bonus
pub const CASTLED_BONUS_MG: i32 = 15;

impl Board {
    /// Evaluate initiative.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_initiative(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let eg = 0; // Initiative is primarily a middlegame concept

        for color in Color::BOTH {
            let sign = color.sign();
            let (color_mg, _) = self.eval_initiative_for_color(color, ctx);
            mg += sign * color_mg;
        }

        // Apply initiative bonus/penalty based on game phase
        // Initiative matters less in endgame
        (mg, eg)
    }

    fn eval_initiative_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;

        // Tempo threats (attacking undefended pieces)
        mg += self.eval_tempo_threats(color, ctx);

        // Attack momentum
        mg += self.eval_attack_momentum(color);

        // Development
        mg += self.eval_development(color);

        (mg, 0)
    }

    /// Evaluate tempo threats (attacks on undefended pieces).
    fn eval_tempo_threats(&self, color: Color, ctx: &AttackContext) -> i32 {
        let mut bonus = 0;

        let our_attacks = ctx.all_attacks(color);
        let enemy_defenses = ctx.all_attacks(color.opponent());

        // Find enemy pieces that are attacked but not defended
        for piece_type in Piece::MINOR_AND_MAJOR {
            let enemy_pieces = self.opponent_pieces(color, piece_type);
            for sq in enemy_pieces.iter() {
                let sq_idx = sq.index();

                // Undefended piece attacked = tempo
                if our_attacks.has_bit(sq_idx) && !enemy_defenses.has_bit(sq_idx) {
                    bonus += TEMPO_THREAT_MG;
                }
            }
        }

        bonus
    }

    /// Evaluate attack momentum (multiple pieces converging on same area).
    fn eval_attack_momentum(&self, color: Color) -> i32 {
        // Focus on enemy king area
        let enemy_king_sq = self.king_square_index(color.opponent());
        let king_zone = Self::king_zone(enemy_king_sq);

        // Count how many of our pieces attack the king zone
        let mut attackers = 0;

        let king_zone_bb = Bitboard(king_zone);

        for sq in self.pieces_of(color, Piece::Knight).iter() {
            if Self::attacks_king_zone(KNIGHT_ATTACKS[sq.index()], king_zone_bb) {
                attackers += 1;
            }
        }

        for sq in self.pieces_of(color, Piece::Bishop).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true);
            if Self::attacks_king_zone(attacks, king_zone_bb) {
                attackers += 1;
            }
        }

        for sq in self.pieces_of(color, Piece::Rook).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, false);
            if Self::attacks_king_zone(attacks, king_zone_bb) {
                attackers += 1;
            }
        }

        for sq in self.pieces_of(color, Piece::Queen).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true)
                | slider_attacks(sq.index(), self.all_occupied.0, false);
            if Self::attacks_king_zone(attacks, king_zone_bb) {
                attackers += 2; // Queen counts double
            }
        }

        // Momentum bonus scales with number of attackers
        if attackers >= 2 {
            attackers * ATTACK_MOMENTUM_MG
        } else {
            0
        }
    }

    /// Get king zone (king square + adjacent squares).
    fn king_zone(king_sq: usize) -> u64 {
        crate::board::attack_tables::KING_ATTACKS[king_sq] | (1u64 << king_sq)
    }

    fn attacks_king_zone(attacks: u64, king_zone: Bitboard) -> bool {
        Bitboard(attacks).intersects(king_zone)
    }

    /// Castled king positions: [White (c1, g1), Black (c8, g8)]
    const CASTLED_SQUARES: [u64; 2] = [
        (1u64 << 2) | (1u64 << 6),   // c1 or g1
        (1u64 << 58) | (1u64 << 62), // c8 or g8
    ];

    /// Evaluate development.
    fn eval_development(&self, color: Color) -> i32 {
        let mut score = 0;

        let (back_rank, knight_starts, bishop_starts, king_start) = match color {
            Color::White => (
                0x0000_0000_0000_00FFu64,
                [1, 6], // b1, g1
                [2, 5], // c1, f1
                4,      // e1
            ),
            Color::Black => (
                0xFF00_0000_0000_0000u64,
                [57, 62], // b8, g8
                [58, 61], // c8, f8
                60,       // e8
            ),
        };

        let back_rank_bb = Bitboard(back_rank);

        // Penalize pieces that remain on their original squares, then award
        // each actual developed piece once.  Counting development per empty
        // starting square would award a surviving developed minor twice when
        // its original partner had been captured.
        let knights = self.pieces_of(color, Piece::Knight);
        for &start_sq in &knight_starts {
            if knights.has_bit(start_sq) {
                score += UNDEVELOPED_PENALTY_MG;
            }
        }
        score += (knights.0 & !back_rank).count_ones() as i32 * DEVELOPMENT_BONUS_MG;

        let bishops = self.pieces_of(color, Piece::Bishop);
        for &start_sq in &bishop_starts {
            if bishops.has_bit(start_sq) {
                score += UNDEVELOPED_PENALTY_MG;
            }
        }
        score += bishops.and(back_rank_bb.not()).popcount() as i32 * DEVELOPMENT_BONUS_MG;

        // Check if castled (king not on starting square and on castled square)
        let kings = self.pieces_of(color, Piece::King);
        let castled = Bitboard(Self::CASTLED_SQUARES[color.index()]);

        if kings.intersects(castled) {
            score += CASTLED_BONUS_MG;
        } else if kings.has_bit(king_start) {
            // King still on starting square - slight penalty in middlegame
            // (already captured by other eval terms, so minor here)
        }

        score
    }
}

#[cfg(test)]
mod tests;
