//! Advanced threats and tactics evaluation.
//!
//! Implements:
//! - Fork threats (knights/pawns threatening multiple pieces)
//! - Pin detection (pieces pinned to king or queen)
//! - Skewer threats (slider alignment through valuable pieces)
//! - Discovery potential (pieces that can discover attacks)

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::single_pawn_attacks;

/// Fork threat bonus
pub const FORK_THREAT_MG: i32 = 20;

/// Pin bonus (piece pinned to king/queen)
pub const PIN_TO_KING_MG: i32 = 25;
pub const PIN_TO_QUEEN_MG: i32 = 15;

/// Skewer threat bonus
pub const SKEWER_THREAT_MG: i32 = 10;

/// Discovery potential bonus
pub const DISCOVERY_POTENTIAL_MG: i32 = 12;

impl Board {
    /// Evaluate advanced threats.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_threats_advanced(&self) -> (i32, i32) {
        let mut mg = 0;
        let eg = 0; // Tactical threats are primarily MG

        for color in Color::BOTH {
            let sign = color.sign();
            let (color_mg, _) = self.eval_threats_for_color(color);
            mg += sign * color_mg;
        }

        (mg, eg)
    }

    fn eval_threats_for_color(&self, color: Color) -> (i32, i32) {
        let mut mg = 0;

        // Fork threats
        mg += self.eval_fork_threats(color);

        // Pin detection
        mg += self.eval_pins(color);

        // Skewer threats
        mg += self.eval_skewers(color);

        // Discovery potential
        mg += self.eval_discovery_potential(color);

        (mg, 0)
    }

    /// Evaluate fork threats.
    fn eval_fork_threats(&self, color: Color) -> i32 {
        let mut bonus = 0;

        let enemy_king = self.opponent_pieces(color, Piece::King);
        let enemy_queen = self.opponent_pieces(color, Piece::Queen);
        let enemy_rooks = self.opponent_pieces(color, Piece::Rook);

        // High-value targets for forks
        let high_value = Bitboard(enemy_king.0 | enemy_queen.0 | enemy_rooks.0);

        // Knight fork threats
        for knight_sq in self.pieces_of(color, Piece::Knight).iter() {
            let attacks = KNIGHT_ATTACKS[knight_sq.index()];
            let targets_hit = (attacks & high_value.0).count_ones();

            if targets_hit >= 2 {
                bonus += FORK_THREAT_MG;
            }
        }

        // Pawn fork threats
        for pawn_sq in self.pieces_of(color, Piece::Pawn).iter() {
            let Some(attacks) = single_pawn_attacks(pawn_sq.index(), color) else {
                continue;
            };

            let targets_hit = (attacks & high_value.0).count_ones();
            if targets_hit >= 2 {
                bonus += FORK_THREAT_MG;
            }
        }

        bonus
    }

    /// Evaluate pins.
    fn eval_pins(&self, color: Color) -> i32 {
        let opp = color.opponent();
        let mut bonus = 0;

        let enemy_queen_bb = self.opponent_pieces(color, Piece::Queen);
        let enemy_king_sq = self.king_square_index(opp);

        bonus += self.eval_slider_pins_to_target(color, enemy_king_sq, opp, true);

        // Pins to queen (if queen exists)
        if enemy_queen_bb.0 != 0 {
            let enemy_queen_sq = enemy_queen_bb.0.trailing_zeros() as usize;
            bonus += self.eval_slider_pins_to_target(color, enemy_queen_sq, opp, false);
        }

        bonus
    }

    fn eval_slider_pins_to_target(
        &self,
        color: Color,
        target_sq: usize,
        opponent: Color,
        to_king: bool,
    ) -> i32 {
        let mut bonus = 0;

        for bishop_sq in self.pieces_of(color, Piece::Bishop).iter() {
            bonus += self.check_pin(bishop_sq.index(), target_sq, true, opponent, to_king);
        }

        if to_king {
            for queen_sq in self.pieces_of(color, Piece::Queen).iter() {
                bonus += self.check_pin(queen_sq.index(), target_sq, true, opponent, to_king);
                bonus += self.check_pin(queen_sq.index(), target_sq, false, opponent, to_king);
            }
        }

        for rook_sq in self.pieces_of(color, Piece::Rook).iter() {
            bonus += self.check_pin(rook_sq.index(), target_sq, false, opponent, to_king);
        }

        bonus
    }

    /// Check if a slider pins a piece to a target.
    fn check_pin(
        &self,
        slider_sq: usize,
        target_sq: usize,
        diagonal: bool,
        opponent: Color,
        to_king: bool,
    ) -> i32 {
        let attacks = slider_attacks(slider_sq, self.all_occupied.0, diagonal);

        // Check if slider attacks the target
        if (attacks & (1u64 << target_sq)) != 0 {
            return 0; // Direct attack, not a pin
        }

        // Check if there's exactly one piece between slider and target
        let between_mask = Self::between_mask(slider_sq, target_sq);
        if between_mask == 0 {
            return 0;
        }

        let blockers = self.all_occupied.0 & between_mask;
        if blockers.is_power_of_two() {
            // Exactly one blocker - check if it's an enemy piece
            if (self.occupied_by(opponent).0 & blockers) != 0 {
                return if to_king {
                    PIN_TO_KING_MG
                } else {
                    PIN_TO_QUEEN_MG
                };
            }
        }

        0
    }

    /// Get the squares between two squares on a line.
    fn between_mask(sq1: usize, sq2: usize) -> u64 {
        let Some((file_diff, rank_diff)) = Self::line_step(sq1, sq2) else {
            return 0;
        };

        let file1 = sq1 % 8;
        let rank1 = sq1 / 8;
        let file2 = sq2 % 8;
        let rank2 = sq2 / 8;

        let mut mask = 0u64;
        let mut f = file1 as i32 + file_diff;
        let mut r = rank1 as i32 + rank_diff;

        while f != file2 as i32 || r != rank2 as i32 {
            if !(0..=7).contains(&f) || !(0..=7).contains(&r) {
                break;
            }
            mask |= 1u64 << (r * 8 + f);
            f += file_diff;
            r += rank_diff;
        }

        mask
    }

    fn line_step(sq1: usize, sq2: usize) -> Option<(i32, i32)> {
        let file1 = sq1 % 8;
        let rank1 = sq1 / 8;
        let file2 = sq2 % 8;
        let rank2 = sq2 / 8;

        let file_delta = file2 as i32 - file1 as i32;
        let rank_delta = rank2 as i32 - rank1 as i32;

        if file_delta == 0 && rank_delta == 0 {
            return None;
        }

        let same_file = file_delta == 0;
        let same_rank = rank_delta == 0;
        let same_diagonal = file_delta.abs() == rank_delta.abs();
        if !(same_file || same_rank || same_diagonal) {
            return None;
        }

        Some((file_delta.signum(), rank_delta.signum()))
    }

    /// Evaluate skewer threats.
    fn eval_skewers(&self, color: Color) -> i32 {
        let opp = color.opponent();
        let mut bonus = 0;

        // Skewer: valuable piece in front of less valuable piece
        let enemy_king = self.opponent_pieces(color, Piece::King);
        let enemy_queen = self.opponent_pieces(color, Piece::Queen);
        let enemy_rooks = self.opponent_pieces(color, Piece::Rook);

        // Check for queen skewers (king in front of rook/queen)
        if enemy_king.0 != 0 {
            let king_sq = self.king_square_index(opp);
            let back_targets = enemy_queen.0 | enemy_rooks.0;
            bonus += self.eval_slider_skewers(color, king_sq, back_targets);
        }

        bonus
    }

    fn eval_slider_skewers(&self, color: Color, front_sq: usize, back_targets: u64) -> i32 {
        let mut bonus = 0;

        for slider in self.pieces_of(color, Piece::Bishop).iter() {
            if self.is_skewer(slider.index(), front_sq, back_targets, true) {
                bonus += SKEWER_THREAT_MG;
            }
        }

        for slider in self.pieces_of(color, Piece::Rook).iter() {
            if self.is_skewer(slider.index(), front_sq, back_targets, false) {
                bonus += SKEWER_THREAT_MG;
            }
        }

        for slider in self.pieces_of(color, Piece::Queen).iter() {
            if self.is_skewer(slider.index(), front_sq, back_targets, true) {
                bonus += SKEWER_THREAT_MG;
            }
            if self.is_skewer(slider.index(), front_sq, back_targets, false) {
                bonus += SKEWER_THREAT_MG;
            }
        }

        bonus
    }

    /// Check if a skewer exists.
    fn is_skewer(
        &self,
        slider_sq: usize,
        front_sq: usize,
        back_targets: u64,
        diagonal: bool,
    ) -> bool {
        // Check if slider attacks front piece
        let attacks = slider_attacks(slider_sq, self.all_occupied.0, diagonal);
        if (attacks & (1u64 << front_sq)) == 0 {
            return false;
        }

        // Check if there's a target behind
        let x_ray = slider_attacks(
            slider_sq,
            self.all_occupied.0 & !(1u64 << front_sq),
            diagonal,
        );
        (x_ray & back_targets) != 0
    }

    /// Evaluate discovery potential.
    fn eval_discovery_potential(&self, color: Color) -> i32 {
        let opp = color.opponent();
        let mut bonus = 0;

        let enemy_king_bb = self.opponent_pieces(color, Piece::King);
        if enemy_king_bb.0 == 0 {
            return 0;
        }

        let enemy_king_sq = self.king_square_index(opp);
        let our_occupied = self.occupied_by(color).0;

        // Check for discovered attack potential
        for bishop_sq in self.pieces_of(color, Piece::Bishop).iter() {
            let x_ray = slider_attacks(bishop_sq.index(), 0, true); // Empty board
            if (x_ray & (1u64 << enemy_king_sq)) != 0 {
                // Our bishop could attack king if blockers moved
                let between = Self::between_mask(bishop_sq.index(), enemy_king_sq);
                let our_blockers = our_occupied & between;
                if our_blockers.is_power_of_two() {
                    // One of our pieces can discover an attack
                    bonus += DISCOVERY_POTENTIAL_MG;
                }
            }
        }

        for rook_sq in self.pieces_of(color, Piece::Rook).iter() {
            let x_ray = slider_attacks(rook_sq.index(), 0, false);
            if (x_ray & (1u64 << enemy_king_sq)) != 0 {
                let between = Self::between_mask(rook_sq.index(), enemy_king_sq);
                let our_blockers = our_occupied & between;
                if our_blockers.is_power_of_two() {
                    bonus += DISCOVERY_POTENTIAL_MG;
                }
            }
        }

        bonus
    }
}

#[cfg(test)]
mod tests;
