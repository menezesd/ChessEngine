use super::*;
use crate::board::types::{Bitboard, Color};

#[test]
fn test_adjacent_files() {
    // File A should only have file B adjacent
    assert_eq!(ADJACENT_FILES[0], Bitboard::FILE_B);
    // File D should have files C and E adjacent
    assert_eq!(ADJACENT_FILES[3].0, Bitboard::FILE_C.0 | Bitboard::FILE_E.0);
    // File H should only have file G adjacent
    assert_eq!(ADJACENT_FILES[7], Bitboard::FILE_G);
}

#[test]
fn test_king_zone() {
    // Corner king (a1) should have 4 squares
    assert_eq!(KING_ZONE[0].popcount(), 4);
    // Edge king (a4) should have 6 squares
    assert_eq!(KING_ZONE[24].popcount(), 6);
    // Center king (d4) should have 9 squares
    assert_eq!(KING_ZONE[27].popcount(), 9);
}

#[test]
fn test_king_attack_table() {
    // Low attack units = low score
    assert!(KING_ATTACK_TABLE[0] == 0);
    assert!(KING_ATTACK_TABLE[5] < 50);
    // Formula: 480 * i² / (i² + 4200)
    // i=50: 480*2500/6700 ≈ 179
    assert!(KING_ATTACK_TABLE[50] > 150);
    // i=100: 480*10000/14200 ≈ 338
    assert!(KING_ATTACK_TABLE[100] > 300);
    // Should never exceed ~480 (asymptote)
    assert!(KING_ATTACK_TABLE[255] < 480);
}

#[test]
fn test_passed_pawn_mask_white() {
    // White pawn on e4 - check that mask includes e5-e7 and d5-d7, f5-f7
    let mask = PASSED_PAWN_MASK[0][28]; // e4
                                        // Should include e5 (36)
    assert!((mask.0 & (1u64 << 36)) != 0, "e5 should be in mask");
    // Should include d5 (35)
    assert!((mask.0 & (1u64 << 35)) != 0, "d5 should be in mask");
    // Should include f5 (37)
    assert!((mask.0 & (1u64 << 37)) != 0, "f5 should be in mask");
    // Should NOT include e4 itself
    assert!((mask.0 & (1u64 << 28)) == 0, "e4 should not be in mask");
}

#[test]
fn test_passed_pawn_mask_black() {
    // Black pawn on e5 - check that mask includes e4-e2 and d4-d2, f4-f2
    let mask = PASSED_PAWN_MASK[1][36]; // e5
                                        // Should include e4 (28)
    assert!((mask.0 & (1u64 << 28)) != 0, "e4 should be in mask");
    // Should include d4 (27)
    assert!((mask.0 & (1u64 << 27)) != 0, "d4 should be in mask");
}

#[test]
fn test_pawn_shield_mask() {
    // White king on g-file - shield should include f2, g2, h2, f3, g3, h3
    let mask = PAWN_SHIELD_MASK[0][6]; // g-file
    assert!((mask.0 & (1u64 << 13)) != 0, "f2 should be in shield");
    assert!((mask.0 & (1u64 << 14)) != 0, "g2 should be in shield");
    assert!((mask.0 & (1u64 << 15)) != 0, "h2 should be in shield");
}

#[test]
fn test_fill_north() {
    // Single bit on a1 should fill the entire a-file
    let filled = fill_north(1);
    assert_eq!(filled, Bitboard::FILE_A.0);
}

#[test]
fn test_fill_south() {
    // Single bit on a8 should fill the entire a-file
    let filled = fill_south(1u64 << 56);
    assert_eq!(filled, Bitboard::FILE_A.0);
}

#[test]
fn test_relative_rank() {
    // White's rank 0 is their first rank
    assert_eq!(relative_rank(0, Color::White), 0);
    // Black's rank 7 is their first rank
    assert_eq!(relative_rank(7, Color::Black), 0);
    // White's rank 7 is promotion rank
    assert_eq!(relative_rank(7, Color::White), 7);
    // Black's rank 0 is promotion rank
    assert_eq!(relative_rank(0, Color::Black), 7);
}

#[test]
fn test_files_array() {
    assert_eq!(FILES[0], Bitboard::FILE_A);
    assert_eq!(FILES[7], Bitboard::FILE_H);
}
