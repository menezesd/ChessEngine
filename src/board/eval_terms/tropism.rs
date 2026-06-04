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
mod tests;
