//! Endgame pattern evaluation.
//!
//! Implements:
//! - Rook endgame activity (rook cutting off king)
//! - King centralization (stronger in endgame)
//! - Wrong bishop detection (can't control rook pawn promotion square)
//! - Single-pawn rook endings (king access and frontal blockade)

use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece, Square};

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
        (
            0,
            self.eval_endgame_for_color(Color::White) - self.eval_endgame_for_color(Color::Black),
        )
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

    /// Static score from White's perspective for one-pawn rook endings.
    pub(in crate::board) fn single_pawn_rook_eval(&self) -> Option<i32> {
        let piece_count = self.all_occupied.popcount();
        if !matches!(piece_count, 4 | 5) {
            return None;
        }
        let pawn = self.all_pieces_of_type(Piece::Pawn).iter().next()?;
        let pawn_side = self.color_on(pawn)?;
        let defender = pawn_side.opponent();
        let defending_rook = self.pieces_of(defender, Piece::Rook).iter().next()?;
        if (piece_count == 5 && self.pieces_of(pawn_side, Piece::Rook).is_empty())
            || self.piece_count(pawn_side, Piece::King) != 1
            || self.piece_count(defender, Piece::King) != 1
            || !(1..=6).contains(&pawn.rank())
        {
            return None;
        }
        let white_pawn = pawn_side == Color::White;
        let next_square = pawn.forward(white_pawn)?;
        let distance_to_advance = |king: Square| {
            king.file()
                .abs_diff(next_square.file())
                .max(king.rank().abs_diff(next_square.rank())) as i32
        };
        let relative_rank = |rank| if white_pawn { rank } else { 7 - rank };
        let pawn_rank = relative_rank(pawn.rank());
        let frontal_rook = defending_rook.file() == pawn.file()
            && relative_rank(defending_rook.rank()) > pawn_rank;
        let defending_king = self.find_king(defender);
        let defender_distance = distance_to_advance(defending_king);
        let pawn_king_distance = distance_to_advance(self.find_king(pawn_side));

        if piece_count == 4 {
            let supported_advance = pawn_king_distance <= 1 && defender_distance >= 2;
            let threat = (pawn_rank as i32 - 3).max(0) * if supported_advance { 65 } else { 25 };
            let rook_score = 420
                - 22 * pawn_rank as i32
                - threat
                - 13 * (defender_distance - pawn_king_distance)
                + 35 * i32::from(frontal_rook);
            return Some(defender.sign() * rook_score.max(0));
        }

        // The extra pawn is worth more when its king can escort it.
        // A rook in front of the pawn can slow its advance from a distance.
        let supporting_rook = self.pieces_of(pawn_side, Piece::Rook).iter().next()?;
        let rook_behind_pawn = supporting_rook.file() == pawn.file()
            && relative_rank(supporting_rook.rank()) < pawn_rank;
        let king_blocks_pawn = defending_king.file().abs_diff(pawn.file()) <= 1
            && relative_rank(defending_king.rank()) > pawn_rank;
        let score = 35 + 13 * pawn_rank as i32 + 22 * (defender_distance - pawn_king_distance)
            - 55 * i32::from(frontal_rook)
            - 32 * i32::from(king_blocks_pawn)
            + 20 * i32::from(rook_behind_pawn);
        Some(pawn_side.sign() * score)
    }
}

#[cfg(test)]
mod tests;
