//! Square types and utilities.

use std::fmt;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::board::error::SquareError;

fn invalid_square_notation(s: &str) -> SquareError {
    SquareError::InvalidNotation {
        notation: s.to_string(),
    }
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
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return Err(invalid_square_notation(s));
        }

        let file = match bytes[0] {
            b'a'..=b'h' => (bytes[0] - b'a') as usize,
            _ => return Err(invalid_square_notation(s)),
        };

        let rank = match bytes[1] {
            b'1'..=b'8' => (bytes[1] - b'1') as usize,
            _ => return Err(invalid_square_notation(s)),
        };

        Ok(Square::new(rank, file))
    }
}

#[cfg(test)]
mod tests;
