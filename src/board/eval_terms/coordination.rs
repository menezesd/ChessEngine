//! Piece coordination evaluation.
//!
//! Implements:
//! - Battery detection (Queen+Bishop or Queen+Rook aligned)
//! - Piece clusters (multiple pieces defending each other)
//! - Overloaded defenders (pieces defending multiple attacked pieces)

use crate::board::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS};
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

        for color in Color::BOTH {
            let sign = color.sign();
            let (color_mg, color_eg) = self.eval_coordination_for_color(color, ctx);
            mg += sign * color_mg;
            eg += sign * color_eg;
        }

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
                if Self::attacks_square(queen_diag_attacks, bishop_sq.index()) {
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
                if Self::attacks_square(queen_file_attacks, rook_sq.index()) {
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

        count += self
            .pieces_of(color, Piece::Pawn)
            .intersect_popcount(Bitboard(Self::pawn_defender_mask(sq, color)))
            as i32;

        // Check knight defenders
        for knight_sq in self.pieces_of(color, Piece::Knight).iter() {
            if Self::attacks_square(KNIGHT_ATTACKS[knight_sq.index()], sq) {
                count += 1;
            }
        }

        count += self.count_slider_defenders(sq, color);

        // Check king defenders
        for king_sq in self.pieces_of(color, Piece::King).iter() {
            if Self::attacks_square(KING_ATTACKS[king_sq.index()], sq) {
                count += 1;
            }
        }

        count
    }

    fn pawn_defender_mask(sq: usize, color: Color) -> u64 {
        let file = sq % 8;
        let mut mask = 0u64;

        match color {
            Color::White if sq >= 8 => {
                if file != 0 {
                    mask |= 1u64 << (sq - 9);
                }
                if file < 7 {
                    mask |= 1u64 << (sq - 7);
                }
            }
            Color::Black if sq < 56 => {
                if file != 0 {
                    mask |= 1u64 << (sq + 7);
                }
                if file < 7 {
                    mask |= 1u64 << (sq + 9);
                }
            }
            _ => {}
        }

        mask
    }

    fn count_slider_defenders(&self, sq: usize, color: Color) -> i32 {
        let mut count = 0;

        for bishop_sq in self.pieces_of(color, Piece::Bishop).iter() {
            if self.slider_defends_square(bishop_sq.index(), sq, true) {
                count += 1;
            }
        }

        for rook_sq in self.pieces_of(color, Piece::Rook).iter() {
            if self.slider_defends_square(rook_sq.index(), sq, false) {
                count += 1;
            }
        }

        for queen_sq in self.pieces_of(color, Piece::Queen).iter() {
            let attacks = slider_attacks(queen_sq.index(), self.all_occupied.0, true)
                | slider_attacks(queen_sq.index(), self.all_occupied.0, false);
            if Self::attacks_square(attacks, sq) {
                count += 1;
            }
        }

        count
    }

    fn slider_defends_square(&self, from: usize, target: usize, diagonal: bool) -> bool {
        Self::attacks_square(slider_attacks(from, self.all_occupied.0, diagonal), target)
    }

    fn attacks_square(attacks: u64, target: usize) -> bool {
        Bitboard(attacks).has_bit(target)
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
mod tests;
