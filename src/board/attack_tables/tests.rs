use super::*;

#[test]
fn test_rook_attacks_empty_board() {
    // Rook on e4 (square 28) on empty board
    let attacks = rook_attacks(28, 0);
    // Should attack entire rank 4 and file e (minus the square itself)
    let expected_rank = 0xFFu64 << 24; // rank 4
    let expected_file = FILE_A << 4; // file e
    let expected = (expected_rank | expected_file) & !(1u64 << 28);
    assert_eq!(attacks, expected);
}

#[test]
fn test_bishop_attacks_empty_board() {
    // Bishop on e4 (square 28) on empty board
    // e4 = rank 3, file 4
    // Diagonal (SW-NE): b1(1), c2(10), d3(19), e4(28), f5(37), g6(46), h7(55)
    // Anti-diagonal (NW-SE): a8(56), b7(49), c6(42), d5(35), e4(28), f3(21), g2(14), h1(7)
    let attacks = bishop_attacks(28, 0);
    // Check some squares on the diagonals
    assert!(attacks & (1u64 << 1) != 0); // b1 - on diagonal
    assert!(attacks & (1u64 << 55) != 0); // h7 - on diagonal
    assert!(attacks & (1u64 << 7) != 0); // h1 - on anti-diagonal
    assert!(attacks & (1u64 << 56) != 0); // a8 - on anti-diagonal
                                          // e4 itself should not be in attacks
    assert!(attacks & (1u64 << 28) == 0);
}

#[test]
fn test_rook_attacks_with_blockers() {
    // Rook on e4 (square 28), blockers on e6 and c4
    let blockers = (1u64 << 44) | (1u64 << 26); // e6 and c4
    let attacks = rook_attacks(28, blockers);
    // Should not attack beyond blockers
    assert!(attacks & (1u64 << 44) != 0); // e6 - can capture
    assert!(attacks & (1u64 << 52) == 0); // e7 - blocked
    assert!(attacks & (1u64 << 26) != 0); // c4 - can capture
    assert!(attacks & (1u64 << 25) == 0); // b4 - blocked
}

#[test]
fn test_bishop_attacks_with_blockers() {
    // Bishop on e4 (square 28), blocker on g6
    let blockers = 1u64 << 46; // g6
    let attacks = bishop_attacks(28, blockers);
    // Should attack up to g6 but not h7
    assert!(attacks & (1u64 << 46) != 0); // g6 - can capture
    assert!(attacks & (1u64 << 55) == 0); // h7 - blocked
}

#[test]
fn test_slider_attacks_compatibility() {
    // Test that slider_attacks works correctly for both piece types
    for sq in 0..64 {
        for occ in [0u64, 0xFF00FF00FF00FF00, 0x00FF00FF00FF00FF] {
            let rook = slider_attacks(sq, occ, false);
            let bishop = slider_attacks(sq, occ, true);
            assert_eq!(rook, rook_attacks(sq, occ));
            assert_eq!(bishop, bishop_attacks(sq, occ));
        }
    }
}
