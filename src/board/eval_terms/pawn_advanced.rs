//! Advanced pawn evaluation terms.
//!
//! Implements:
//! - Pawn storm threat (pawns advancing toward enemy king)
//! - Pawn levers (pawns that can capture to open lines)
//! - Candidate passers (pawns that can become passed with one push)
//! - Pawn chains (connected diagonal pawn structures)

use crate::board::masks::{relative_rank, PASSED_PAWN_MASK};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};


/// Pawn storm bonus by rank (relative to enemy king)
/// Higher bonus for pawns closer to enemy king
pub const PAWN_STORM_BONUS: [i32; 8] = [0, 0, 0, 5, 10, 20, 0, 0];

/// Candidate passer bonus (can become passed with one push)
pub const CANDIDATE_PASSER_MG: i32 = 8;
pub const CANDIDATE_PASSER_EG: i32 = 12;

/// Pawn chain bonus per chain link
pub const PAWN_CHAIN_MG: i32 = 3;
pub const PAWN_CHAIN_EG: i32 = 2;

/// Pawn lever bonus (pawn can capture to open a file toward enemy king)
pub const PAWN_LEVER_MG: i32 = 5;

impl Board {
    /// Evaluate advanced pawn features.
    ///
    /// Returns (middlegame, endgame) score from white's perspective.
    #[must_use]
    pub fn eval_pawn_advanced(&self) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let white_pawns = self.pieces[0][Piece::Pawn.index()];
        let black_pawns = self.pieces[1][Piece::Pawn.index()];

        // Get king positions
        let white_king_sq = self.pieces[0][Piece::King.index()].0.trailing_zeros() as usize;
        let black_king_sq = self.pieces[1][Piece::King.index()].0.trailing_zeros() as usize;

        // Evaluate for both colors
        let (w_mg, w_eg) = self.eval_pawn_advanced_for_color(
            white_pawns,
            black_pawns,
            black_king_sq,
            Color::White,
        );
        let (b_mg, b_eg) = self.eval_pawn_advanced_for_color(
            black_pawns,
            white_pawns,
            white_king_sq,
            Color::Black,
        );

        mg += w_mg - b_mg;
        eg += w_eg - b_eg;

