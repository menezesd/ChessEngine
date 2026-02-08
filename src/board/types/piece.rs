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

    /// Minor and major pieces (Knight, Bishop, Rook, Queen) - excludes Pawn and King
    pub const MINOR_AND_MAJOR: [Piece; 4] =
        [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen];

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

#[cfg(test)]
mod tests {
    use super::*;

    // Piece tests
    #[test]
    fn test_piece_index() {
        assert_eq!(Piece::Pawn.index(), 0);
        assert_eq!(Piece::Knight.index(), 1);
        assert_eq!(Piece::Bishop.index(), 2);
        assert_eq!(Piece::Rook.index(), 3);
        assert_eq!(Piece::Queen.index(), 4);
        assert_eq!(Piece::King.index(), 5);
    }

    #[test]
    fn test_piece_from_char() {
        assert_eq!(Piece::from_char('p'), Some(Piece::Pawn));
        assert_eq!(Piece::from_char('N'), Some(Piece::Knight));
        assert_eq!(Piece::from_char('b'), Some(Piece::Bishop));
        assert_eq!(Piece::from_char('R'), Some(Piece::Rook));
        assert_eq!(Piece::from_char('q'), Some(Piece::Queen));
        assert_eq!(Piece::from_char('K'), Some(Piece::King));
        assert_eq!(Piece::from_char('x'), None);
    }

    #[test]
    fn test_piece_to_char() {
        assert_eq!(Piece::Pawn.to_char(), 'p');
        assert_eq!(Piece::Knight.to_char(), 'n');
        assert_eq!(Piece::Bishop.to_char(), 'b');
        assert_eq!(Piece::Rook.to_char(), 'r');
        assert_eq!(Piece::Queen.to_char(), 'q');
        assert_eq!(Piece::King.to_char(), 'k');
    }

    #[test]
    fn test_piece_to_fen_char() {
        assert_eq!(Piece::Pawn.to_fen_char(Color::White), 'P');
        assert_eq!(Piece::Pawn.to_fen_char(Color::Black), 'p');
        assert_eq!(Piece::Knight.to_fen_char(Color::White), 'N');
        assert_eq!(Piece::Queen.to_fen_char(Color::Black), 'q');
    }

    #[test]
    fn test_piece_value_ordering() {
        assert!(Piece::Pawn.value() < Piece::Knight.value());
        assert!(Piece::Knight.value() < Piece::Bishop.value());
        assert!(Piece::Bishop.value() < Piece::Rook.value());
        assert!(Piece::Rook.value() < Piece::Queen.value());
        assert!(Piece::Queen.value() < Piece::King.value());
    }

    #[test]
    fn test_piece_attacks_diagonally() {
        assert!(!Piece::Pawn.attacks_diagonally());
        assert!(!Piece::Knight.attacks_diagonally());
        assert!(Piece::Bishop.attacks_diagonally());
        assert!(!Piece::Rook.attacks_diagonally());
        assert!(Piece::Queen.attacks_diagonally());
        assert!(!Piece::King.attacks_diagonally());
    }

    #[test]
    fn test_piece_attacks_straight() {
        assert!(!Piece::Pawn.attacks_straight());
        assert!(!Piece::Knight.attacks_straight());
        assert!(!Piece::Bishop.attacks_straight());
        assert!(Piece::Rook.attacks_straight());
        assert!(Piece::Queen.attacks_straight());
        assert!(!Piece::King.attacks_straight());
    }

    #[test]
    fn test_piece_is_slider() {
        assert!(!Piece::Pawn.is_slider());
        assert!(!Piece::Knight.is_slider());
        assert!(Piece::Bishop.is_slider());
        assert!(Piece::Rook.is_slider());
        assert!(Piece::Queen.is_slider());
        assert!(!Piece::King.is_slider());
    }

    #[test]
    fn test_piece_all_array() {
        assert_eq!(Piece::ALL.len(), 6);
        for (i, piece) in Piece::ALL.iter().enumerate() {
            assert_eq!(piece.index(), i);
        }
    }

    #[test]
    fn test_piece_minor_and_major() {
        assert_eq!(Piece::MINOR_AND_MAJOR.len(), 4);
        assert!(!Piece::MINOR_AND_MAJOR.contains(&Piece::Pawn));
        assert!(!Piece::MINOR_AND_MAJOR.contains(&Piece::King));
    }

    // Color tests
    #[test]
    fn test_color_index() {
        assert_eq!(Color::White.index(), 0);
        assert_eq!(Color::Black.index(), 1);
    }

    #[test]
    fn test_color_opponent() {
        assert_eq!(Color::White.opponent(), Color::Black);
        assert_eq!(Color::Black.opponent(), Color::White);
    }

    #[test]
    fn test_color_sign() {
        assert_eq!(Color::White.sign(), 1);
        assert_eq!(Color::Black.sign(), -1);
    }

    #[test]
    fn test_color_back_rank() {
        assert_eq!(Color::White.back_rank(), 0);
        assert_eq!(Color::Black.back_rank(), 7);
    }

    #[test]
    fn test_color_pawn_direction() {
        assert_eq!(Color::White.pawn_direction(), 1);
        assert_eq!(Color::Black.pawn_direction(), -1);
    }

    #[test]
    fn test_color_pawn_start_rank() {
        assert_eq!(Color::White.pawn_start_rank(), 1);
        assert_eq!(Color::Black.pawn_start_rank(), 6);
    }

    #[test]
    fn test_color_pawn_promotion_rank() {
        assert_eq!(Color::White.pawn_promotion_rank(), 7);
        assert_eq!(Color::Black.pawn_promotion_rank(), 0);
    }

    #[test]
    fn test_color_display() {
        assert_eq!(format!("{}", Color::White), "White");
        assert_eq!(format!("{}", Color::Black), "Black");
    }

    #[test]
    fn test_color_both_array() {
        assert_eq!(Color::BOTH.len(), 2);
        assert_eq!(Color::BOTH[0], Color::White);
        assert_eq!(Color::BOTH[1], Color::Black);
    }
}
