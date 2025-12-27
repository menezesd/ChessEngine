//! King safety evaluation.
//!
//! Evaluates king safety using attack units and pawn shield.

use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS};
use crate::board::masks::{FILES, KING_ATTACK_TABLE, KING_ZONE_EXTENDED, PAWN_SHIELD_MASK};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Piece};

use super::helpers::AttackContext;
use super::tables::{
    ATTACK_WEIGHTS, KING_OPEN_FILE_MG, KING_SEMI_OPEN_FILE_MG, KING_SHIELD_BONUS_MG,
    QUEEN_CHECK_BONUS,
};

impl Board {
    /// Evaluate king safety using attack units.
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_king_safety(&self) -> (i32, i32) {
        let ctx = self.compute_attack_context();
        self.eval_king_safety_with_context(&ctx)
    }

    /// Evaluate king safety using pre-computed attack context.
    #[must_use]
    pub fn eval_king_safety_with_context(&self, ctx: &AttackContext) -> (i32, i32) {
        self.eval_king_safety_inner(ctx.white_pawn_attacks, ctx.black_pawn_attacks)
    }

    /// Internal king safety evaluation with pawn attacks.
    fn eval_king_safety_inner(
        &self,
        white_pawn_attacks: Bitboard,
        black_pawn_attacks: Bitboard,
    ) -> (i32, i32) {
        let mut mg = 0;

        for color_idx in 0..2 {
            let sign = if color_idx == 0 { 1 } else { -1 };
            let enemy_idx = 1 - color_idx;

            // Find our king
            let king_bb = self.pieces[color_idx][Piece::King.index()];
            if king_bb.is_empty() {
                continue;
            }
            let king_sq_idx = king_bb.0.trailing_zeros() as usize;

            // Get king zone
            let king_zone = KING_ZONE_EXTENDED[color_idx][king_sq_idx];

            // Get pawn defenses of king zone
            let our_pawn_attacks = if color_idx == 0 {
                white_pawn_attacks
            } else {
                black_pawn_attacks
            };

            // Accumulate attack units
            let mut attack_units = 0i32;

            // Pre-compute king's slider attacks for queen check threat detection
            // (computed once per king, not per queen)
            let king_diag_attacks = slider_attacks(king_sq_idx, self.all_occupied.0, true);
            let king_straight_attacks = slider_attacks(king_sq_idx, self.all_occupied.0, false);
            let king_queen_rays = king_diag_attacks | king_straight_attacks;

            // Knight attacks on king zone
            for sq_idx in self.pieces[enemy_idx][Piece::Knight.index()].iter() {
                let attacks =
                    crate::board::Bitboard(KNIGHT_ATTACKS[sq_idx.index()] & king_zone.0);
                if !attacks.is_empty() {
                    let defended = (attacks.0 & our_pawn_attacks.0).count_ones() as i32;
                    let undefended = attacks.popcount() as i32 - defended;
                    attack_units += ATTACK_WEIGHTS[Piece::Knight.index()].0 * undefended
                        + ATTACK_WEIGHTS[Piece::Knight.index()].1 * defended;
                }
            }

            // Bishop attacks on king zone
            for sq_idx in self.pieces[enemy_idx][Piece::Bishop.index()].iter() {
                let moves = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
                let attacks = crate::board::Bitboard(moves & king_zone.0);
                if !attacks.is_empty() {
                    let defended = (attacks.0 & our_pawn_attacks.0).count_ones() as i32;
                    let undefended = attacks.popcount() as i32 - defended;
                    attack_units += ATTACK_WEIGHTS[Piece::Bishop.index()].0 * undefended
                        + ATTACK_WEIGHTS[Piece::Bishop.index()].1 * defended;
                }
            }

            // Rook attacks on king zone
            for sq_idx in self.pieces[enemy_idx][Piece::Rook.index()].iter() {
                let moves = slider_attacks(sq_idx.index(), self.all_occupied.0, false);
                let attacks = crate::board::Bitboard(moves & king_zone.0);
                if !attacks.is_empty() {
                    let defended = (attacks.0 & our_pawn_attacks.0).count_ones() as i32;
                    let undefended = attacks.popcount() as i32 - defended;
                    attack_units += ATTACK_WEIGHTS[Piece::Rook.index()].0 * undefended
                        + ATTACK_WEIGHTS[Piece::Rook.index()].1 * defended;
                }
            }

            // Queen attacks on king zone
            for sq_idx in self.pieces[enemy_idx][Piece::Queen.index()].iter() {
                let diag = slider_attacks(sq_idx.index(), self.all_occupied.0, true);
                let straight = slider_attacks(sq_idx.index(), self.all_occupied.0, false);
                let moves = diag | straight;
                let attacks = crate::board::Bitboard(moves & king_zone.0);
                if !attacks.is_empty() {
                    let defended = (attacks.0 & our_pawn_attacks.0).count_ones() as i32;
                    let undefended = attacks.popcount() as i32 - defended;
                    attack_units += ATTACK_WEIGHTS[Piece::Queen.index()].0 * undefended
                        + ATTACK_WEIGHTS[Piece::Queen.index()].1 * defended;

                    // Queen check threats (uses pre-computed king rays)
                    if (moves & king_queen_rays) != 0 {
                        attack_units += QUEEN_CHECK_BONUS;
                    }
                }
            }

            // Convert attack units to score (negative for the side being attacked)
            let attack_score = KING_ATTACK_TABLE[attack_units.min(255) as usize];
            mg -= sign * attack_score; // Subtract because we're evaluating attacks AGAINST us
        }

        // King safety is primarily a middlegame concern
        (mg, 0)
    }

