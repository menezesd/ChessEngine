//! Attack tables for move generation.
//!
//! Uses Hyperbola Quintessence for sliding piece attacks (bishop, rook, queen).
//! This is a fast, branch-free algorithm that uses the `o^(o-2r)` trick.

#![allow(clippy::needless_range_loop)] // Index loops are clearer for board coordinates
#![allow(clippy::inline_always)] // Performance-critical hot path functions

mod tables;

pub(crate) use tables::{KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};

use std::sync::LazyLock;

// File mask for column A
const FILE_A: u64 = 0x0101010101010101;

/// Diagonal masks for each square (bottom-left to top-right direction)
static DIAG_MASKS: LazyLock<[u64; 64]> = LazyLock::new(|| {
    let mut masks = [0u64; 64];
    for sq in 0..64 {
        let rank = sq / 8;
        let file = sq % 8;
        let mut mask = 0u64;
        // Go in both directions along the diagonal
        let mut r = rank as isize;
        let mut f = file as isize;
        while r < 8 && f < 8 {
            mask |= 1u64 << (r * 8 + f);
            r += 1;
            f += 1;
        }
        r = rank as isize - 1;
        f = file as isize - 1;
        while r >= 0 && f >= 0 {
            mask |= 1u64 << (r * 8 + f);
            r -= 1;
            f -= 1;
        }
        masks[sq] = mask;
    }
    masks
});

/// Anti-diagonal masks for each square (top-left to bottom-right direction)
static ANTI_MASKS: LazyLock<[u64; 64]> = LazyLock::new(|| {
    let mut masks = [0u64; 64];
    for sq in 0..64 {
        let rank = sq / 8;
        let file = sq % 8;
        let mut mask = 0u64;
        // Go in both directions along the anti-diagonal
        let mut r = rank as isize;
        let mut f = file as isize;
        while r < 8 && f >= 0 {
            mask |= 1u64 << (r * 8 + f);
            r += 1;
            f -= 1;
        }
        r = rank as isize - 1;
        f = file as isize + 1;
        while r >= 0 && f < 8 {
            mask |= 1u64 << (r * 8 + f);
            r -= 1;
            f += 1;
        }
        masks[sq] = mask;
    }
    masks
});

/// File masks for each square
static FILE_MASKS: LazyLock<[u64; 64]> = LazyLock::new(|| {
    let mut masks = [0u64; 64];
    for sq in 0..64 {
        let file = sq % 8;
        masks[sq] = FILE_A << file;
    }
    masks
});

/// Rank attack lookup table: `[8 * occupancy_6bit + file]` -> attacks on that rank
/// Only stores attacks for file positions, shifted to rank 0
static RANK_ATTACKS: LazyLock<[u64; 512]> = LazyLock::new(|| {
    let mut attacks = [0u64; 512];
    for occ_6bit in 0..64 {
        for file in 0..8 {
            let mut attack = 0u64;
            // Attacks to the right (increasing file)
            for f in (file + 1)..8 {
                attack |= 1u64 << f;
                // Check if blocked (occupancy is bits 1-6, representing files b-g)
                if (1..=6).contains(&f) && (occ_6bit & (1 << (f - 1))) != 0 {
                    break;
                }
            }
            // Attacks to the left (decreasing file)
            for f in (0..file).rev() {
                attack |= 1u64 << f;
                // Check if blocked
                if (1..=6).contains(&f) && (occ_6bit & (1 << (f - 1))) != 0 {
                    break;
                }
            }
            attacks[8 * occ_6bit + file] = attack;
        }
    }
    attacks
});

/// Byteswap - reverses the order of bytes (flips board vertically)
#[inline(always)]
const fn byteswap(x: u64) -> u64 {
    x.swap_bytes()
}

/// Hyperbola Quintessence attack calculation for a single ray direction.
/// Uses the o^(o-2r) trick with byteswap for the reverse direction.
#[inline(always)]
fn hyp_quint(occupied: u64, mask: u64, square: usize) -> u64 {
    let piece_bit = 1u64 << square;
    let forward = occupied & mask;
    let backward = byteswap(forward);
    let forward_attacks = forward.wrapping_sub(piece_bit.wrapping_mul(2));
    let backward_attacks = byteswap(backward.wrapping_sub(byteswap(piece_bit).wrapping_mul(2)));
    (forward_attacks ^ backward_attacks) & mask
}

/// Diagonal attacks (bottom-left to top-right)
#[inline(always)]
fn diag_attacks(occupied: u64, square: usize) -> u64 {
    hyp_quint(occupied, DIAG_MASKS[square], square)
}

/// Anti-diagonal attacks (top-left to bottom-right)
#[inline(always)]
fn anti_attacks(occupied: u64, square: usize) -> u64 {
    hyp_quint(occupied, ANTI_MASKS[square], square)
}

/// File attacks (vertical)
#[inline(always)]
fn file_attacks(occupied: u64, square: usize) -> u64 {
    hyp_quint(occupied, FILE_MASKS[square], square)
}

/// Rank attacks (horizontal) - uses lookup table since byteswap doesn't help
#[inline(always)]
fn rank_attacks(occupied: u64, square: usize) -> u64 {
    let rank = square / 8;
    let file = square % 8;
    let rank_occ = occupied >> (rank * 8);
    // Extract bits 1-6 (files b-g) as the relevant occupancy
    let occ_6bit = ((rank_occ >> 1) & 63) as usize;
    RANK_ATTACKS[8 * occ_6bit + file] << (rank * 8)
}

/// Get sliding attacks for a piece.
/// `bishop` = true for bishop/queen diagonal attacks
/// `bishop` = false for rook/queen orthogonal attacks
#[inline]
pub(crate) fn slider_attacks(square: usize, occupancy: u64, bishop: bool) -> u64 {
    if bishop {
        diag_attacks(occupancy, square) | anti_attacks(occupancy, square)
    } else {
        file_attacks(occupancy, square) | rank_attacks(occupancy, square)
    }
}

/// Get bishop attacks (diagonals only)
#[inline]
pub(crate) fn bishop_attacks(square: usize, occupancy: u64) -> u64 {
    diag_attacks(occupancy, square) | anti_attacks(occupancy, square)
}

/// Get rook attacks (ranks and files only)
#[inline]
pub(crate) fn rook_attacks(square: usize, occupancy: u64) -> u64 {
    file_attacks(occupancy, square) | rank_attacks(occupancy, square)
}

/// Get queen attacks (all 8 directions)
#[inline]
pub(crate) fn queen_attacks(square: usize, occupancy: u64) -> u64 {
    bishop_attacks(square, occupancy) | rook_attacks(square, occupancy)
}

#[cfg(test)]
mod tests {
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
}
