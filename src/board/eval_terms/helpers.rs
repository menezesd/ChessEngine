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

impl Board {
    /// Get all squares attacked by pawns of a color.
    #[must_use]
    pub fn pawn_attacks(&self, color: Color) -> Bitboard {
        let pawns = self.pieces[color.index()][Piece::Pawn.index()];
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
        let c_idx = color.index();
        let mut attacks = self.pawn_attacks(color);

        // Knight attacks
        for sq_idx in self.pieces[c_idx][Piece::Knight.index()].iter() {
            attacks.0 |= KNIGHT_ATTACKS[sq_idx.index()];
        }

        // Bishop attacks
        for sq_idx in self.pieces[c_idx][Piece::Bishop.index()].iter() {
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, true);
        }

        // Rook attacks
        for sq_idx in self.pieces[c_idx][Piece::Rook.index()].iter() {
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, false);
        }

        // Queen attacks
        for sq_idx in self.pieces[c_idx][Piece::Queen.index()].iter() {
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, true);
            attacks.0 |= slider_attacks(sq_idx.index(), self.all_occupied.0, false);
        }

        // King attacks
        for sq_idx in self.pieces[c_idx][Piece::King.index()].iter() {
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
