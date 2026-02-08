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
        let mut bonus = 0;

        let our_attacks = ctx.all_attacks(color);
        let enemy_defenses = ctx.all_attacks(color.opponent());

        // Find enemy pieces that are attacked but not defended
        for piece_type in Piece::MINOR_AND_MAJOR {
            let enemy_pieces = self.opponent_pieces(color, piece_type);
            for sq in enemy_pieces.iter() {
                let sq_idx = sq.index();

                // Attacked by us
                if our_attacks.has_bit(sq_idx) {
                    // Not defended by enemy
                    if !enemy_defenses.has_bit(sq_idx) {
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
        // Focus on enemy king area
        let enemy_king_sq = self.king_square_index(color.opponent());
        let king_zone = Self::king_zone(enemy_king_sq);

        // Count how many of our pieces attack the king zone
        let mut attackers = 0;

        let king_zone_bb = crate::board::Bitboard(king_zone);

        for sq in self.pieces_of(color, Piece::Knight).iter() {
            if crate::board::Bitboard(KNIGHT_ATTACKS[sq.index()]).intersects(king_zone_bb) {
                attackers += 1;
            }
        }

        for sq in self.pieces_of(color, Piece::Bishop).iter() {
            if crate::board::Bitboard(slider_attacks(sq.index(), self.all_occupied.0, true)).intersects(king_zone_bb) {
                attackers += 1;
            }
        }

        for sq in self.pieces_of(color, Piece::Rook).iter() {
            if crate::board::Bitboard(slider_attacks(sq.index(), self.all_occupied.0, false)).intersects(king_zone_bb) {
                attackers += 1;
            }
        }

        for sq in self.pieces_of(color, Piece::Queen).iter() {
            let attacks = slider_attacks(sq.index(), self.all_occupied.0, true)
                | slider_attacks(sq.index(), self.all_occupied.0, false);
            if crate::board::Bitboard(attacks).intersects(king_zone_bb) {
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

    /// Castled king positions: [White (c1, g1), Black (c8, g8)]
    const CASTLED_SQUARES: [u64; 2] = [
        (1u64 << 2) | (1u64 << 6),   // c1 or g1
        (1u64 << 58) | (1u64 << 62), // c8 or g8
    ];

    /// Evaluate development.
    fn eval_development(&self, color: Color) -> i32 {
        let mut score = 0;

        // Starting squares for pieces
        let (back_rank, knight_starts, bishop_starts, _rook_starts, _queen_start, king_start) =
            match color {
                Color::White => (
                    0x0000_0000_0000_00FFu64,
                    [1, 6], // b1, g1
                    [2, 5], // c1, f1
                    [0, 7], // a1, h1
                    3,      // d1
                    4,      // e1
                ),
                Color::Black => (
                    0xFF00_0000_0000_0000u64,
                    [57, 62], // b8, g8
                    [58, 61], // c8, f8
                    [56, 63], // a8, h8
                    59,       // d8
                    60,       // e8
                ),
            };

        let back_rank_bb = crate::board::Bitboard(back_rank);

        // Check knights
        let knights = self.pieces_of(color, Piece::Knight);
        for &start_sq in &knight_starts {
            if knights.has_bit(start_sq) {
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
        let bishops = self.pieces_of(color, Piece::Bishop);
        for &start_sq in &bishop_starts {
            if bishops.has_bit(start_sq) {
                score += UNDEVELOPED_PENALTY_MG;
            } else {
                let developed = bishops.and(back_rank_bb.not());
                if !developed.is_empty() {
                    score += DEVELOPMENT_BONUS_MG;
                }
            }
        }

        // Check if castled (king not on starting square and on castled square)
        let kings = self.pieces_of(color, Piece::King);
        let castled = crate::board::Bitboard(Self::CASTLED_SQUARES[color.index()]);

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
        assert!(
            w_dev < 0,
            "starting position should have development penalty"
        );
        assert!(
            b_dev < 0,
            "starting position should have development penalty"
        );
    }

    #[test]
    fn test_developed_pieces() {
        // All minor pieces developed
        let board: Board = "r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1"
            .parse()
            .unwrap();
        let w_dev = board.eval_development(Color::White);
        // Knights are developed - should be better than starting position
        let start_board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let w_start = start_board.eval_development(Color::White);
        assert!(w_dev > w_start, "developed pieces should have better score");
    }

    #[test]
    fn test_tempo_threat() {
        // Position where white attacks an undefended piece
        let board: Board = "8/8/8/3n4/4B3/8/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let bonus = board.eval_tempo_threats(Color::White, &ctx);
        // Bishop attacks undefended knight
        assert!(bonus > 0, "attacking undefended piece should give bonus");
    }

    #[test]
    fn test_no_tempo_defended() {
        // Defended piece shouldn't give tempo
        // Black knight on d5 defended by pawn on e6 (attacks d5), attacked by bishop on b3
        let board: Board = "8/8/4p3/3n4/8/1B6/8/8 w - - 0 1".parse().unwrap();
        let ctx = board.compute_attack_context();
        let bonus = board.eval_tempo_threats(Color::White, &ctx);
        // Knight is defended by pawn - should get no tempo bonus
        assert_eq!(
            bonus, 0,
            "defended piece shouldn't give tempo bonus: {bonus}"
        );
    }

    #[test]
    fn test_castled_bonus() {
        // White is castled kingside
        let board: Board = "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQK2R w KQkq - 0 1"
            .parse()
            .unwrap();
        let uncastled = board.eval_development(Color::White);

        let castled: Board = "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQ1RK1 w kq - 0 1"
            .parse()
            .unwrap();
        let castled_dev = castled.eval_development(Color::White);

        assert!(
            castled_dev > uncastled,
            "castled position should have better development score"
        );
    }

    #[test]
    fn test_initiative_symmetry() {
        // Symmetric position should have balanced initiative
        let board = Board::new();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_initiative(&ctx);
        assert!(
            mg.abs() < 20,
            "symmetric initiative should be near zero: {mg}"
        );
        assert!(
            eg.abs() < 20,
            "symmetric initiative eg should be near zero: {eg}"
        );
    }
}
