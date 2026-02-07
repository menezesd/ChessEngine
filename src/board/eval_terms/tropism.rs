//! King tropism evaluation.
//!
//! Evaluates piece proximity to the enemy king.
//! Queens and rooks get bonuses for being close to the enemy king.

use crate::board::state::Board;
use crate::board::types::{Color, Piece};

use super::tables::{QUEEN_TROPISM_MG, ROOK_TROPISM_MG};

impl Board {
    /// Evaluate king tropism (piece proximity to enemy king).
    /// Returns middlegame score from white's perspective (tropism is mainly a MG concept).
    #[must_use]
    pub fn eval_tropism(&self) -> i32 {
        let mut score = 0;

        for color in Color::BOTH {
            let sign = color.sign();
            let color_idx = color.index();
            let enemy_king_bb = self.pieces[color.opponent().index()][Piece::King.index()];

            if enemy_king_bb.is_empty() {
                continue;
            }

            let enemy_king_sq = enemy_king_bb.0.trailing_zeros() as usize;
            let king_rank = (enemy_king_sq / 8) as i32;
            let king_file = (enemy_king_sq % 8) as i32;

            // Queen tropism - closer is better
            for sq in self.pieces[color_idx][Piece::Queen.index()].iter() {
                let rank = sq.rank() as i32;
                let file = sq.file() as i32;
                let distance = (rank - king_rank).abs() + (file - king_file).abs();
                // Max distance is 14 (corner to corner), min is 1
                // Bonus = (14 - distance) * factor / 14
                let bonus = ((14 - distance) * QUEEN_TROPISM_MG) / 7;
                score += sign * bonus;
            }

            // Rook tropism - closer is better (smaller bonus)
            for sq in self.pieces[color_idx][Piece::Rook.index()].iter() {
                let rank = sq.rank() as i32;
                let file = sq.file() as i32;
                let distance = (rank - king_rank).abs() + (file - king_file).abs();
                let bonus = ((14 - distance) * ROOK_TROPISM_MG) / 7;
                score += sign * bonus;
            }
        }

        score
    }
}
