//! Attack tables for move generation.
//!
//! Uses Hyperbola Quintessence for sliding piece attacks (bishop, rook, queen).
//! This is a fast, branch-free algorithm that uses the `o^(o-2r)` trick.

#![allow(clippy::needless_range_loop)] // Index loops are clearer for board coordinates
#![allow(clippy::inline_always)] // Performance-critical hot path functions

mod tables;

pub(crate) use tables::{pawn_attacks_from_bitboard, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};

// File mask for column A
const FILE_A: u64 = 0x0101_0101_0101_0101;

static DIAG_MASKS: [u64; 64] = diagonal_masks(1);
static ANTI_MASKS: [u64; 64] = diagonal_masks(-1);
static DIAGONAL_RAYS: [u64; 64] = rays(true);
static STRAIGHT_RAYS: [u64; 64] = rays(false);
static RANK_ATTACKS: [u8; 512] = rank_attack_table();

const fn diagonal_masks(file_step: isize) -> [u64; 64] {
    let mut masks = [0; 64];
    let mut square = 0;
    while square < 64 {
        let rank = (square / 8) as isize;
        let file = (square % 8) as isize;
        let mut row = 0;
        let mut mask = 0;
        while row < 8 {
            let column = file + (row - rank) * file_step;
            if column >= 0 && column < 8 {
                mask |= 1 << (row * 8 + column);
            }
            row += 1;
        }
        masks[square] = mask;
        square += 1;
    }
    masks
}

const fn rays(diagonal: bool) -> [u64; 64] {
    let mut masks = [0; 64];
    let mut square = 0;
    while square < 64 {
        let lines = if diagonal {
            DIAG_MASKS[square] | ANTI_MASKS[square]
        } else {
            (FILE_A << (square % 8)) | (0xff << (square & 0b11_1000))
        };
        masks[square] = lines & !(1 << square);
        square += 1;
    }
    masks
}

/// Each entry is a rank-zero attack mask. Only files b-g can block a
/// ray before its edge, so six occupancy bits identify the attack.
const fn rank_attack_table() -> [u8; 512] {
    let mut attacks = [0; 512];
    let mut occupancy = 0;
    while occupancy < 64 {
        let mut file = 0;
        while file < 8 {
            let blockers = (occupancy as u8) << 1;
            let mut attack = 0;
            let mut right = file + 1;
            while right < 8 {
                let target = 1 << right;
                attack |= target;
                if blockers & target != 0 {
                    break;
                }
                right += 1;
            }
            let mut left = file;
            while left > 0 {
                left -= 1;
                let target = 1 << left;
                attack |= target;
                if blockers & target != 0 {
                    break;
                }
            }
            attacks[8 * occupancy + file] = attack;
            file += 1;
        }
        occupancy += 1;
    }
    attacks
}

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
    hyp_quint(occupied, FILE_A << (square % 8), square)
}

/// Rank attacks (horizontal) - uses lookup table since byteswap doesn't help
#[inline(always)]
fn rank_attacks(occupied: u64, square: usize) -> u64 {
    let rank = square / 8;
    let file = square % 8;
    let rank_occ = occupied >> (rank * 8);
    // Extract bits 1-6 (files b-g) as the relevant occupancy
    let occ_6bit = ((rank_occ >> 1) & 63) as usize;
    u64::from(RANK_ATTACKS[8 * occ_6bit + file]) << (rank * 8)
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

/// Squares geometrically aligned with a bishop, before accounting for blockers.
#[inline]
pub(crate) fn diagonal_rays(square: usize) -> u64 {
    DIAGONAL_RAYS[square]
}

/// Squares geometrically aligned with a rook, before accounting for blockers.
#[inline]
pub(crate) fn straight_rays(square: usize) -> u64 {
    STRAIGHT_RAYS[square]
}

#[cfg(test)]
mod tests;
