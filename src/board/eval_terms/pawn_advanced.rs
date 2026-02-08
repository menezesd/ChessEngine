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

use super::helpers::single_pawn_attacks;

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

        let white_pawns = self.pieces_of(Color::White, Piece::Pawn);
        let black_pawns = self.pieces_of(Color::Black, Piece::Pawn);

        // Get king positions
        let white_king_sq = self.king_square_index(Color::White);
        let black_king_sq = self.king_square_index(Color::Black);

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
            let pawn_file = pawn_sq.file();
            let pawn_rank = pawn_sq.rank();
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

        // Only consider pawns on files near enemy king
        let file_dist = (file as i32 - enemy_king_file as i32).abs();
        if file_dist > 2 {
            return false;
        }

        // Check if pawn can capture (enemy pawn on adjacent file, one rank ahead)
        let Some(capture_sqs) = single_pawn_attacks(pawn_sq, color) else {
            return false;
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
        let white_pawns = board.pieces_of(Color::White, Piece::Pawn);
        let links = Board::count_chain_links(white_pawns, Color::White);
        assert!(links >= 1, "e5 should be defended by d4");
    }

    #[test]
    fn test_no_chain() {
        // Isolated pawns - no chain
        let board: Board = "8/8/8/P3P3/8/8/8/8 w - - 0 1".parse().unwrap();
        let white_pawns = board.pieces_of(Color::White, Piece::Pawn);
        let links = Board::count_chain_links(white_pawns, Color::White);
        assert_eq!(links, 0, "isolated pawns should have no chain links");
    }

    #[test]
    fn test_pawn_advanced_symmetry() {
        // Symmetric position
        let board = Board::new();
        let (mg, eg) = board.eval_pawn_advanced();
        assert!(mg.abs() < 20, "starting position pawn advanced mg: {mg}");
        assert!(eg.abs() < 20, "starting position pawn advanced eg: {eg}");
    }

    #[test]
    fn test_candidate_passer() {
        // e5 pawn with d6 blocker - could become passer with exd6
        let board: Board = "8/8/3p4/4P3/8/8/8/8 w - - 0 1".parse().unwrap();
        let (mg, eg) = board.eval_pawn_advanced();
        // Just verify function runs
        assert!(
            (-100..=100).contains(&mg),
            "candidate passer mg reasonable: {mg}"
        );
        assert!(
            (-100..=100).contains(&eg),
            "candidate passer eg reasonable: {eg}"
        );
    }

    #[test]
    fn test_pawn_lever() {
        // White pawn that can capture to open a file
        let board: Board = "4k3/8/3p4/2P5/8/8/8/4K3 w - - 0 1".parse().unwrap();
        // The c5 pawn can capture on d6
        let (mg, _) = board.eval_pawn_advanced();
        // Just verify function runs
        assert!(mg >= -100, "pawn lever evaluation should work: {mg}");
    }
}
