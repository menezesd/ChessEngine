//! Square types and utilities.

use std::fmt;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::board::error::SquareError;

pub(crate) fn file_to_index(file: char) -> usize {
    file as usize - ('a' as usize)
}

pub(crate) fn rank_to_index(rank: char) -> usize {
    (rank as usize) - ('0' as usize) - 1
}

/// A square on the chess board, stored as a compact 0-63 index.
///
/// Index layout: rank * 8 + file, where a1=0, b1=1, ..., h8=63.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Square(u8);

impl Square {
    /// Create a new square from rank and file (both 0-7).
    /// Does not perform bounds checking - use `try_new` for checked construction.
    #[inline]
    #[must_use]
    pub const fn new(rank: usize, file: usize) -> Self {
        Square((rank * 8 + file) as u8)
    }

    /// Create a new square with bounds checking
    #[must_use]
    pub const fn try_new(rank: usize, file: usize) -> Option<Self> {
        if rank < 8 && file < 8 {
            Some(Square::new(rank, file))
        } else {
            None
        }
    }

    /// Get the rank (0-7, where 0 = rank 1)
    #[inline]
    #[must_use]
    pub const fn rank(self) -> usize {
        (self.0 / 8) as usize
    }

    /// Get the file (0-7, where 0 = file a)
    #[inline]
    #[must_use]
    pub const fn file(self) -> usize {
        (self.0 % 8) as usize
    }

    /// Flip the square vertically (e.g., a1 <-> a8)
    #[inline]
    #[must_use]
    pub const fn flip_vertical(self) -> Self {
        Square::new(7 - self.rank(), self.file())
    }

    /// Flip the square horizontally (e.g., a1 <-> h1)
    #[inline]
    #[must_use]
    pub const fn flip_horizontal(self) -> Self {
        Square::new(self.rank(), 7 - self.file())
    }

    /// Get the square one rank forward from a color's perspective.
    /// Returns None if the square is already at the edge (rank 7 for White, rank 0 for Black).
    #[inline]
    #[must_use]
    pub const fn forward(self, is_white: bool) -> Option<Self> {
        let rank = self.rank();
        if is_white {
            if rank < 7 {
                Some(Square::new(rank + 1, self.file()))
            } else {
                None
            }
        } else if rank > 0 {
            Some(Square::new(rank - 1, self.file()))
        } else {
            None
        }
    }

    /// Get the square's index (0-63, a1=0, b1=1, ..., h8=63)
    #[inline]
    #[must_use]
    pub const fn as_index(self) -> usize {
        self.0 as usize
    }

    /// Create a square from an index (0-63)
    #[inline]
    #[must_use]
    pub const fn from_index(idx: usize) -> Self {
        Square(idx as u8)
    }

    /// Alias for `as_index`, returns the internal index directly
    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    /// Calculate Manhattan distance to another square
    #[inline]
    #[must_use]
    pub fn manhattan_distance(self, other: Square) -> i32 {
        let file_dist = (self.file() as i32 - other.file() as i32).abs();
        let rank_dist = (self.rank() as i32 - other.rank() as i32).abs();
        file_dist + rank_dist
    }

    /// Calculate file distance to another square
    #[inline]
    #[must_use]
    pub fn file_distance(self, other: Square) -> i32 {
        (self.file() as i32 - other.file() as i32).abs()
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            (self.file() as u8 + b'a') as char,
            self.rank() + 1
        )
    }
}

impl PartialOrd for Square {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Square {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl TryFrom<(usize, usize)> for Square {
    type Error = SquareError;

    fn try_from((rank, file): (usize, usize)) -> Result<Self, Self::Error> {
        if rank >= 8 {
            return Err(SquareError::RankOutOfBounds { rank });
        }
        if file >= 8 {
            return Err(SquareError::FileOutOfBounds { file });
        }
        Ok(Square::new(rank, file))
    }
}

impl FromStr for Square {
    type Err = SquareError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 2 {
            return Err(SquareError::InvalidNotation {
                notation: s.to_string(),
            });
        }

        let file = match chars[0] {
            'a'..='h' => chars[0] as usize - 'a' as usize,
            _ => {
                return Err(SquareError::InvalidNotation {
                    notation: s.to_string(),
                })
            }
        };

        let rank = match chars[1] {
            '1'..='8' => chars[1] as usize - '1' as usize,
            _ => {
                return Err(SquareError::InvalidNotation {
                    notation: s.to_string(),
                })
            }
        };

