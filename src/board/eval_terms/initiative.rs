//! Initiative evaluation.
//!
//! Implements:
//! - Tempo threats (forcing moves available)
//! - Attack momentum (building pressure)
//! - Development advantage (in opening/middlegame)

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Color, Piece};

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

        let (w_mg, _) = self.eval_initiative_for_color(Color::White, ctx);
        let (b_mg, _) = self.eval_initiative_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;

        // Apply initiative bonus/penalty based on game phase
        // Initiative matters less in endgame
        (mg, eg)
    }

    fn eval_initiative_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;

        let _c_idx = color.index();
        let _opp_idx = color.opponent().index();

        // Tempo threats (attacking undefended pieces)
        mg += self.eval_tempo_threats(color, ctx);

        // Attack momentum
        mg += self.eval_attack_momentum(color, ctx);

        // Development
        mg += self.eval_development(color);

        (mg, 0)
    }

    /// Evaluate tempo threats (attacks on undefended pieces).
    fn eval_tempo_threats(&self, color: Color, ctx: &AttackContext) -> i32 {
        let _c_idx = color.index();
        let opp_idx = color.opponent().index();
        let mut bonus = 0;

        let our_attacks = ctx.all_attacks(color);
        let enemy_defenses = ctx.all_attacks(color.opponent());

        // Find enemy pieces that are attacked but not defended
        for piece_type in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
            let enemy_pieces = self.pieces[opp_idx][piece_type.index()];
            for sq in enemy_pieces.iter() {
                let sq_bit = 1u64 << sq.index();

                // Attacked by us
                if (our_attacks.0 & sq_bit) != 0 {
                    // Not defended by enemy
                    if (enemy_defenses.0 & sq_bit) == 0 {
                        // Undefended piece attacked = tempo
                        bonus += TEMPO_THREAT_MG;
                    }
                }
            }
        }

        bonus
    }

    /// Evaluate attack momentum (multiple pieces converging on same area).
    fn eval_attack_momentum(&self, color: Color, _ctx: &AttackContext) -> i32 {
        let c_idx = color.index();
        let opp_idx = color.opponent().index();

        // Focus on enemy king area
        let enemy_king_bb = self.pieces[opp_idx][Piece::King.index()];
        if enemy_king_bb.0 == 0 {
            return 0;
        }

        let enemy_king_sq = enemy_king_bb.0.trailing_zeros() as usize;
        let king_zone = Self::king_zone(enemy_king_sq);

        // Count how many of our pieces attack the king zone
        let mut attackers = 0;

        let knights = self.pieces[c_idx][Piece::Knight.index()];
        for sq in knights.iter() {
            if (KNIGHT_ATTACKS[sq.index()] & king_zone) != 0 {
                attackers += 1;
            }
        }

        let bishops = self.pieces[c_idx][Piece::Bishop.index()];
        for sq in bishops.iter() {
            if (slider_attacks(sq.index(), self.all_occupied.0, true) & king_zone) != 0 {
                attackers += 1;
            }
        }

        let rooks = self.pieces[c_idx][Piece::Rook.index()];
        for sq in rooks.iter() {
            if (slider_attacks(sq.index(), self.all_occupied.0, false) & king_zone) != 0 {
                attackers += 1;
            }
        }

        let queens = self.pieces[c_idx][Piece::Queen.index()];
        for sq in queens.iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true)
                | slider_attacks(sq.index(), self.all_occupied.0, false);
            if (attacks & king_zone) != 0 {
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

    /// Evaluate development.
    fn eval_development(&self, color: Color) -> i32 {
        let c_idx = color.index();
        let mut score = 0;

        // Starting squares for pieces
        let (back_rank, knight_starts, bishop_starts, _rook_starts, _queen_start, king_start) = match color {
            Color::White => (
                0x0000_0000_0000_00FFu64,
                [1, 6],       // b1, g1
                [2, 5],       // c1, f1
                [0, 7],       // a1, h1
                3,            // d1
                4,            // e1
            ),
            Color::Black => (
                0xFF00_0000_0000_0000u64,
                [57, 62],     // b8, g8
                [58, 61],     // c8, f8
                [56, 63],     // a8, h8
                59,           // d8
                60,           // e8
            ),
        };

        // Check knights
        let knights = self.pieces[c_idx][Piece::Knight.index()];
        for &start_sq in &knight_starts {
            if (knights.0 & (1u64 << start_sq)) != 0 {
                score += UNDEVELOPED_PENALTY_MG;
            } else {
                // Knight developed
                let developed = knights.0 & !back_rank;
                if developed.count_ones() > 0 {
                    score += DEVELOPMENT_BONUS_MG;
                }
            }
        }

        // Check bishops
        let bishops = self.pieces[c_idx][Piece::Bishop.index()];
        for &start_sq in &bishop_starts {
            if (bishops.0 & (1u64 << start_sq)) != 0 {
                score += UNDEVELOPED_PENALTY_MG;
            } else {
                let developed = bishops.0 & !back_rank;
                if developed.count_ones() > 0 {
                    score += DEVELOPMENT_BONUS_MG;
                }
            }
        }

        // Check if castled (king not on starting square and on castled square)
        let kings = self.pieces[c_idx][Piece::King.index()];
        let castled_squares = match color {
            Color::White => (1u64 << 2) | (1u64 << 6),  // c1 or g1
            Color::Black => (1u64 << 58) | (1u64 << 62), // c8 or g8
        };

        if (kings.0 & castled_squares) != 0 {
            score += CASTLED_BONUS_MG;
        } else if (kings.0 & (1u64 << king_start)) != 0 {
            // King still on starting square - slight penalty in middlegame
            // (already captured by other eval terms, so minor here)
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_development() {
        // Starting position
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let w_dev = board.eval_development(Color::White);
        let b_dev = board.eval_development(Color::Black);
        // Both should have undeveloped penalties
        assert!(w_dev < 0);
        assert!(b_dev < 0);
    }

    #[test]
    fn test_tempo_threat() {
        // Position where white attacks an undefended piece
        let board: Board = "8/8/8/3n4/4B3/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let bonus = board.eval_tempo_threats(Color::White, &ctx);
        // Bishop attacks undefended knight
        assert!(bonus > 0);
    }
}
