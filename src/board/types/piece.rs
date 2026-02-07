//! Piece and color types.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Chess piece types.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Piece {
    /// All piece types in index order
    pub const ALL: [Piece; 6] = [
        Piece::Pawn,
        Piece::Knight,
        Piece::Bishop,
        Piece::Rook,
        Piece::Queen,
        Piece::King,
    ];

    /// All piece types except King (useful for evaluation and hanging piece detection)
    pub const NON_KING: [Piece; 5] = [
        Piece::Pawn,
        Piece::Knight,
        Piece::Bishop,
        Piece::Rook,
        Piece::Queen,
    ];

    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        match self {
            Piece::Pawn => 0,
            Piece::Knight => 1,
            Piece::Bishop => 2,
            Piece::Rook => 3,
            Piece::Queen => 4,
            Piece::King => 5,
        }
    }

    /// Parse a piece from a lowercase character (p, n, b, r, q, k)
    #[must_use]
    pub fn from_char(c: char) -> Option<Piece> {
        match c.to_ascii_lowercase() {
            'p' => Some(Piece::Pawn),
            'n' => Some(Piece::Knight),
            'b' => Some(Piece::Bishop),
            'r' => Some(Piece::Rook),
            'q' => Some(Piece::Queen),
            'k' => Some(Piece::King),
            _ => None,
        }
    }

    /// Convert piece to lowercase character
    #[inline]
    #[must_use]
    pub const fn to_char(self) -> char {
        match self {
            Piece::Pawn => 'p',
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::Rook => 'r',
            Piece::Queen => 'q',
            Piece::King => 'k',
        }
    }

    /// Convert piece to character with case based on color (uppercase for White)
    #[inline]
    #[must_use]
    pub fn to_fen_char(self, color: Color) -> char {
        let c = self.to_char();
        if color == Color::White {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }

    /// Get the standard material value in centipawns.
    ///
    /// Returns approximate values: Pawn=100, Knight=320, Bishop=330,
    /// Rook=500, Queen=900, King=20000 (effectively infinite).
    #[inline]
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Piece::Pawn => 100,
            Piece::Knight => 320,
            Piece::Bishop => 330,
            Piece::Rook => 500,
            Piece::Queen => 900,
            Piece::King => 20000,
        }
    }

    /// Returns true if this piece can attack diagonally (Bishop, Queen)
    #[inline]
    #[must_use]
    pub const fn attacks_diagonally(self) -> bool {
        matches!(self, Piece::Bishop | Piece::Queen)
    }

    /// Returns true if this piece can attack along ranks/files (Rook, Queen)
    #[inline]
    #[must_use]
    pub const fn attacks_straight(self) -> bool {
        matches!(self, Piece::Rook | Piece::Queen)
    }

    /// Returns true if this piece is a slider (Bishop, Rook, Queen)
    #[inline]
    #[must_use]
    pub const fn is_slider(self) -> bool {
        matches!(self, Piece::Bishop | Piece::Rook | Piece::Queen)
    }
}

/// Promotion piece choices in order of typical preference (queen first)
pub(crate) const PROMOTION_PIECES: [Piece; 4] =
    [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];

/// Chess colors.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum Color {
    White,
    Black,
}

impl Color {
    /// Both colors in index order (White=0, Black=1)
    pub const BOTH: [Color; 2] = [Color::White, Color::Black];

    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }

    /// Returns the opposite color
    #[inline]
    #[must_use]
    pub(crate) const fn opponent(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// Scoring sign for evaluation (+1 for White, -1 for Black)
    #[inline]
    #[must_use]
    pub(crate) const fn sign(self) -> i32 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    /// Back rank for this color (0 for White, 7 for Black)
    #[inline]
    #[must_use]
    pub(crate) const fn back_rank(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 7,
        }
    }

    /// Pawn forward direction (+1 for White, -1 for Black)
    #[inline]
    #[must_use]
    pub(crate) const fn pawn_direction(self) -> isize {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    /// Pawn starting rank (1 for White, 6 for Black)
    #[inline]
    #[must_use]
    pub(crate) const fn pawn_start_rank(self) -> usize {
        match self {
            Color::White => 1,
            Color::Black => 6,
        }
    }

    /// Pawn promotion rank (7 for White, 0 for Black)
    #[inline]
    #[must_use]
    pub(crate) const fn pawn_promotion_rank(self) -> usize {
        match self {
            Color::White => 7,
            Color::Black => 0,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::White => write!(f, "White"),
            Color::Black => write!(f, "Black"),
        }
    }
}
