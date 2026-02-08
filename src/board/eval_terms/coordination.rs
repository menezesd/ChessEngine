//! Piece coordination evaluation.
//!
//! Implements:
//! - Battery detection (Queen+Bishop or Queen+Rook aligned)
//! - Piece clusters (multiple pieces defending each other)
//! - Overloaded defenders (pieces defending multiple attacked pieces)

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;

/// Battery bonus (Queen + Bishop on diagonal or Queen + Rook on file/rank)
pub const BATTERY_DIAGONAL_MG: i32 = 15;
pub const BATTERY_FILE_MG: i32 = 20;

/// Overloaded defender penalty
pub const OVERLOADED_PENALTY_MG: i32 = -12;

/// Piece cluster bonus (multiple pieces defending each other)
pub const CLUSTER_BONUS_MG: i32 = 3;
pub const CLUSTER_BONUS_EG: i32 = 2;

/// Doubled rooks bonus (two rooks on same file)
pub const DOUBLED_ROOKS_MG: i32 = BATTERY_FILE_MG / 2;

impl Board {
    /// Evaluate piece coordination.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_coordination(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        // Evaluate for both colors
        let (w_mg, w_eg) = self.eval_coordination_for_color(Color::White, ctx);
        let (b_mg, b_eg) = self.eval_coordination_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        (mg, eg)
    }

    fn eval_coordination_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        // Battery detection
        mg += self.eval_batteries(color);

        // Piece cluster evaluation
        let (cluster_mg, cluster_eg) = self.eval_clusters(color);
        mg += cluster_mg;
        eg += cluster_eg;

        // Overloaded defenders
        mg += self.eval_overloaded(color, ctx);

        (mg, eg)
    }

    /// Evaluate batteries (aligned heavy pieces)
    fn eval_batteries(&self, color: Color) -> i32 {
        let mut bonus = 0;

        let queens = self.pieces_of(color, Piece::Queen);
        let bishops = self.pieces_of(color, Piece::Bishop);
        let rooks = self.pieces_of(color, Piece::Rook);

        // Check for Queen + Bishop batteries (diagonal alignment)
        for queen_sq in queens.iter() {
            let queen_diag_attacks = slider_attacks(queen_sq.index(), self.all_occupied.0, true);

            for bishop_sq in bishops.iter() {
                // Check if bishop is on same diagonal as queen
                if Bitboard(queen_diag_attacks).has_bit(bishop_sq.index()) {
                    // They're aligned on a diagonal
                    bonus += BATTERY_DIAGONAL_MG;
                }
            }
        }

        // Check for Queen + Rook batteries (file/rank alignment)
        for queen_sq in queens.iter() {
            let queen_file_attacks = slider_attacks(queen_sq.index(), self.all_occupied.0, false);

            for rook_sq in rooks.iter() {
                // Check if rook is on same file/rank as queen
                if Bitboard(queen_file_attacks).has_bit(rook_sq.index()) {
                    bonus += BATTERY_FILE_MG;
                }
            }
        }

        // Check for doubled rooks (Rook + Rook on same file)
        for rook1 in rooks.iter() {
            for rook2 in rooks.iter() {
                if rook2.index() > rook1.index() && rook1.file() == rook2.file() {
                    bonus += DOUBLED_ROOKS_MG;
                }
            }
        }

        bonus
    }

    /// Evaluate piece clusters (pieces defending each other)
    fn eval_clusters(&self, color: Color) -> (i32, i32) {
        let mut defended_count = 0;

        // Count how many of our pieces are defended by other pieces
        let own_attacks = self.all_attacks(color);

        // Check each piece type (except pawns, handled elsewhere)
        for piece_type in Piece::MINOR_AND_MAJOR {
            for sq in self.pieces_of(color, piece_type).iter() {
                // Is this piece defended by another of our pieces?
                if own_attacks.has_bit(sq.index()) {
                    defended_count += 1;
                }
            }
        }

        (
            defended_count * CLUSTER_BONUS_MG,
            defended_count * CLUSTER_BONUS_EG,
        )
    }

    /// Evaluate overloaded defenders
    fn eval_overloaded(&self, color: Color, ctx: &AttackContext) -> i32 {
        let enemy_attacks = ctx.all_attacks(color.opponent());

        let mut penalty = 0;

        // Find pieces that are attacked by enemy and defended only once
        for piece_type in Piece::MINOR_AND_MAJOR {
            for sq in self.pieces_of(color, piece_type).iter() {
                // Is this piece attacked?
                if enemy_attacks.has_bit(sq.index()) {
                    // Count defenders for this piece
                    let defender_count = self.count_defenders(sq.index(), color);

                    // If only one defender and that defender defends multiple pieces, it's overloaded
                    if defender_count == 1 {
                        // Find the defender and check if it defends other attacked pieces
                        if self.is_defender_overloaded(sq.index(), color, enemy_attacks) {
                            penalty += OVERLOADED_PENALTY_MG;
                        }
                    }
                }
            }
        }

        penalty
    }

    /// Count how many pieces defend a square
    fn count_defenders(&self, sq: usize, color: Color) -> i32 {
        let mut count = 0;

        // Check pawn defenders
        // Pawns that could attack this square are on adjacent files, one rank behind
        let file = sq % 8;
        let defending_pawn_sqs = match color {
            Color::White => {
                let mut sqs = 0u64;
                if sq >= 8 {
                    if file != 0 {
                        sqs |= 1u64 << (sq - 9);
                    }
                    if file < 7 {
                        sqs |= 1u64 << (sq - 7);
                    }
                }
                sqs
            }
            Color::Black => {
                let mut sqs = 0u64;
                if sq < 56 {
                    if file != 0 {
                        sqs |= 1u64 << (sq + 7);
                    }
                    if file < 7 {
                        sqs |= 1u64 << (sq + 9);
                    }
                }
                sqs
            }
        };
        count += self.pieces_of(color, Piece::Pawn).intersect_popcount(Bitboard(defending_pawn_sqs)) as i32;

        // Check knight defenders
        for knight_sq in self.pieces_of(color, Piece::Knight).iter() {
            if Bitboard(KNIGHT_ATTACKS[knight_sq.index()]).has_bit(sq) {
                count += 1;
            }
        }

        // Check bishop/queen diagonal defenders
        for bishop_sq in self.pieces_of(color, Piece::Bishop).iter() {
            if Bitboard(slider_attacks(bishop_sq.index(), self.all_occupied.0, true)).has_bit(sq) {
                count += 1;
            }
        }

        // Check rook/queen file defenders
        for rook_sq in self.pieces_of(color, Piece::Rook).iter() {
            if Bitboard(slider_attacks(rook_sq.index(), self.all_occupied.0, false)).has_bit(sq) {
                count += 1;
            }
        }

        // Check queen defenders (both diagonal and file)
        for queen_sq in self.pieces_of(color, Piece::Queen).iter() {
            let attacks = slider_attacks(queen_sq.index(), self.all_occupied.0, true)
                | slider_attacks(queen_sq.index(), self.all_occupied.0, false);
            if Bitboard(attacks).has_bit(sq) {
                count += 1;
            }
        }

        // Check king defenders
        for king_sq in self.pieces_of(color, Piece::King).iter() {
            if Bitboard(crate::board::attack_tables::KING_ATTACKS[king_sq.index()]).has_bit(sq) {
                count += 1;
            }
        }

        count
    }

    /// Check if the defender of a piece is overloaded (defends multiple attacked pieces)
    fn is_defender_overloaded(&self, sq: usize, color: Color, enemy_attacks: Bitboard) -> bool {
        // Simplified: check if there are multiple attacked pieces nearby
        // A full implementation would trace the specific defender

        let mut attacked_pieces_near = 0;

        // Count attacked pieces within knight-move distance
        let file = sq % 8;
        let rank = sq / 8;

        for piece_type in Piece::MINOR_AND_MAJOR {
            for piece_sq in self.pieces_of(color, piece_type).iter() {
                let pf = piece_sq.file();
                let pr = piece_sq.rank();

                // Within 2 squares and attacked by enemy
                if (file as i32 - pf as i32).abs() <= 2
                    && (rank as i32 - pr as i32).abs() <= 2
                    && enemy_attacks.has_bit(piece_sq.index())
                {
                    attacked_pieces_near += 1;
                }
            }
        }

        attacked_pieces_near >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_battery_detection() {
        // Queen and bishop on same diagonal
        let board: Board = "8/8/8/8/3B4/8/1Q6/8 w - - 0 1".parse().unwrap();
        let bonus = board.eval_batteries(Color::White);
        assert!(bonus > 0, "diagonal battery should give bonus");
    }

    #[test]
    fn test_doubled_rooks() {
        // Doubled rooks on e-file
        let board: Board = "8/8/8/8/4R3/8/4R3/8 w - - 0 1".parse().unwrap();
        let bonus = board.eval_batteries(Color::White);
        assert!(bonus > 0, "doubled rooks should give bonus");
    }

    #[test]
    fn test_queen_rook_battery() {
        // Queen and rook on same file
        let board: Board = "8/8/8/4Q3/8/8/4R3/8 w - - 0 1".parse().unwrap();
        let bonus = board.eval_batteries(Color::White);
        assert!(
            bonus >= BATTERY_FILE_MG,
            "Q+R file battery should give bonus"
        );
    }

    #[test]
    fn test_no_battery_unaligned() {
        // Queen and bishop not aligned
        let board: Board = "8/8/8/8/3B4/8/8/1Q6 w - - 0 1".parse().unwrap();
        let bonus = board.eval_batteries(Color::White);
        // Should be 0 or much smaller than aligned
        assert!(
            bonus < BATTERY_DIAGONAL_MG,
            "unaligned pieces should not get full battery bonus"
        );
    }

    #[test]
    fn test_cluster_defended_pieces() {
        // Knight defended by bishop
        let board: Board = "8/8/8/3N4/2B5/8/8/8 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_clusters(Color::White);
        assert!(mg > 0 || eg > 0, "defended piece should give cluster bonus");
    }

    #[test]
    fn test_full_coordination_symmetry() {
        // Symmetric position should give roughly equal coordination
        let board: Board = "r1bqkb1r/pppppppp/2n2n2/8/8/2N2N2/PPPPPPPP/R1BQKB1R w KQkq - 0 1"
            .parse()
            .unwrap();
        let ctx = board.compute_attack_context();
        let (mg, eg) = board.eval_coordination(&ctx);
        assert!(
            mg.abs() < 20,
            "symmetric coordination should be near zero: {mg}"
        );
        assert!(
            eg.abs() < 20,
            "symmetric coordination eg should be near zero: {eg}"
        );
    }
}
