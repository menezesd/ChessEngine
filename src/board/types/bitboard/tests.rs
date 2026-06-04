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

    let intersection = a.and(b);
    assert_eq!(intersection.popcount(), 1);

    let union = a.or(b);
    assert_eq!(union.popcount(), 15);

    let xor = a.xor(b);
    assert_eq!(xor.popcount(), 14);

    let not_a = a.not();
    assert_eq!(not_a.popcount(), 56);
}

#[test]
fn test_bitboard_iterator() {
    let bb = Bitboard::from_square(Square::new(0, 0)).or(Bitboard::from_square(Square::new(1, 1)));

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

    assert!(a.intersects(b));
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

    assert_eq!(a.intersect_popcount(b), 1);
    assert_eq!(a.intersect_popcount(a), 8);
}

#[test]
fn test_has_bit() {
    let bb = Bitboard::FILE_A;
    assert!(bb.has_bit(0)); // a1
    assert!(bb.has_bit(8)); // a2
    assert!(!bb.has_bit(1)); // b1
}