    /// Evaluate king pawn shield.
    /// Returns `(middlegame_score, endgame_score)` from white's perspective.
    #[must_use]
    pub fn eval_king_shield(&self) -> (i32, i32) {
        let mut mg = 0;

        for color_idx in 0..2 {
            let sign = if color_idx == 0 { 1 } else { -1 };

            // Find our king
            let king_bb = self.pieces[color_idx][Piece::King.index()];
            if king_bb.is_empty() {
                continue;
            }
            let king_sq_idx = king_bb.0.trailing_zeros() as usize;
            let king_file = king_sq_idx % 8;

            // Get pawn shield mask based on king file
            let shield_mask = PAWN_SHIELD_MASK[color_idx][king_file];
            let our_pawns = self.pieces[color_idx][Piece::Pawn.index()];

            // Count shield pawns
            let shield_pawns = (shield_mask.0 & our_pawns.0).count_ones() as i32;
            mg += sign * shield_pawns * KING_SHIELD_BONUS_MG;

            // Penalize open files near king
            let file_mask = FILES[king_file];
            let our_pawns_on_file = (file_mask.0 & our_pawns.0) != 0;
            let enemy_pawns_on_file =
                (file_mask.0 & self.pieces[1 - color_idx][Piece::Pawn.index()].0) != 0;

            if !our_pawns_on_file && !enemy_pawns_on_file {
                mg += sign * KING_OPEN_FILE_MG;
            } else if !our_pawns_on_file {
                mg += sign * KING_SEMI_OPEN_FILE_MG;
            }

            // Check adjacent files too (half penalty for adjacent files)
            let enemy_pawns = self.pieces[1 - color_idx][Piece::Pawn.index()];
            for &adj_file_idx in &[king_file.wrapping_sub(1), king_file + 1] {
                if adj_file_idx < 8 {
                    let adj_file = FILES[adj_file_idx];
                    let our_adj = (adj_file.0 & our_pawns.0) != 0;
                    let enemy_adj = (adj_file.0 & enemy_pawns.0) != 0;
                    if !our_adj && !enemy_adj {
                        mg += sign * (KING_OPEN_FILE_MG / 2);
                    } else if !our_adj {
                        mg += sign * (KING_SEMI_OPEN_FILE_MG / 2);
                    }
                }
            }
        }

        (mg, 0)
    }
}