        (mg, eg)
    }

    fn eval_pawn_advanced_for_color(
        &self,
        own_pawns: Bitboard,
        enemy_pawns: Bitboard,
        enemy_king_sq: usize,
        color: Color,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let enemy_king_file = enemy_king_sq % 8;

        // Pawn storm - evaluate pawns on files near enemy king
        for pawn_sq in own_pawns.iter() {
            let pawn_file = pawn_sq.index() % 8;
            let pawn_rank = pawn_sq.index() / 8;
            let rel_rank = relative_rank(pawn_rank, color);

            // Check if pawn is on files near enemy king (within 2 files)
            let file_distance = (pawn_file as i32 - enemy_king_file as i32).abs();
            if file_distance <= 2 {
                // Pawn storm bonus
                mg += PAWN_STORM_BONUS[rel_rank];
            }

            // Candidate passer detection
            // A pawn is a candidate passer if it can become passed with one push
            // This means: no enemy pawn directly ahead, and we can push past blockers
            if self.is_candidate_passer(pawn_sq.index(), own_pawns, enemy_pawns, color) {
                mg += CANDIDATE_PASSER_MG;
                eg += CANDIDATE_PASSER_EG;
            }

            // Pawn lever detection
            // A lever is a pawn that can capture to open a file
            if Self::is_pawn_lever(pawn_sq.index(), enemy_pawns, enemy_king_file, color) {
                mg += PAWN_LEVER_MG;
            }
        }

        // Pawn chain evaluation
        let chain_bonus = Self::count_chain_links(own_pawns, color);
        mg += chain_bonus * PAWN_CHAIN_MG;
        eg += chain_bonus * PAWN_CHAIN_EG;

        (mg, eg)
    }

    /// Check if a pawn is a candidate passer
    fn is_candidate_passer(
        &self,
        pawn_sq: usize,
        _own_pawns: Bitboard,
        enemy_pawns: Bitboard,
        color: Color,
    ) -> bool {
        let rank = pawn_sq / 8;

        // Already a passed pawn? Not a candidate
        let passed_mask = PASSED_PAWN_MASK[color.index()][pawn_sq];
        if (enemy_pawns.0 & passed_mask.0) == 0 {
            return false; // Already passed
        }

        // Check if pushing one square would make it passed or give it a clear path
        let push_sq = match color {
            Color::White => {
                if rank >= 6 {
                    return false;
                }
                pawn_sq + 8
            }
            Color::Black => {
                if rank <= 1 {
                    return false;
                }
                pawn_sq - 8
            }
        };

        // Is the push square blocked?
        if (self.all_occupied.0 & (1u64 << push_sq)) != 0 {
            return false;
        }

        // After pushing, would it have fewer blockers?
        let new_passed_mask = PASSED_PAWN_MASK[color.index()][push_sq];
        let current_blockers = (enemy_pawns.0 & passed_mask.0).count_ones();
        let new_blockers = (enemy_pawns.0 & new_passed_mask.0).count_ones();

        // Candidate if pushing reduces blockers significantly or leaves just one
        new_blockers < current_blockers && new_blockers <= 1
    }

    /// Check if a pawn is a lever that can open lines toward enemy king
    fn is_pawn_lever(
        pawn_sq: usize,
        enemy_pawns: Bitboard,
        enemy_king_file: usize,
        color: Color,
    ) -> bool {
        let file = pawn_sq % 8;
        let rank = pawn_sq / 8;

        // Only consider pawns on files near enemy king
        let file_dist = (file as i32 - enemy_king_file as i32).abs();
        if file_dist > 2 {
            return false;
        }

        // Check if pawn can capture (enemy pawn on adjacent file, one rank ahead)
        let capture_sqs = match color {
            Color::White => {
                if rank >= 7 {
                    return false;
                }
                let mut caps = 0u64;
                if file > 0 {
                    caps |= 1u64 << (pawn_sq + 7);
                }
                if file < 7 {
                    caps |= 1u64 << (pawn_sq + 9);
                }
                caps
            }
            Color::Black => {
                if rank == 0 {
                    return false;
                }
                let mut caps = 0u64;
                if file > 0 {
                    caps |= 1u64 << (pawn_sq - 9);
                }
                if file < 7 {
                    caps |= 1u64 << (pawn_sq - 7);
                }
                caps
            }
        };

        (enemy_pawns.0 & capture_sqs) != 0
    }

    /// Count pawn chain links (pawns defended by other pawns diagonally)
    fn count_chain_links(pawns: Bitboard, color: Color) -> i32 {
        // A chain link is a pawn defended by another pawn
        let defenders = match color {
            Color::White => {
                // Pawns that can defend (shifted down-left and down-right)
                let left = (pawns.0 >> 7) & !Bitboard::FILE_A.0;
                let right = (pawns.0 >> 9) & !Bitboard::FILE_H.0;
                Bitboard(left | right)
            }
            Color::Black => {
                let left = (pawns.0 << 9) & !Bitboard::FILE_A.0;
                let right = (pawns.0 << 7) & !Bitboard::FILE_H.0;
                Bitboard(left | right)
            }
        };

        // Count pawns that are defended by other pawns
        (pawns.0 & defenders.0).count_ones() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pawn_storm() {
        // Position with white pawns storming black's kingside castle
        let board: Board = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/4P3/3P1N2/PPPB1PPP/RN1QKB1R w KQkq - 0 1"
            .parse()
            .unwrap();
        let (mg, eg) = board.eval_pawn_advanced();
        // Just verify it runs without panic
        let _ = (mg, eg); // Use both values to avoid warnings
    }

    #[test]
    fn test_chain_links() {
        // Classic pawn chain d4-e5
        let board: Board = "8/8/8/4P3/3P4/8/8/8 w - - 0 1".parse().unwrap();
        let white_pawns = board.pieces[0][Piece::Pawn.index()];
        let links = Board::count_chain_links(white_pawns, Color::White);
        assert!(links >= 1); // e5 is defended by d4
    }
}
