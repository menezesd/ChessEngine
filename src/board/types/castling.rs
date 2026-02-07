//! Castling rights type.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::piece::Color;

pub(crate) const CASTLE_WHITE_K: u8 = 1 << 0;
pub(crate) const CASTLE_WHITE_Q: u8 = 1 << 1;
pub(crate) const CASTLE_BLACK_K: u8 = 1 << 2;
pub(crate) const CASTLE_BLACK_Q: u8 = 1 << 3;

/// All castling rights combined
pub(crate) const ALL_CASTLING_RIGHTS: u8 =
    CASTLE_WHITE_K | CASTLE_WHITE_Q | CASTLE_BLACK_K | CASTLE_BLACK_Q;

/// Castling rights represented as a bitmask
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CastlingRights(u8);

impl CastlingRights {
    /// No castling rights
    #[must_use]
    pub const fn none() -> Self {
        CastlingRights(0)
    }

    /// All castling rights (both sides can castle kingside and queenside)
    #[must_use]
    pub const fn all() -> Self {
        CastlingRights(ALL_CASTLING_RIGHTS)
    }

    /// Check if a specific castling right is set
    #[inline]
    #[must_use]
    pub const fn has(self, color: Color, kingside: bool) -> bool {
        let bit = Self::bit_for(color, kingside);
        self.0 & bit != 0
    }

    /// Set a specific castling right
    #[inline]
    pub fn set(&mut self, color: Color, kingside: bool) {
        self.0 |= Self::bit_for(color, kingside);
    }

    /// Remove a specific castling right
    #[inline]
    pub fn remove(&mut self, color: Color, kingside: bool) {
        self.0 &= !Self::bit_for(color, kingside);
    }

    /// Get the raw bitmask value (for Zobrist hashing)
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Create from raw bitmask value
    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        CastlingRights(value)
    }

    /// Get the bit for a specific castling right
    #[inline]
    const fn bit_for(color: Color, kingside: bool) -> u8 {
        match (color, kingside) {
            (Color::White, true) => CASTLE_WHITE_K,
            (Color::White, false) => CASTLE_WHITE_Q,
            (Color::Black, true) => CASTLE_BLACK_K,
            (Color::Black, false) => CASTLE_BLACK_Q,
        }
    }
}

pub(crate) fn castle_bit(color: Color, side: char) -> u8 {
    match (color, side) {
        (Color::White, 'K') => CASTLE_WHITE_K,
        (Color::White, 'Q') => CASTLE_WHITE_Q,
        (Color::Black, 'K') => CASTLE_BLACK_K,
        (Color::Black, 'Q') => CASTLE_BLACK_Q,
        _ => 0,
    }
}
