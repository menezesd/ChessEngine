//! Structural imbalances evaluation.
//!
//! Implements:
//! - Knight vs Bishop by pawn count
//! - Rook pair vs Queen
//! - Minor piece pair values

use crate::board::state::Board;
use crate::board::types::{Color, Piece};

/// Knight preference bonus per pawn on board (knights better with more pawns)
pub const KNIGHT_PAWN_BONUS: i32 = 3;

/// Bishop preference in open positions (fewer pawns)
pub const BISHOP_OPEN_BONUS: i32 = 5;

/// Two rooks vs queen adjustment
pub const ROOK_PAIR_VS_QUEEN_MG: i32 = 10;
pub const ROOK_PAIR_VS_QUEEN_EG: i32 = 20;

/// Two bishops bonus (already in base eval, but adjusted here)
pub const BISHOP_PAIR_EXTRA_MG: i32 = 5;
pub const BISHOP_PAIR_EXTRA_EG: i32 = 10;

/// Two knights slight penalty (they don't coordinate as well)
pub const KNIGHT_PAIR_PENALTY_MG: i32 = -5;

impl Board {
    /// Evaluate structural imbalances.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_imbalances(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let (w_mg, w_eg) = self.eval_imbalances_for_color(Color::White);
        let (b_mg, b_eg) = self.eval_imbalances_for_color(Color::Black);

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        // Cross-side imbalances
        let (cross_mg, cross_eg) = self.eval_cross_imbalances();
        mg += cross_mg;
        eg += cross_eg;

        (mg, eg)
    }

    fn eval_imbalances_for_color(&self, color: Color) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let c_idx = color.index();

        let knights = self.pieces[c_idx][Piece::Knight.index()].0.count_ones();
        let bishops = self.pieces[c_idx][Piece::Bishop.index()].0.count_ones();

        // Minor piece pair adjustments
        if knights >= 2 {
            mg += KNIGHT_PAIR_PENALTY_MG;
        }

        if bishops >= 2 {
            // Extra bishop pair bonus (on top of base eval)
            mg += BISHOP_PAIR_EXTRA_MG;
            eg += BISHOP_PAIR_EXTRA_EG;
        }

        // Knight vs bishop based on pawn count
        let total_pawns = self.pieces[0][Piece::Pawn.index()].0.count_ones()
            + self.pieces[1][Piece::Pawn.index()].0.count_ones();

        // Knights are better with more pawns (closed positions)
        // Bishops are better with fewer pawns (open positions)
        let pawn_factor = total_pawns as i32 - 8; // Neutral at 8 pawns

        if knights > 0 && bishops == 0 {
            // Only knights - bonus in closed positions
            mg += knights as i32 * pawn_factor * KNIGHT_PAWN_BONUS / 4;
        } else if bishops > 0 && knights == 0 {
            // Only bishops - bonus in open positions
            mg -= pawn_factor * BISHOP_OPEN_BONUS / 4;
            eg -= pawn_factor * BISHOP_OPEN_BONUS / 4;
        }

        (mg, eg)
    }

    /// Evaluate cross-side imbalances (comparing piece compositions).
    fn eval_cross_imbalances(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let white_queens = self.pieces[0][Piece::Queen.index()].0.count_ones();
        let black_queens = self.pieces[1][Piece::Queen.index()].0.count_ones();
        let white_rooks = self.pieces[0][Piece::Rook.index()].0.count_ones();
        let black_rooks = self.pieces[1][Piece::Rook.index()].0.count_ones();

        // Two rooks vs queen
        // White has 2 rooks, black has queen (and no rooks)
        if white_rooks >= 2 && black_queens >= 1 && white_queens == 0 && black_rooks == 0 {
            mg += ROOK_PAIR_VS_QUEEN_MG;
            eg += ROOK_PAIR_VS_QUEEN_EG;
        }

        // Black has 2 rooks, white has queen
        if black_rooks >= 2 && white_queens >= 1 && black_queens == 0 && white_rooks == 0 {
            mg -= ROOK_PAIR_VS_QUEEN_MG;
            eg -= ROOK_PAIR_VS_QUEEN_EG;
        }

        // Bishop vs knight imbalances
        let white_knights = self.pieces[0][Piece::Knight.index()].0.count_ones();
        let black_knights = self.pieces[1][Piece::Knight.index()].0.count_ones();
        let white_bishops = self.pieces[0][Piece::Bishop.index()].0.count_ones();
        let black_bishops = self.pieces[1][Piece::Bishop.index()].0.count_ones();

        // Minor piece imbalance (one side has bishop, other has knight)
        // Value depends on pawn structure
        let total_pawns = self.pieces[0][Piece::Pawn.index()].0.count_ones()
            + self.pieces[1][Piece::Pawn.index()].0.count_ones();

        // In very open positions (few pawns), bishop > knight
        // In very closed positions (many pawns), knight might be better
        if total_pawns <= 6 {
            // Open position - bishop advantage
            if white_bishops > black_bishops && white_knights < black_knights {
                // White has bishop advantage
                mg += 10;
                eg += 15;
            } else if black_bishops > white_bishops && black_knights < white_knights {
                // Black has bishop advantage
                mg -= 10;
                eg -= 15;
            }
        }

        (mg, eg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bishop_pair_bonus() {
        // Position with bishop pair for white
        let board: Board = "8/8/8/8/8/8/8/2BB4 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_imbalances_for_color(Color::White);
        assert!(mg > 0 || eg > 0); // Should have some bonus
    }

    #[test]
    fn test_knight_closed_position() {
        // Closed position with many pawns
        let board: Board = "8/pppppppp/8/8/8/8/PPPPPPPP/2N5 w - - 0 1".parse().unwrap();
        let (mg, _) = board.eval_imbalances_for_color(Color::White);
        // Knight should be valued more in closed position
        assert!(mg >= 0);
    }
}
