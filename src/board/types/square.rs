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

/// Index into a 64-square bitboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SquareIdx(pub u8);

impl SquareIdx {
    #[inline]
    #[must_use]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// A square on the chess board, represented as (rank, file).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Square(pub usize, pub usize); // (rank, file)

impl Square {
    /// Create a new square with bounds checking
    #[must_use]
    pub fn new(rank: usize, file: usize) -> Option<Self> {
        if rank < 8 && file < 8 {
            Some(Square(rank, file))
        } else {
            None
        }
    }

    /// Get the rank (0-7, where 0 = rank 1)
    #[inline]
    #[must_use]
    pub const fn rank(self) -> usize {
        self.0
    }

    /// Get the file (0-7, where 0 = file a)
    #[inline]
    #[must_use]
    pub const fn file(self) -> usize {
        self.1
    }

    /// Flip the square vertically (e.g., a1 <-> a8)
    #[inline]
    #[must_use]
    pub const fn flip_vertical(self) -> Self {
        Square(7 - self.0, self.1)
    }

    /// Flip the square horizontally (e.g., a1 <-> h1)
    #[inline]
    #[must_use]
    pub const fn flip_horizontal(self) -> Self {
        Square(self.0, 7 - self.1)
    }

    /// Get the square's index (0-63, a1=0, b1=1, ..., h8=63)
    #[inline]
    #[must_use]
    pub const fn as_index(self) -> usize {
        self.0 * 8 + self.1
    }

    /// Create a square from an index (0-63)
    #[must_use]
    pub const fn from_index_const(idx: usize) -> Self {
        Square(idx / 8, idx % 8)
    }

    #[inline]
    #[must_use]
    pub(crate) fn from_index(idx: SquareIdx) -> Self {
        let idx = idx.0 as usize;
        Square(idx / 8, idx % 8)
    }

    #[inline]
    #[must_use]
    pub(crate) const fn index(self) -> SquareIdx {
        SquareIdx((self.0 * 8 + self.1) as u8)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", (self.1 as u8 + b'a') as char, self.0 + 1)
    }
}

impl PartialOrd for Square {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Square {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare by index (a1=0, b1=1, ..., h8=63)
        self.index().0.cmp(&other.index().0)
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
        Ok(Square(rank, file))
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

        Ok(Square(rank, file))
    }
}
