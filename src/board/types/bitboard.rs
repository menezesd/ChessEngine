//! Bitboard type and operations.

use super::square::{Square, SquareIdx};

/// A 64-bit bitboard representing piece positions or attack squares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bitboard(pub u64);

// File masks (columns)
impl Bitboard {
    pub const FILE_A: Bitboard = Bitboard(0x0101010101010101);
    pub const FILE_B: Bitboard = Bitboard(0x0202020202020202);
    pub const FILE_C: Bitboard = Bitboard(0x0404040404040404);
    pub const FILE_D: Bitboard = Bitboard(0x0808080808080808);
    pub const FILE_E: Bitboard = Bitboard(0x1010101010101010);
    pub const FILE_F: Bitboard = Bitboard(0x2020202020202020);
    pub const FILE_G: Bitboard = Bitboard(0x4040404040404040);
    pub const FILE_H: Bitboard = Bitboard(0x8080808080808080);

    pub const RANK_1: Bitboard = Bitboard(0x00000000000000FF);
    pub const RANK_2: Bitboard = Bitboard(0x000000000000FF00);
    pub const RANK_3: Bitboard = Bitboard(0x0000000000FF0000);
    pub const RANK_4: Bitboard = Bitboard(0x00000000FF000000);
    pub const RANK_5: Bitboard = Bitboard(0x000000FF00000000);
    pub const RANK_6: Bitboard = Bitboard(0x0000FF0000000000);
    pub const RANK_7: Bitboard = Bitboard(0x00FF000000000000);
    pub const RANK_8: Bitboard = Bitboard(0xFF00000000000000);

    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(!0);

    /// Light squares (a1, c1, e1, g1, b2, d2, ...)
    pub const LIGHT_SQUARES: Bitboard = Bitboard(0x55AA55AA55AA55AA);
    /// Dark squares (b1, d1, f1, h1, a2, c2, ...)
    pub const DARK_SQUARES: Bitboard = Bitboard(0xAA55AA55AA55AA55);
}

impl Bitboard {
    /// Create a bitboard with a single square set
    #[inline]
    #[must_use]
    pub const fn from_square(sq: Square) -> Self {
        Bitboard(1 << (sq.0 * 8 + sq.1))
    }

    /// Returns an iterator over the square indices set in this bitboard
    #[inline]
    #[must_use]
    pub fn iter(self) -> BitboardIter {
        BitboardIter(self)
    }

    /// Returns true if the bitboard is empty
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the number of set bits (population count)
    #[inline]
    #[must_use]
    pub const fn popcount(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns true if exactly one bit is set
    #[inline]
    #[must_use]
    pub const fn is_single(self) -> bool {
        self.0.is_power_of_two()
    }

    /// Returns true if the given square is set
    #[inline]
    #[must_use]
    pub const fn contains(self, sq: Square) -> bool {
        (self.0 & (1 << (sq.0 * 8 + sq.1))) != 0
    }

    /// Shift all bits north (toward rank 8)
    #[inline]
    #[must_use]
    pub const fn shift_north(self) -> Self {
        Bitboard(self.0 << 8)
    }

    /// Shift all bits south (toward rank 1)
    #[inline]
    #[must_use]
    pub const fn shift_south(self) -> Self {
        Bitboard(self.0 >> 8)
    }

    /// Shift all bits east (toward file h), masking off file a wraparound
    #[inline]
    #[must_use]
    pub const fn shift_east(self) -> Self {
        Bitboard((self.0 << 1) & !Self::FILE_A.0)
    }

    /// Shift all bits west (toward file a), masking off file h wraparound
    #[inline]
    #[must_use]
    pub const fn shift_west(self) -> Self {
        Bitboard((self.0 >> 1) & !Self::FILE_H.0)
    }

    /// Get the file mask for a given file index (0-7)
    #[inline]
    #[must_use]
    pub const fn file_mask(file: usize) -> Self {
        Bitboard(Self::FILE_A.0 << file)
    }

    /// Get the rank mask for a given rank index (0-7)
    #[inline]
    #[must_use]
    pub const fn rank_mask(rank: usize) -> Self {
        Bitboard(Self::RANK_1.0 << (rank * 8))
    }

    /// Bitwise AND
    #[inline]
    #[must_use]
    pub const fn and(self, other: Self) -> Self {
        Bitboard(self.0 & other.0)
    }

    /// Bitwise OR
    #[inline]
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Bitboard(self.0 | other.0)
    }

    /// Bitwise XOR
    #[inline]
    #[must_use]
    pub const fn xor(self, other: Self) -> Self {
        Bitboard(self.0 ^ other.0)
    }

    /// Bitwise NOT
    #[inline]
    #[must_use]
    pub const fn not(self) -> Self {
        Bitboard(!self.0)
    }
}

pub(crate) fn bit_for_square(sq: Square) -> Bitboard {
    Bitboard(1u64 << sq.index().as_usize())
}

pub(crate) fn pop_lsb(bb: &mut Bitboard) -> SquareIdx {
    let idx = bb.0.trailing_zeros() as u8;
    bb.0 &= bb.0 - 1;
    SquareIdx(idx)
}

/// Iterator over set bits in a Bitboard
pub struct BitboardIter(Bitboard);

impl Iterator for BitboardIter {
    type Item = SquareIdx;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else {
            Some(pop_lsb(&mut self.0))
        }
    }
}
