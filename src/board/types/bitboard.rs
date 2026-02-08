//! Bitboard type and operations.

use super::square::Square;

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

    /// Queenside files (a, b, c)
    pub const QUEENSIDE_FILES: Bitboard = Bitboard(0x0707_0707_0707_0707);
    /// Kingside files (f, g, h)
    pub const KINGSIDE_FILES: Bitboard = Bitboard(0xE0E0_E0E0_E0E0_E0E0);
}

impl Bitboard {
    /// Create a bitboard with a single square set
    #[inline]
    #[must_use]
    pub const fn from_square(sq: Square) -> Self {
        Bitboard(1 << sq.as_index())
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
        (self.0 & (1 << sq.as_index())) != 0
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

    /// Returns true if this bitboard has any bits in common with other
    #[inline]
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns true if this bitboard has no bits in common with other
    #[inline]
    #[must_use]
    pub const fn is_disjoint(self, other: Self) -> bool {
        (self.0 & other.0) == 0
    }

    /// Returns the population count of the intersection with other
    #[inline]
    #[must_use]
    pub const fn intersect_popcount(self, other: Self) -> u32 {
        (self.0 & other.0).count_ones()
    }

    /// Returns true if the given bit index is set
    #[inline]
    #[must_use]
    pub const fn has_bit(self, idx: usize) -> bool {
        (self.0 & (1u64 << idx)) != 0
    }
}

pub(crate) fn bit_for_square(sq: Square) -> Bitboard {
    Bitboard(1u64 << sq.index())
}

pub(crate) fn pop_lsb(bb: &mut Bitboard) -> Square {
    let idx = bb.0.trailing_zeros() as usize;
    bb.0 &= bb.0 - 1;
    Square::from_index(idx)
}

/// Iterator over set bits in a Bitboard
pub struct BitboardIter(Bitboard);

impl Iterator for BitboardIter {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else {
            Some(pop_lsb(&mut self.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitboard_from_square() {
        let bb = Bitboard::from_square(Square::new(0, 0)); // a1
        assert_eq!(bb.0, 1);

        let bb = Bitboard::from_square(Square::new(7, 7)); // h8
        assert_eq!(bb.0, 1u64 << 63);
    }

    #[test]
    fn test_bitboard_is_empty() {
        assert!(Bitboard::EMPTY.is_empty());
        assert!(!Bitboard::ALL.is_empty());
        assert!(!Bitboard::from_square(Square::new(0, 0)).is_empty());
    }

    #[test]
    fn test_bitboard_popcount() {
        assert_eq!(Bitboard::EMPTY.popcount(), 0);
        assert_eq!(Bitboard::from_square(Square::new(0, 0)).popcount(), 1);
        assert_eq!(Bitboard::RANK_1.popcount(), 8);
        assert_eq!(Bitboard::ALL.popcount(), 64);
    }

    #[test]
    fn test_bitboard_is_single() {
        assert!(Bitboard::from_square(Square::new(0, 0)).is_single());
        assert!(!Bitboard::EMPTY.is_single());
        assert!(!Bitboard::RANK_1.is_single());
    }

    #[test]
    fn test_bitboard_contains() {
        let bb = Bitboard::FILE_A;
        assert!(bb.contains(Square::new(0, 0))); // a1
        assert!(bb.contains(Square::new(7, 0))); // a8
        assert!(!bb.contains(Square::new(0, 1))); // b1
    }

    #[test]
    fn test_bitboard_shift_north() {
        let bb = Bitboard::RANK_1.shift_north();
        assert_eq!(bb, Bitboard::RANK_2);
    }

    #[test]
    fn test_bitboard_shift_south() {
        let bb = Bitboard::RANK_2.shift_south();
        assert_eq!(bb, Bitboard::RANK_1);
    }

    #[test]
    fn test_bitboard_shift_east() {
        let bb = Bitboard::FILE_A.shift_east();
        assert_eq!(bb, Bitboard::FILE_B);
    }

    #[test]
    fn test_bitboard_shift_west() {
        let bb = Bitboard::FILE_B.shift_west();
        assert_eq!(bb, Bitboard::FILE_A);
    }

    #[test]
    fn test_bitboard_file_mask() {
        assert_eq!(Bitboard::file_mask(0), Bitboard::FILE_A);
        assert_eq!(Bitboard::file_mask(7), Bitboard::FILE_H);
    }

    #[test]
    fn test_bitboard_rank_mask() {
        assert_eq!(Bitboard::rank_mask(0), Bitboard::RANK_1);
        assert_eq!(Bitboard::rank_mask(7), Bitboard::RANK_8);
    }

    #[test]
    fn test_bitboard_logical_ops() {
        let a = Bitboard::FILE_A;
        let b = Bitboard::RANK_1;

        // AND: intersection (a1 only)
        let intersection = a.and(b);
        assert_eq!(intersection.popcount(), 1);

        // OR: union
        let union = a.or(b);
        assert_eq!(union.popcount(), 15); // 8 + 8 - 1

        // XOR: symmetric difference
        let xor = a.xor(b);
        assert_eq!(xor.popcount(), 14); // 15 - 1

        // NOT
        let not_a = a.not();
        assert_eq!(not_a.popcount(), 56); // 64 - 8
    }

    #[test]
    fn test_bitboard_iterator() {
        let bb =
            Bitboard::from_square(Square::new(0, 0)).or(Bitboard::from_square(Square::new(1, 1)));

        let squares: Vec<Square> = bb.iter().collect();
        assert_eq!(squares.len(), 2);
    }

    #[test]
    fn test_bit_for_square() {
        let bb = bit_for_square(Square::new(3, 4)); // e4
        assert_eq!(bb.0, 1u64 << 28);
    }

    #[test]
    fn test_pop_lsb() {
        let mut bb = Bitboard(0b1100); // bits 2 and 3 set
        let sq = pop_lsb(&mut bb);
        assert_eq!(sq.index(), 2);
        assert_eq!(bb.0, 0b1000);
    }

    #[test]
    fn test_light_dark_squares() {
        // Light and dark squares should be complements
        assert_eq!(Bitboard::LIGHT_SQUARES.popcount(), 32);
        assert_eq!(Bitboard::DARK_SQUARES.popcount(), 32);
        assert_eq!(
            Bitboard::LIGHT_SQUARES.or(Bitboard::DARK_SQUARES),
            Bitboard::ALL
        );
        assert!(Bitboard::LIGHT_SQUARES
            .and(Bitboard::DARK_SQUARES)
            .is_empty());
    }

    #[test]
    fn test_intersects() {
        let a = Bitboard::FILE_A;
        let b = Bitboard::RANK_1;
        let c = Bitboard::FILE_H;

        // FILE_A and RANK_1 share a1
        assert!(a.intersects(b));
        // FILE_A and FILE_H don't share any squares
        assert!(!a.intersects(c));
    }

    #[test]
    fn test_is_disjoint() {
        let a = Bitboard::FILE_A;
        let c = Bitboard::FILE_H;

        assert!(a.is_disjoint(c));
        assert!(!a.is_disjoint(Bitboard::RANK_1));
    }

    #[test]
    fn test_intersect_popcount() {
        let a = Bitboard::FILE_A;
        let b = Bitboard::RANK_1;

        // FILE_A and RANK_1 share only a1
        assert_eq!(a.intersect_popcount(b), 1);
        // FILE_A with itself
        assert_eq!(a.intersect_popcount(a), 8);
    }

    #[test]
    fn test_has_bit() {
        let bb = Bitboard::FILE_A;
        assert!(bb.has_bit(0));  // a1
        assert!(bb.has_bit(8));  // a2
        assert!(!bb.has_bit(1)); // b1
    }
}
