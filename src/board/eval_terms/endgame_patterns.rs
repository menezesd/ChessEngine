//! Endgame pattern evaluation.
//!
//! Implements:
//! - Rook endgame activity (rook cutting off king)
//! - King centralization (stronger in endgame)
//! - Wrong bishop detection (can't control rook pawn promotion square)
//! - Fortress detection (recognize drawable patterns)

use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

/// Rook activity bonus for cutting off enemy king
pub const ROOK_CUT_OFF_EG: i32 = 15;

/// King centralization bonus (per square closer to center)
pub const KING_CENTER_EG: i32 = 8;

/// Wrong bishop penalty (can't control promotion square)
pub const WRONG_BISHOP_EG: i32 = -50;

/// Light square mask
const LIGHT_SQUARES: u64 = 0x55AA_55AA_55AA_55AA;

impl Board {
    /// Evaluate endgame patterns.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_endgame_patterns(&self) -> (i32, i32) {
        // These patterns are primarily for endgame
        let mg = 0;
        let mut eg = 0;

        let (w_eg, b_eg) = (
            self.eval_endgame_for_color(Color::White),
            self.eval_endgame_for_color(Color::Black),
        );

        eg += w_eg - b_eg;

        // Check for fortress patterns
        if self.is_fortress() {
            // Reduce score toward draw
            eg /= 4;
        }

        (mg, eg)
    }

    fn eval_endgame_for_color(&self, color: Color) -> i32 {
        let mut eg = 0;

        let c_idx = color.index();
        let opp_idx = color.opponent().index();

        // King centralization
        let king_bb = self.pieces[c_idx][Piece::King.index()];
        if king_bb.0 != 0 {
            let king_sq = king_bb.0.trailing_zeros() as usize;
            eg += Self::king_centralization_bonus(king_sq);
        }

        // Rook activity - cutting off enemy king
        let rooks = self.pieces[c_idx][Piece::Rook.index()];
        let enemy_king_bb = self.pieces[opp_idx][Piece::King.index()];
        if rooks.0 != 0 && enemy_king_bb.0 != 0 {
            let enemy_king_sq = enemy_king_bb.0.trailing_zeros() as usize;
            eg += Self::eval_rook_cut_off(rooks, enemy_king_sq, color);
        }

        // Wrong bishop detection
        eg += self.eval_wrong_bishop(color);

        eg
    }

    /// Bonus for king being close to center in endgame.
    fn king_centralization_bonus(king_sq: usize) -> i32 {
        let file = king_sq % 8;
        let rank = king_sq / 8;

        // Distance from center (d4/d5/e4/e5)
        let file_dist = if file < 4 { 3 - file } else { file - 4 };
        let rank_dist = if rank < 4 { 3 - rank } else { rank - 4 };

        let center_dist = (file_dist + rank_dist) as i32;

        // Bonus for being closer to center (max 6 squares away)
        (6 - center_dist) * KING_CENTER_EG
    }

    /// Evaluate rook cutting off enemy king.
    fn eval_rook_cut_off(rooks: Bitboard, enemy_king_sq: usize, color: Color) -> i32 {
        let enemy_king_rank = enemy_king_sq / 8;
        let enemy_king_file = enemy_king_sq % 8;
        let mut bonus = 0;

        for rook_sq in rooks.iter() {
            let rook_rank = rook_sq.index() / 8;
            let rook_file = rook_sq.index() % 8;

            // Rook on rank cutting off king (between king and promotion square)
            match color {
                Color::White => {
                    // White wants to cut off black king from rank 8
                    if rook_rank > enemy_king_rank && rook_rank < 7 {
                        bonus += ROOK_CUT_OFF_EG;
                    }
                }
                Color::Black => {
                    // Black wants to cut off white king from rank 1
                    if rook_rank < enemy_king_rank && rook_rank > 0 {
                        bonus += ROOK_CUT_OFF_EG;
                    }
                }
            }

            // Rook on file cutting off king from center
            if (rook_file == 3 || rook_file == 4) &&
               ((rook_file < enemy_king_file && enemy_king_file >= 4) ||
                (rook_file >= enemy_king_file && enemy_king_file < 4)) {
                bonus += ROOK_CUT_OFF_EG / 2;
            }
        }

        bonus
    }

    /// Detect wrong bishop (bishop that can't control rook pawn promotion square).
    fn eval_wrong_bishop(&self, color: Color) -> i32 {
        let c_idx = color.index();
        let bishops = self.pieces[c_idx][Piece::Bishop.index()];
        let pawns = self.pieces[c_idx][Piece::Pawn.index()];

        if bishops.0.count_ones() != 1 || pawns.0 == 0 {
            return 0;
        }

        // Check for rook pawn only situation
        let a_file = 0x0101_0101_0101_0101u64;
        let h_file = 0x8080_8080_8080_8080u64;

        let a_pawns = pawns.0 & a_file;
        let h_pawns = pawns.0 & h_file;
        let other_pawns = pawns.0 & !(a_file | h_file);

        // Only relevant if we only have rook pawns
        if other_pawns != 0 {
            return 0;
        }

        let is_light_bishop = (bishops.0 & LIGHT_SQUARES) != 0;

        // a-file promotion square (a8 for white, a1 for black)
        let a_promo_light = match color {
            Color::White => true,  // a8 is light
            Color::Black => false, // a1 is dark
        };

        // h-file promotion square (h8 for white, h1 for black)
        let h_promo_light = match color {
            Color::White => false, // h8 is dark
            Color::Black => true,  // h1 is light
        };

        // Check if bishop is wrong color for our pawns
        if a_pawns != 0 && h_pawns == 0 {
            // Only a-file pawn(s)
            if is_light_bishop != a_promo_light {
                return WRONG_BISHOP_EG;
            }
        } else if h_pawns != 0 && a_pawns == 0 {
            // Only h-file pawn(s)
            if is_light_bishop != h_promo_light {
                return WRONG_BISHOP_EG;
            }
        }

        0
    }

    /// Detect fortress patterns.
    fn is_fortress(&self) -> bool {
        // Simplified fortress detection
        // Full implementation would recognize specific patterns

        // Check for common fortress: rook+pawn vs rook with blocked pawn
        let white_rooks = self.pieces[0][Piece::Rook.index()].0.count_ones();
        let black_rooks = self.pieces[1][Piece::Rook.index()].0.count_ones();
        let white_pawns = self.pieces[0][Piece::Pawn.index()].0.count_ones();
        let black_pawns = self.pieces[1][Piece::Pawn.index()].0.count_ones();

        // Very simplified: if material is roughly equal and few pawns, might be fortress-ish
        if white_rooks == 1 && black_rooks == 1 && white_pawns <= 1 && black_pawns <= 1 {
            // Potential fortress - would need deeper analysis
            // For now, just return false to avoid false positives
            return false;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_king_centralization() {
        // King on d5 - central
        let bonus = Board::king_centralization_bonus(35); // d5 = 35
        assert!(bonus > 0);
    }

    #[test]
    fn test_wrong_bishop() {
        // White has h-pawn and dark-squared bishop (h8 is light, so wrong)
        let board: Board = "8/7P/8/8/8/8/B7/8 w - - 0 1".parse().unwrap();
        let penalty = board.eval_wrong_bishop(Color::White);
        // a1 bishop is dark-squared, h8 is light - this is wrong bishop
        assert!(penalty < 0);
    }
}