        Ok(Square::new(rank, file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_new() {
        let sq = Square::new(0, 0);
        assert_eq!(sq.rank(), 0);
        assert_eq!(sq.file(), 0);

        let sq = Square::new(7, 7);
        assert_eq!(sq.rank(), 7);
        assert_eq!(sq.file(), 7);
    }

    #[test]
    fn test_square_try_new() {
        assert!(Square::try_new(0, 0).is_some());
        assert!(Square::try_new(7, 7).is_some());
        assert!(Square::try_new(8, 0).is_none());
        assert!(Square::try_new(0, 8).is_none());
    }

    #[test]
    fn test_square_index() {
        let a1 = Square::new(0, 0);
        assert_eq!(a1.index(), 0);
        assert_eq!(a1.as_index(), 0);

        let h8 = Square::new(7, 7);
        assert_eq!(h8.index(), 63);
    }

    #[test]
    fn test_square_from_index() {
        let sq = Square::from_index(0);
        assert_eq!(sq.rank(), 0);
        assert_eq!(sq.file(), 0);

        let sq = Square::from_index(63);
        assert_eq!(sq.rank(), 7);
        assert_eq!(sq.file(), 7);
    }

    #[test]
    fn test_square_flip_vertical() {
        let a1 = Square::new(0, 0);
        let a8 = a1.flip_vertical();
        assert_eq!(a8.rank(), 7);
        assert_eq!(a8.file(), 0);
    }

    #[test]
    fn test_square_flip_horizontal() {
        let a1 = Square::new(0, 0);
        let h1 = a1.flip_horizontal();
        assert_eq!(h1.rank(), 0);
        assert_eq!(h1.file(), 7);
    }

    #[test]
    fn test_square_forward_white() {
        let e4 = Square::new(3, 4);
        let e5 = e4.forward(true).unwrap();
        assert_eq!(e5.rank(), 4);
        assert_eq!(e5.file(), 4);

        // Can't go forward from rank 8
        let e8 = Square::new(7, 4);
        assert!(e8.forward(true).is_none());
    }

    #[test]
    fn test_square_forward_black() {
        let e5 = Square::new(4, 4);
        let e4 = e5.forward(false).unwrap();
        assert_eq!(e4.rank(), 3);

        // Can't go forward from rank 1
        let e1 = Square::new(0, 4);
        assert!(e1.forward(false).is_none());
    }

    #[test]
    fn test_square_manhattan_distance() {
        let a1 = Square::new(0, 0);
        let h8 = Square::new(7, 7);
        assert_eq!(a1.manhattan_distance(h8), 14);

        let e4 = Square::new(3, 4);
        assert_eq!(e4.manhattan_distance(e4), 0);
    }

    #[test]
    fn test_square_file_distance() {
        let a1 = Square::new(0, 0);
        let h1 = Square::new(0, 7);
        assert_eq!(a1.file_distance(h1), 7);

        let a1 = Square::new(0, 0);
        let a8 = Square::new(7, 0);
        assert_eq!(a1.file_distance(a8), 0);
    }

    #[test]
    fn test_square_display() {
        let a1 = Square::new(0, 0);
        assert_eq!(a1.to_string(), "a1");

        let h8 = Square::new(7, 7);
        assert_eq!(h8.to_string(), "h8");

        let e4 = Square::new(3, 4);
        assert_eq!(e4.to_string(), "e4");
    }

    #[test]
    fn test_square_from_str() {
        let sq: Square = "a1".parse().unwrap();
        assert_eq!(sq.rank(), 0);
        assert_eq!(sq.file(), 0);

        let sq: Square = "h8".parse().unwrap();
        assert_eq!(sq.rank(), 7);
        assert_eq!(sq.file(), 7);
    }

    #[test]
    fn test_square_from_str_error() {
        assert!("z1".parse::<Square>().is_err());
        assert!("a9".parse::<Square>().is_err());
        assert!("a".parse::<Square>().is_err());
        assert!("a1b".parse::<Square>().is_err());
    }

    #[test]
    fn test_square_try_from_tuple() {
        let sq: Square = (3, 4).try_into().unwrap();
        assert_eq!(sq.rank(), 3);
        assert_eq!(sq.file(), 4);

        assert!(Square::try_from((8, 0)).is_err());
        assert!(Square::try_from((0, 8)).is_err());
    }

    #[test]
    fn test_square_ord() {
        let a1 = Square::new(0, 0);
        let b1 = Square::new(0, 1);
        let a2 = Square::new(1, 0);

        assert!(a1 < b1);
        assert!(b1 < a2);
    }

    #[test]
    fn test_file_to_index() {
        assert_eq!(file_to_index('a'), 0);
        assert_eq!(file_to_index('h'), 7);
    }

    #[test]
    fn test_rank_to_index() {
        assert_eq!(rank_to_index('1'), 0);
        assert_eq!(rank_to_index('8'), 7);
    }
}
