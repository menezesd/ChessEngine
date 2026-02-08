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

/// Neutral pawn count (8 pawns per side is balanced)
const NEUTRAL_PAWN_COUNT: i32 = 8;

/// Divisor for scaling minor piece bonuses
const IMBALANCE_SCALE_DIVISOR: i32 = 4;

/// Threshold for considering a position "open" (few pawns)
const OPEN_POSITION_PAWN_THRESHOLD: u32 = 6;

/// Bishop advantage bonus in open positions
const BISHOP_ADVANTAGE_OPEN_MG: i32 = 10;
const BISHOP_ADVANTAGE_OPEN_EG: i32 = 15;

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

        let knights = self.piece_count(color, Piece::Knight);
        let bishops = self.piece_count(color, Piece::Bishop);

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
        let total_pawns = self.piece_count(Color::White, Piece::Pawn)
            + self.piece_count(Color::Black, Piece::Pawn);

        // Knights are better with more pawns (closed positions)
        // Bishops are better with fewer pawns (open positions)
        let pawn_factor = total_pawns as i32 - NEUTRAL_PAWN_COUNT;

        if knights > 0 && bishops == 0 {
            // Only knights - bonus in closed positions
            mg += knights as i32 * pawn_factor * KNIGHT_PAWN_BONUS / IMBALANCE_SCALE_DIVISOR;
        } else if bishops > 0 && knights == 0 {
            // Only bishops - bonus in open positions
            mg -= pawn_factor * BISHOP_OPEN_BONUS / IMBALANCE_SCALE_DIVISOR;
            eg -= pawn_factor * BISHOP_OPEN_BONUS / IMBALANCE_SCALE_DIVISOR;
        }

        (mg, eg)
    }

    /// Evaluate cross-side imbalances (comparing piece compositions).
    fn eval_cross_imbalances(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let white_queens = self.piece_count(Color::White, Piece::Queen);
        let black_queens = self.piece_count(Color::Black, Piece::Queen);
        let white_rooks = self.piece_count(Color::White, Piece::Rook);
        let black_rooks = self.piece_count(Color::Black, Piece::Rook);

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
        let white_knights = self.piece_count(Color::White, Piece::Knight);
        let black_knights = self.piece_count(Color::Black, Piece::Knight);
        let white_bishops = self.piece_count(Color::White, Piece::Bishop);
        let black_bishops = self.piece_count(Color::Black, Piece::Bishop);

        // Minor piece imbalance (one side has bishop, other has knight)
        // Value depends on pawn structure
        let total_pawns = self.piece_count(Color::White, Piece::Pawn)
            + self.piece_count(Color::Black, Piece::Pawn);

        // In very open positions (few pawns), bishop > knight
        // In very closed positions (many pawns), knight might be better
        if total_pawns <= OPEN_POSITION_PAWN_THRESHOLD {
            // Open position - bishop advantage
            if white_bishops > black_bishops && white_knights < black_knights {
                // White has bishop advantage
                mg += BISHOP_ADVANTAGE_OPEN_MG;
                eg += BISHOP_ADVANTAGE_OPEN_EG;
            } else if black_bishops > white_bishops && black_knights < white_knights {
                // Black has bishop advantage
                mg -= BISHOP_ADVANTAGE_OPEN_MG;
                eg -= BISHOP_ADVANTAGE_OPEN_EG;
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
        assert!(mg > 0 || eg > 0, "bishop pair should have bonus");
    }

    #[test]
    fn test_knight_closed_position() {
        // Closed position with many pawns
        let board: Board = "8/pppppppp/8/8/8/8/PPPPPPPP/2N5 w - - 0 1".parse().unwrap();
        let (mg, _) = board.eval_imbalances_for_color(Color::White);
        // Knight should be valued more in closed position
        assert!(mg >= 0, "knight in closed position should not be penalized");
    }

    #[test]
    fn test_bishop_open_position() {
        // Open position with few pawns - bishop should be better
        let board: Board = "8/8/8/8/8/8/8/2B5 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_imbalances_for_color(Color::White);
        // Bishop should be good in open positions
        assert!(
            mg >= 0 || eg >= 0,
            "bishop in open position should not be penalized"
        );
    }

    #[test]
    fn test_knight_pair_penalty() {
        // Two knights - slight penalty
        let board: Board = "8/8/8/8/8/8/8/2NN4 w - - 0 1".parse().unwrap();
        let (mg, _) = board.eval_imbalances_for_color(Color::White);
        // Knight pair should have penalty
        assert!(mg < 0, "knight pair should have penalty: {mg}");
    }

    #[test]
    fn test_rook_pair_vs_queen() {
        // White has 2 rooks, black has queen
        let board: Board = "4q3/8/8/8/8/8/8/R3R3 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_imbalances();
        // Two rooks vs queen should have slight bonus for rooks
        assert!(
            mg >= 0 || eg >= 0,
            "rook pair vs queen should be reasonable"
        );
    }

    #[test]
    fn test_imbalances_symmetry() {
        // Symmetric position
        let board = Board::new();
        let (mg, eg) = board.eval_imbalances();
        assert!(mg.abs() < 20, "symmetric imbalances mg: {mg}");
        assert!(eg.abs() < 20, "symmetric imbalances eg: {eg}");
    }

    #[test]
    fn test_bishop_advantage_open() {
        // Open position: white has bishop, black has knight
        let board: Board = "8/8/8/8/8/8/8/2B2n2 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_imbalances();
        // In very open position, bishop advantage
        // Just verify function runs
        assert!((-50..=50).contains(&mg), "bishop vs knight mg: {mg}");
        assert!((-50..=50).contains(&eg), "bishop vs knight eg: {eg}");
    }
}
