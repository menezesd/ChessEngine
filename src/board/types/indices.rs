//! Type-safe indices for colors and pieces.
//!
//! These newtypes prevent accidentally mixing color and piece indices,
//! which could lead to subtle bugs in array access patterns.

use super::piece::{Color, Piece};

/// Type-safe index for color arrays (0 = White, 1 = Black).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColorIndex(usize);

impl ColorIndex {
    /// Index for White
    pub const WHITE: ColorIndex = ColorIndex(0);
    /// Index for Black
    pub const BLACK: ColorIndex = ColorIndex(1);

    /// Create from a Color
    #[inline]
    #[must_use]
    pub const fn from_color(color: Color) -> Self {
        match color {
            Color::White => Self::WHITE,
            Color::Black => Self::BLACK,
        }
    }

    /// Get the opponent's color index
    #[inline]
    #[must_use]
    pub const fn opponent(self) -> Self {
        ColorIndex(1 - self.0)
    }

    /// Convert to usize for array indexing
    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Convert back to Color
    #[inline]
    #[must_use]
    pub const fn to_color(self) -> Color {
        match self.0 {
            0 => Color::White,
            _ => Color::Black,
        }
    }

    /// Iterate over both colors
    #[must_use = "iterators are lazy and do nothing unless consumed"]
    pub fn iter() -> impl Iterator<Item = ColorIndex> {
        [Self::WHITE, Self::BLACK].into_iter()
    }
}

impl From<Color> for ColorIndex {
    fn from(color: Color) -> Self {
        Self::from_color(color)
    }
}

impl From<ColorIndex> for usize {
    fn from(idx: ColorIndex) -> usize {
        idx.0
    }
}

/// Type-safe index for piece arrays (0-5 for Pawn through King).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PieceIndex(usize);

impl PieceIndex {
    /// Index for Pawn
    pub const PAWN: PieceIndex = PieceIndex(0);
    /// Index for Knight
    pub const KNIGHT: PieceIndex = PieceIndex(1);
    /// Index for Bishop
    pub const BISHOP: PieceIndex = PieceIndex(2);
    /// Index for Rook
    pub const ROOK: PieceIndex = PieceIndex(3);
    /// Index for Queen
    pub const QUEEN: PieceIndex = PieceIndex(4);
    /// Index for King
    pub const KING: PieceIndex = PieceIndex(5);

    /// All piece indices in order
    pub const ALL: [PieceIndex; 6] = [
        Self::PAWN,
        Self::KNIGHT,
        Self::BISHOP,
        Self::ROOK,
        Self::QUEEN,
        Self::KING,
    ];

    /// Non-pawn piece indices
    pub const NON_PAWN: [PieceIndex; 5] = [
        Self::KNIGHT,
        Self::BISHOP,
        Self::ROOK,
        Self::QUEEN,
        Self::KING,
    ];

    /// Create from a Piece
    #[inline]
    #[must_use]
    pub const fn from_piece(piece: Piece) -> Self {
        PieceIndex(piece.index())
    }

    /// Convert to usize for array indexing
    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Convert back to Piece
    #[inline]
    #[must_use]
    pub const fn to_piece(self) -> Piece {
        match self.0 {
            0 => Piece::Pawn,
            1 => Piece::Knight,
            2 => Piece::Bishop,
            3 => Piece::Rook,
            4 => Piece::Queen,
            _ => Piece::King,
        }
    }

    /// Iterate over all piece types
    #[must_use = "iterators are lazy and do nothing unless consumed"]
    pub fn iter() -> impl Iterator<Item = PieceIndex> {
        Self::ALL.into_iter()
    }
}

impl From<Piece> for PieceIndex {
    fn from(piece: Piece) -> Self {
        Self::from_piece(piece)
    }
}

impl From<PieceIndex> for usize {
    fn from(idx: PieceIndex) -> usize {
        idx.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_index_round_trip() {
        assert_eq!(ColorIndex::WHITE.to_color(), Color::White);
        assert_eq!(ColorIndex::BLACK.to_color(), Color::Black);
        assert_eq!(ColorIndex::from_color(Color::White), ColorIndex::WHITE);
        assert_eq!(ColorIndex::from_color(Color::Black), ColorIndex::BLACK);
    }

    #[test]
    fn test_color_index_opponent() {
        assert_eq!(ColorIndex::WHITE.opponent(), ColorIndex::BLACK);
        assert_eq!(ColorIndex::BLACK.opponent(), ColorIndex::WHITE);
    }

    #[test]
    fn test_piece_index_round_trip() {
        for piece in [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            let idx = PieceIndex::from_piece(piece);
            assert_eq!(idx.to_piece(), piece);
        }
    }
}
