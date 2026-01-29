//! Space and territory control evaluation.
//!
//! Implements:
//! - Territory control (safe squares in enemy half)
//! - Central control (bonus for controlling e4/d4/e5/d5)
//! - Expansion potential (pawn breaks available)

use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

use super::helpers::AttackContext;

/// Space bonus per controlled square in enemy territory
pub const SPACE_BONUS_MG: i32 = 2;
pub const SPACE_BONUS_EG: i32 = 1;

/// Central control bonus (e4, d4, e5, d5)
pub const CENTER_CONTROL_MG: i32 = 10;
pub const CENTER_CONTROL_EG: i32 = 5;

/// Extended center (c3-f3-f6-c6)
pub const EXTENDED_CENTER_MG: i32 = 3;

/// Pawn break potential bonus
pub const PAWN_BREAK_MG: i32 = 5;

/// Center squares mask (e4, d4, e5, d5)
const CENTER_SQUARES: u64 = 0x0000_0018_1800_0000;

/// Extended center (c3-f6 rectangle)
const EXTENDED_CENTER: u64 = 0x0000_3C3C_3C3C_0000;

impl Board {
    /// Evaluate space and territory control.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_space_control(&self, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let (w_mg, w_eg) = self.eval_space_for_color(Color::White, ctx);
        let (b_mg, b_eg) = self.eval_space_for_color(Color::Black, ctx);

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        (mg, eg)
    }

    fn eval_space_for_color(&self, color: Color, ctx: &AttackContext) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let _c_idx = color.index();
        let our_attacks = ctx.all_attacks(color);
        let enemy_attacks = ctx.all_attacks(color.opponent());

        // Safe squares: attacked by us, not attacked by enemy
        let safe_squares = Bitboard(our_attacks.0 & !enemy_attacks.0);

        // Territory control - safe squares in enemy half
        let enemy_half = match color {
            Color::White => 0xFFFF_FFFF_0000_0000u64, // Ranks 5-8
            Color::Black => 0x0000_0000_FFFF_FFFFu64, // Ranks 1-4
        };

        let space_count = (safe_squares.0 & enemy_half).count_ones() as i32;
        mg += space_count * SPACE_BONUS_MG;
        eg += space_count * SPACE_BONUS_EG;

        // Central control
        let _center_control = (our_attacks.0 & CENTER_SQUARES).count_ones() as i32;
        let safe_center = (safe_squares.0 & CENTER_SQUARES).count_ones() as i32;

        // Extra bonus for safely controlling center
        mg += safe_center * CENTER_CONTROL_MG;
        eg += safe_center * CENTER_CONTROL_EG;

        // Extended center control
        let extended_control = (safe_squares.0 & EXTENDED_CENTER & !CENTER_SQUARES).count_ones() as i32;
        mg += extended_control * EXTENDED_CENTER_MG;

        // Pawn break potential
        mg += self.eval_pawn_breaks(color);

        (mg, eg)
    }

    /// Evaluate available pawn breaks.
    fn eval_pawn_breaks(&self, color: Color) -> i32 {
        let c_idx = color.index();
        let opp_idx = color.opponent().index();

        let own_pawns = self.pieces[c_idx][Piece::Pawn.index()];
        let enemy_pawns = self.pieces[opp_idx][Piece::Pawn.index()];

        let mut breaks = 0;

        for pawn_sq in own_pawns.iter() {
            let file = pawn_sq.index() % 8;
            let rank = pawn_sq.index() / 8;

            // Check if pawn can capture to create a break
            let capture_sqs = match color {
                Color::White => {
                    if rank >= 7 {
                        continue;
                    }
                    let mut caps = 0u64;
                    if file > 0 {
                        caps |= 1u64 << (pawn_sq.index() + 7);
                    }
                    if file < 7 {
                        caps |= 1u64 << (pawn_sq.index() + 9);
                    }
                    caps
                }
                Color::Black => {
                    if rank <= 0 {
                        continue;
                    }
                    let mut caps = 0u64;
                    if file > 0 {
                        caps |= 1u64 << (pawn_sq.index() - 9);
                    }
                    if file < 7 {
                        caps |= 1u64 << (pawn_sq.index() - 7);
                    }
                    caps
                }
            };

            // Check if capture is available (enemy pawn present)
            if (enemy_pawns.0 & capture_sqs) != 0 {
                // This is a potential pawn break
                // More valuable if it opens lines toward center or enemy king
                let in_center = (capture_sqs & EXTENDED_CENTER) != 0;
                if in_center {
                    breaks += 2; // Central breaks worth more
                } else {
                    breaks += 1;
                }
            }
        }

        breaks * PAWN_BREAK_MG
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_control() {
        // Position with good central control for white
        let board: Board = "rnbqkbnr/pppppppp/8/8/3PP3/8/PPP2PPP/RNBQKBNR w KQkq - 0 1"
            .parse()
            .unwrap();
        let ctx = super::super::helpers::AttackContext::new(&board);
        let (mg, _) = board.eval_space_control(&ctx);
        // White should have positive space advantage
        assert!(mg > 0);
    }

    #[test]
    fn test_pawn_breaks() {
        // Position with available pawn break
        let board: Board = "8/8/3p4/2P5/8/8/8/8 w - - 0 1".parse().unwrap();
        let breaks = board.eval_pawn_breaks(Color::White);
        assert!(breaks > 0);
    }
}
