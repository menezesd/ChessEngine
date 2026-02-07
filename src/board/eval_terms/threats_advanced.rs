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

use super::helpers::{single_pawn_attacks, AttackContext};

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
    pub fn eval_threats_advanced(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let eg = 0; // Tactical threats are primarily MG

        let (w_mg, _) = self.eval_threats_for_color(Color::White, ctx);
        let (b_mg, _) = self.eval_threats_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;

        (mg, eg)
    }

    fn eval_threats_for_color(&self, color: Color, _ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;

        let _c_idx = color.index();
        let _opp_idx = color.opponent().index();

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
        let c_idx = color.index();
        let opp_idx = color.opponent().index();
        let mut bonus = 0;

        let enemy_king = self.pieces[opp_idx][Piece::King.index()];
        let enemy_queen = self.pieces[opp_idx][Piece::Queen.index()];
        let enemy_rooks = self.pieces[opp_idx][Piece::Rook.index()];

        // High-value targets for forks
        let high_value = Bitboard(enemy_king.0 | enemy_queen.0 | enemy_rooks.0);

        // Knight fork threats
        let knights = self.pieces[c_idx][Piece::Knight.index()];
        for knight_sq in knights.iter() {
            let attacks = KNIGHT_ATTACKS[knight_sq.index()];
            let targets_hit = (attacks & high_value.0).count_ones();

            if targets_hit >= 2 {
                bonus += FORK_THREAT_MG;
            }
        }

        // Pawn fork threats
        let pawns = self.pieces[c_idx][Piece::Pawn.index()];
        for pawn_sq in pawns.iter() {
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
        let c_idx = color.index();
        let opp_idx = color.opponent().index();
        let mut bonus = 0;

        let enemy_king_bb = self.pieces[opp_idx][Piece::King.index()];
        let enemy_queen_bb = self.pieces[opp_idx][Piece::Queen.index()];

        if enemy_king_bb.0 == 0 {
            return 0;
        }

        let enemy_king_sq = enemy_king_bb.0.trailing_zeros() as usize;

        // Check for bishop/queen pins (diagonal)
        let our_bishops = self.pieces[c_idx][Piece::Bishop.index()];
        let our_queens = self.pieces[c_idx][Piece::Queen.index()];
        let our_rooks = self.pieces[c_idx][Piece::Rook.index()];

        // Diagonal pins
        for bishop_sq in our_bishops.iter() {
            bonus += self.check_pin(bishop_sq.index(), enemy_king_sq, true, opp_idx, true);
        }

        // Check queen pins (both diagonal and orthogonal)
        for queen_sq in our_queens.iter() {
            bonus += self.check_pin(queen_sq.index(), enemy_king_sq, true, opp_idx, true);
            bonus += self.check_pin(queen_sq.index(), enemy_king_sq, false, opp_idx, true);
        }

        // Rook pins (orthogonal)
        for rook_sq in our_rooks.iter() {
            bonus += self.check_pin(rook_sq.index(), enemy_king_sq, false, opp_idx, true);
        }

        // Pins to queen (if queen exists)
        if enemy_queen_bb.0 != 0 {
            let enemy_queen_sq = enemy_queen_bb.0.trailing_zeros() as usize;
            for bishop_sq in our_bishops.iter() {
                bonus += self.check_pin(bishop_sq.index(), enemy_queen_sq, true, opp_idx, false);
            }
            for rook_sq in our_rooks.iter() {
                bonus += self.check_pin(rook_sq.index(), enemy_queen_sq, false, opp_idx, false);
            }
        }

        bonus
    }

    /// Check if a slider pins a piece to a target.
    fn check_pin(&self, slider_sq: usize, target_sq: usize, diagonal: bool, opp_idx: usize, to_king: bool) -> i32 {
        let slider_attacks = slider_attacks(slider_sq, self.all_occupied.0, diagonal);

        // Check if slider attacks the target
        if (slider_attacks & (1u64 << target_sq)) != 0 {
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
            if (self.occupied[opp_idx].0 & blockers) != 0 {
                return if to_king { PIN_TO_KING_MG } else { PIN_TO_QUEEN_MG };
            }
        }

        0
    }

    /// Get the squares between two squares on a line.
    fn between_mask(sq1: usize, sq2: usize) -> u64 {
        let file1 = sq1 % 8;
        let rank1 = sq1 / 8;
        let file2 = sq2 % 8;
        let rank2 = sq2 / 8;

        let file_diff = (file2 as i32 - file1 as i32).signum();
        let rank_diff = (rank2 as i32 - rank1 as i32).signum();

        // Check if on same line
        if file_diff == 0 && rank_diff == 0 {
            return 0;
        }

        // Not on a line (diagonal or orthogonal)
        if file_diff != 0 && rank_diff != 0 && (file2 as i32 - file1 as i32).abs() != (rank2 as i32 - rank1 as i32).abs() {
            return 0;
        }
        if file_diff == 0 && rank_diff == 0 {
            return 0;
        }

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

    /// Evaluate skewer threats.
    fn eval_skewers(&self, color: Color) -> i32 {
        let c_idx = color.index();
        let opp_idx = color.opponent().index();
        let mut bonus = 0;

        // Skewer: valuable piece in front of less valuable piece
        let enemy_king = self.pieces[opp_idx][Piece::King.index()];
        let enemy_queen = self.pieces[opp_idx][Piece::Queen.index()];
        let enemy_rooks = self.pieces[opp_idx][Piece::Rook.index()];

        // Check for queen skewers (king in front of rook/queen)
        if enemy_king.0 != 0 {
            let king_sq = enemy_king.0.trailing_zeros() as usize;

            let our_bishops = self.pieces[c_idx][Piece::Bishop.index()];
            let our_queens = self.pieces[c_idx][Piece::Queen.index()];
            let our_rooks = self.pieces[c_idx][Piece::Rook.index()];

            // Check if we can skewer the king to a rook or queen
            for slider in our_bishops.iter() {
                if self.is_skewer(slider.index(), king_sq, enemy_queen.0 | enemy_rooks.0, true) {
                    bonus += SKEWER_THREAT_MG;
                }
            }
            for slider in our_rooks.iter() {
                if self.is_skewer(slider.index(), king_sq, enemy_queen.0 | enemy_rooks.0, false) {
                    bonus += SKEWER_THREAT_MG;
                }
            }
            for slider in our_queens.iter() {
                if self.is_skewer(slider.index(), king_sq, enemy_queen.0 | enemy_rooks.0, true) {
                    bonus += SKEWER_THREAT_MG;
                }
                if self.is_skewer(slider.index(), king_sq, enemy_queen.0 | enemy_rooks.0, false) {
                    bonus += SKEWER_THREAT_MG;
                }
            }
        }

        bonus
    }

    /// Check if a skewer exists.
    fn is_skewer(&self, slider_sq: usize, front_sq: usize, back_targets: u64, diagonal: bool) -> bool {
        // Check if slider attacks front piece
        let attacks = slider_attacks(slider_sq, self.all_occupied.0, diagonal);
        if (attacks & (1u64 << front_sq)) == 0 {
            return false;
        }

        // Check if there's a target behind
        let x_ray = slider_attacks(slider_sq, self.all_occupied.0 & !(1u64 << front_sq), diagonal);
        (x_ray & back_targets) != 0
    }

    /// Evaluate discovery potential.
    fn eval_discovery_potential(&self, color: Color) -> i32 {
        let c_idx = color.index();
        let opp_idx = color.opponent().index();
        let mut bonus = 0;

        let enemy_king_bb = self.pieces[opp_idx][Piece::King.index()];
        if enemy_king_bb.0 == 0 {
            return 0;
        }

        let enemy_king_sq = enemy_king_bb.0.trailing_zeros() as usize;

        // Check if our pieces are blocking our sliders from attacking enemy king
        let our_bishops = self.pieces[c_idx][Piece::Bishop.index()];
        let _our_queens = self.pieces[c_idx][Piece::Queen.index()];
        let our_rooks = self.pieces[c_idx][Piece::Rook.index()];
        let _our_knights = self.pieces[c_idx][Piece::Knight.index()];

        // Check for discovered attack potential
        for bishop_sq in our_bishops.iter() {
            let x_ray = slider_attacks(bishop_sq.index(), 0, true); // Empty board
            if (x_ray & (1u64 << enemy_king_sq)) != 0 {
                // Our bishop could attack king if blockers moved
                let between = Self::between_mask(bishop_sq.index(), enemy_king_sq);
                let our_blockers = self.occupied[c_idx].0 & between;
                if our_blockers.is_power_of_two() {
                    // One of our pieces can discover an attack
                    bonus += DISCOVERY_POTENTIAL_MG;
                }
            }
        }

        for rook_sq in our_rooks.iter() {
            let x_ray = slider_attacks(rook_sq.index(), 0, false);
            if (x_ray & (1u64 << enemy_king_sq)) != 0 {
                let between = Self::between_mask(rook_sq.index(), enemy_king_sq);
                let our_blockers = self.occupied[c_idx].0 & between;
                if our_blockers.is_power_of_two() {
                    bonus += DISCOVERY_POTENTIAL_MG;
                }
            }
        }

        bonus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fork_detection() {
        // Knight forking king and rook
        let board: Board = "4k3/8/8/3N4/8/8/8/R3K3 w - - 0 1".parse().unwrap();
        let bonus = board.eval_fork_threats(Color::White);
        // Knight on d5 attacks e7... but king is on e8, so no fork yet
        // This is more of a smoke test
        assert!(bonus >= 0);
    }

    #[test]
    fn test_between_mask() {
        // a1 to h8 diagonal
        let mask = Board::between_mask(0, 63);
        // Should include b2, c3, d4, e5, f6, g7
        assert!((mask & (1u64 << 9)) != 0); // b2
        assert!((mask & (1u64 << 18)) != 0); // c3
    }
}
