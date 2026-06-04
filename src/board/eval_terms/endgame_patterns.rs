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

impl Board {
    /// Evaluate endgame patterns.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_endgame_patterns(&self) -> (i32, i32) {
        // These patterns are primarily for endgame
        let mg = 0;
        let mut eg = 0;

        for color in Color::BOTH {
            eg += color.sign() * self.eval_endgame_for_color(color);
        }

        // Check for fortress patterns
        if self.is_fortress() {
            // Reduce score toward draw
            eg /= 4;
        }

        (mg, eg)
    }

    fn eval_endgame_for_color(&self, color: Color) -> i32 {
        let mut eg = 0;

        // King centralization
        let king_sq = self.king_square_index(color);
        eg += Self::king_centralization_bonus(king_sq);

        // Rook activity - cutting off enemy king
        let rooks = self.pieces_of(color, Piece::Rook);
        if rooks.0 != 0 {
            let enemy_king_sq = self.king_square_index(color.opponent());
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
            let rook_rank = rook_sq.rank();
            let rook_file = rook_sq.file();

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
            if (rook_file == 3 || rook_file == 4)
                && ((rook_file < enemy_king_file && enemy_king_file >= 4)
                    || (rook_file >= enemy_king_file && enemy_king_file < 4))
            {
                bonus += ROOK_CUT_OFF_EG / 2;
            }
        }

        bonus
    }

    /// Whether a-file promotion square is light: [White (a8=light), Black (a1=dark)]
    const A_PROMO_LIGHT: [bool; 2] = [true, false];
    /// Whether h-file promotion square is light: [White (h8=dark), Black (h1=light)]
    const H_PROMO_LIGHT: [bool; 2] = [false, true];

    /// Detect wrong bishop (bishop that can't control rook pawn promotion square).
    fn eval_wrong_bishop(&self, color: Color) -> i32 {
        let bishops = self.pieces_of(color, Piece::Bishop);
        let pawns = self.pieces_of(color, Piece::Pawn);

        if bishops.popcount() != 1 || pawns.is_empty() {
            return 0;
        }

        // Check for rook pawn only situation
        let a_pawns = pawns.intersects(Bitboard::FILE_A);
        let h_pawns = pawns.intersects(Bitboard::FILE_H);
        let other_pawns = !pawns
            .and(Bitboard::FILE_A.or(Bitboard::FILE_H).not())
            .is_empty();

        // Only relevant if we only have rook pawns
        if other_pawns {
            return 0;
        }

        let is_light_bishop = bishops.intersects(Bitboard::LIGHT_SQUARES);
        let color_idx = color.index();

        // Check if bishop is wrong color for our pawns
        if a_pawns && !h_pawns {
            // Only a-file pawn(s)
            if is_light_bishop != Self::A_PROMO_LIGHT[color_idx] {
                return WRONG_BISHOP_EG;
            }
        } else if h_pawns && !a_pawns {
            // Only h-file pawn(s)
            if is_light_bishop != Self::H_PROMO_LIGHT[color_idx] {
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
        let white_rooks = self.piece_count(Color::White, Piece::Rook);
        let black_rooks = self.piece_count(Color::Black, Piece::Rook);
        let white_pawns = self.piece_count(Color::White, Piece::Pawn);
        let black_pawns = self.piece_count(Color::Black, Piece::Pawn);

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
mod tests;
