//! King tropism evaluation.
//!
//! Evaluates piece proximity to the enemy king.
//! Queens and rooks get bonuses for being close to the enemy king.

use crate::board::state::Board;
use crate::board::types::{Color, Piece, Square};

use super::tables::{QUEEN_TROPISM_MG, ROOK_TROPISM_MG};

/// Maximum Manhattan distance on a chess board
const MAX_MANHATTAN_DISTANCE: i32 = 14;

impl Board {
    /// Evaluate king tropism (piece proximity to enemy king).
    /// Returns middlegame score from white's perspective (tropism is mainly a MG concept).
    #[must_use]
    pub fn eval_tropism(&self) -> i32 {
        let mut score = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let enemy_king = Square::from_index(self.king_square_index(color.opponent()));

            // Queen tropism - closer is better
            for sq in self.pieces_of(color, Piece::Queen).iter() {
                let distance = sq.manhattan_distance(enemy_king);
                // Max distance is 14 (corner to corner), min is 1
                // Bonus = (14 - distance) * factor / 7
                let bonus = ((MAX_MANHATTAN_DISTANCE - distance) * QUEEN_TROPISM_MG) / 7;
                score += sign * bonus;
            }

            // Rook tropism - closer is better (smaller bonus)
            for sq in self.pieces_of(color, Piece::Rook).iter() {
                let distance = sq.manhattan_distance(enemy_king);
                let bonus = ((MAX_MANHATTAN_DISTANCE - distance) * ROOK_TROPISM_MG) / 7;
                score += sign * bonus;
            }
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tropism_starting_position() {
        let board = Board::new();
        let score = board.eval_tropism();
        // Starting position should be roughly equal
        assert!(
            score.abs() < 20,
            "tropism should be near 0 in start: {score}"
        );
    }

    #[test]
    fn test_queen_closer_to_enemy_king() {
        // White queen close to black king
        let board1: Board = "4k3/8/8/8/8/4Q3/8/4K3 w - - 0 1".parse().unwrap();
        let score1 = board1.eval_tropism();

        // White queen far from black king
        let board2: Board = "4k3/8/8/8/8/8/8/Q3K3 w - - 0 1".parse().unwrap();
        let score2 = board2.eval_tropism();

        assert!(score1 > score2, "closer queen should have higher tropism");
    }

    #[test]
    fn test_rook_tropism() {
        // White rook close to black king
        let board: Board = "4k3/8/4R3/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        let score = board.eval_tropism();
        // Rook close to enemy king should give bonus
        assert!(score > 0, "rook near enemy king should give bonus: {score}");
    }

    #[test]
    fn test_symmetric_position() {
        // Symmetric position - both queens equidistant
        let board: Board = "4k3/8/4q3/8/8/4Q3/8/4K3 w - - 0 1".parse().unwrap();
        let score = board.eval_tropism();
        // Should be close to 0 due to symmetry
        assert!(
            score.abs() < 10,
            "symmetric position should be near 0: {score}"
        );
    }
}
