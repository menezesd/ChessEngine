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

        attacks.0 |= self.knight_attacks_for(color);
        attacks.0 |= self.slider_attacks_for(color, Piece::Bishop, true);
        attacks.0 |= self.slider_attacks_for(color, Piece::Rook, false);
        attacks.0 |= self.queen_attacks_for(color);
        attacks.0 |= self.king_attacks_for(color);

        attacks
    }

    fn knight_attacks_for(&self, color: Color) -> u64 {
        let mut attacks = 0;
        for sq in self.pieces_of(color, Piece::Knight).iter() {
            attacks |= KNIGHT_ATTACKS[sq.index()];
        }
        attacks
    }

    fn slider_attacks_for(&self, color: Color, piece: Piece, bishop: bool) -> u64 {
        let mut attacks = 0;
        for sq in self.pieces_of(color, piece).iter() {
            attacks |= slider_attacks(sq.index(), self.all_occupied.0, bishop);
        }
        attacks
    }

    fn queen_attacks_for(&self, color: Color) -> u64 {
        let mut attacks = 0;
        for sq in self.pieces_of(color, Piece::Queen).iter() {
            attacks |= slider_attacks(sq.index(), self.all_occupied.0, true);
            attacks |= slider_attacks(sq.index(), self.all_occupied.0, false);
        }
        attacks
    }

    fn king_attacks_for(&self, color: Color) -> u64 {
        let mut attacks = 0;
        for sq in self.pieces_of(color, Piece::King).iter() {
            attacks |= KING_ATTACKS[sq.index()];
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
mod tests;
