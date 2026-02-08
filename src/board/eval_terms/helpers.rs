//! Helper methods for evaluation.
//!
//! Contains attack computation methods used by multiple evaluation terms.

use crate::board::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS};
use crate::board::state::Board;
use crate::board::types::{Bitboard, Color, Piece};

/// Cached attack information for both colors.
///
/// Computing attacks is expensive, so we cache them here to avoid
/// redundant calculations across multiple evaluation terms.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
pub struct AttackContext {
    /// All squares attacked by white pieces
    pub white_attacks: Bitboard,
    /// All squares attacked by black pieces
    pub black_attacks: Bitboard,
    /// Squares attacked by white pawns only
    pub white_pawn_attacks: Bitboard,
    /// Squares attacked by black pawns only
    pub black_pawn_attacks: Bitboard,
}

impl AttackContext {
    /// Get all attacks for a color
    #[inline]
    pub fn all_attacks(&self, color: Color) -> Bitboard {
        match color {
            Color::White => self.white_attacks,
            Color::Black => self.black_attacks,
        }
    }

    /// Get pawn attacks for a color
    #[inline]
    pub fn pawn_attacks(&self, color: Color) -> Bitboard {
        match color {
            Color::White => self.white_pawn_attacks,
            Color::Black => self.black_pawn_attacks,
        }
    }
}

/// Compute attack squares for a single pawn.
///
/// Returns `None` if the pawn cannot attack (on promotion/first rank).
/// This is a standalone function to avoid code duplication in eval terms.
#[inline]
#[must_use]
pub fn single_pawn_attacks(sq: usize, color: Color) -> Option<u64> {
    let file = sq % 8;
    let rank = sq / 8;

    match color {
        Color::White => {
            if rank >= 7 {
                return None;
            }
            let mut attacks = 0u64;
            if file > 0 {
                attacks |= 1u64 << (sq + 7);
            }
            if file < 7 {
                attacks |= 1u64 << (sq + 9);
            }
            Some(attacks)
        }
        Color::Black => {
            if rank == 0 {
                return None;
            }
            let mut attacks = 0u64;
            if file > 0 {
                attacks |= 1u64 << (sq - 9);
            }
            if file < 7 {
                attacks |= 1u64 << (sq - 7);
            }
            Some(attacks)
        }
    }
}

impl Board {
    /// Get all squares attacked by pawns of a color.
    #[must_use]
    pub fn pawn_attacks(&self, color: Color) -> Bitboard {
        let pawns = self.pieces_of(color, Piece::Pawn);
        match color {
            Color::White => {
                let left = (pawns.0 << 7) & !Bitboard::FILE_H.0;
                let right = (pawns.0 << 9) & !Bitboard::FILE_A.0;
                Bitboard(left | right)
            }
            Color::Black => {
                let left = (pawns.0 >> 9) & !Bitboard::FILE_H.0;
                let right = (pawns.0 >> 7) & !Bitboard::FILE_A.0;
                Bitboard(left | right)
            }
        }
    }

    /// Get all squares attacked by any piece of a color.
    #[must_use]
    pub fn all_attacks(&self, color: Color) -> Bitboard {
        let mut attacks = self.pawn_attacks(color);

        // Knight attacks
        for sq_idx in self.pieces_of(color, Piece::Knight).iter() {
            attacks.0 |= KNIGHT_ATTACKS[sq_idx.index()];
        }

        // Bishop attacks
        for sq_idx in self.pieces_of(color, Piece::Bishop).iter() {
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, true);
        }

        // Rook attacks
        for sq_idx in self.pieces_of(color, Piece::Rook).iter() {
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, false);
        }

        // Queen attacks
        for sq_idx in self.pieces_of(color, Piece::Queen).iter() {
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, true);
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, false);
        }

        // King attacks
        for sq_idx in self.pieces_of(color, Piece::King).iter() {
            attacks.0 |= KING_ATTACKS[sq_idx.index()];
        }

        attacks
    }

    /// Compute attack context for all evaluation terms.
    ///
    /// This computes attacks once and caches them for use by all evaluation functions.
    #[must_use]
    pub fn compute_attack_context(&self) -> AttackContext {
        let white_pawn_attacks = self.pawn_attacks(Color::White);
        let black_pawn_attacks = self.pawn_attacks(Color::Black);
        let white_attacks = self.all_attacks(Color::White);
        let black_attacks = self.all_attacks(Color::Black);

        AttackContext {
            white_attacks,
            black_attacks,
            white_pawn_attacks,
            black_pawn_attacks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_pawn_attacks_white_center() {
        // e4 pawn attacks d5 and f5
        let attacks = single_pawn_attacks(28, Color::White).unwrap(); // e4 = index 28
        let d5 = 1u64 << 35; // d5
        let f5 = 1u64 << 37; // f5
        assert_eq!(attacks, d5 | f5);
    }

    #[test]
    fn test_single_pawn_attacks_white_a_file() {
        // a4 pawn only attacks b5
        let attacks = single_pawn_attacks(24, Color::White).unwrap(); // a4
        let b5 = 1u64 << 33;
        assert_eq!(attacks, b5);
    }

    #[test]
    fn test_single_pawn_attacks_black_center() {
        // e5 pawn attacks d4 and f4
        let attacks = single_pawn_attacks(36, Color::Black).unwrap(); // e5 = index 36
        let d4 = 1u64 << 27;
        let f4 = 1u64 << 29;
        assert_eq!(attacks, d4 | f4);
    }

    #[test]
    fn test_single_pawn_attacks_edge_cases() {
        // White pawn on 8th rank can't attack (already promoted)
        assert!(single_pawn_attacks(56, Color::White).is_none()); // a8

        // Black pawn on 1st rank can't attack (illegal position)
        assert!(single_pawn_attacks(0, Color::Black).is_none()); // a1
    }

    #[test]
    fn test_pawn_attacks_starting_position() {
        let board = Board::new();
        let white_attacks = board.pawn_attacks(Color::White);
        let black_attacks = board.pawn_attacks(Color::Black);

        // White pawns on rank 2 attack rank 3
        // Black pawns on rank 7 attack rank 6
        assert!(white_attacks.0 & (1u64 << 16) != 0); // a3 attacked by b2 pawn
        assert!(black_attacks.0 & (1u64 << 40) != 0); // a6 attacked by b7 pawn
    }

    #[test]
    fn test_all_attacks_starting_position() {
        let board = Board::new();
        let white_attacks = board.all_attacks(Color::White);
        let black_attacks = board.all_attacks(Color::Black);

        // Both sides attack many squares in starting position
        assert!(white_attacks.popcount() > 16);
        assert!(black_attacks.popcount() > 16);
    }

    #[test]
    fn test_attack_context_symmetry() {
        let board = Board::new();
        let ctx = board.compute_attack_context();

        // Starting position should be symmetric
        assert_eq!(ctx.white_attacks.popcount(), ctx.black_attacks.popcount());
        assert_eq!(
            ctx.white_pawn_attacks.popcount(),
            ctx.black_pawn_attacks.popcount()
        );
    }

    #[test]
    fn test_knight_attacks_in_all_attacks() {
        // Board with just a knight
        let board: Board = "8/8/8/8/4N3/8/8/8 w - - 0 1".parse().unwrap();
        let attacks = board.all_attacks(Color::White);

        // Knight on e4 attacks d2, f2, c3, g3, c5, g5, d6, f6
        assert_eq!(attacks.popcount(), 8);
    }
}
