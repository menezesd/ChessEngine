use crate::core::types::Square;
use once_cell::sync::Lazy;

pub type Bitboard = u64;

// Lazy-initialized rook masks and attack tables
static ROOK_MASKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut masks = [0u64; 64];
    for (sq, slot) in masks.iter_mut().enumerate() {
        *slot = rook_mask_from_square(sq);
    }
    masks
});

// Flattened rook attack table: a single Vec<Bitboard> plus per-square offsets and sizes.
// This improves cache locality and removes an extra Vec indirection.
static ROOK_ATTACKS_FLAT: Lazy<(Vec<Bitboard>, [usize; 64], [usize; 64])> = Lazy::new(|| {
    let mut flat: Vec<Bitboard> = Vec::new();
    let mut offsets = [0usize; 64];
    let mut sizes = [0usize; 64];
    for sq in 0..64 {
        let mask = ROOK_MASKS[sq];
        let bits: Vec<usize> = (0..64).filter(|&i| (mask >> i) & 1 != 0).collect();
        let relevant_bits = bits.len();
        let table_size = 1usize << relevant_bits;
        offsets[sq] = flat.len();
        sizes[sq] = table_size;

        for index in 0..table_size {
            // Build blockers bitboard from index (same ordering as bits)
            let mut blockers = 0u64;
            for (j, &bit_index) in bits.iter().enumerate() {
                if (index >> j) & 1 != 0 {
                    blockers |= 1u64 << bit_index;
                }
            }
            let attack = rook_attacks_by_rays(sq, blockers);
            flat.push(attack);
        }
    }
    (flat, offsets, sizes)
});

fn rook_mask_from_square(sq: usize) -> Bitboard {
    let rank = (sq / 8) as isize;
    let file = (sq % 8) as isize;
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut mask = 0u64;
    for (dr, df) in directions.iter() {
        let mut r = rank + dr;
        let mut f = file + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            // Stop before reaching the board edge in this direction
            if (*dr != 0 && (r == 0 || r == 7)) || (*df != 0 && (f == 0 || f == 7)) {
                break;
            }
            let idx = (r as usize) * 8 + (f as usize);
            mask |= 1u64 << idx;
            r += dr;
            f += df;
        }
    }
    mask
}

fn rook_attacks_by_rays(sq: usize, occupancy: Bitboard) -> Bitboard {
    let rank = (sq / 8) as isize;
    let file = (sq % 8) as isize;
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut attacks = 0u64;
    for (dr, df) in directions.iter() {
        let mut r = rank + dr;
        let mut f = file + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let idx = (r as usize) * 8 + (f as usize);
            let mask = 1u64 << idx;
            attacks |= mask;
            if occupancy & mask != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

pub fn rook_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    let sq = square.0 * 8 + square.1;
    let mask = ROOK_MASKS[sq];
    let blockers = occupancy & mask;
    // Pack the blocker bits into an index using same ordering as generation
    let mut idx = 0usize;
    let mut bit = 0usize;
    for i in 0..64 {
        if (mask >> i) & 1 != 0 {
            if (blockers >> i) & 1 != 0 {
                idx |= 1usize << bit;
            }
            bit += 1;
        }
    }
    let (ref flat, ref offsets, _) = *ROOK_ATTACKS_FLAT;
    let off = offsets[sq];
    // safety: idx should always be < sizes[sq]
    flat[off + idx]
}

// --- Bishop tables (diagonals) ---
static BISHOP_MASKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut masks = [0u64; 64];
    for (sq, slot) in masks.iter_mut().enumerate() {
        *slot = bishop_mask_from_square(sq);
    }
    masks
});

// Flattened bishop attack table, same approach as rooks.
static BISHOP_ATTACKS_FLAT: Lazy<(Vec<Bitboard>, [usize; 64], [usize; 64])> = Lazy::new(|| {
    let mut flat: Vec<Bitboard> = Vec::new();
    let mut offsets = [0usize; 64];
    let mut sizes = [0usize; 64];
    for sq in 0..64 {
        let mask = BISHOP_MASKS[sq];
        let bits: Vec<usize> = (0..64).filter(|&i| (mask >> i) & 1 != 0).collect();
        let relevant_bits = bits.len();
        let table_size = 1usize << relevant_bits;
        offsets[sq] = flat.len();
        sizes[sq] = table_size;
        for index in 0..table_size {
            let mut blockers = 0u64;
            for (j, &bit_index) in bits.iter().enumerate() {
                if (index >> j) & 1 != 0 {
                    blockers |= 1u64 << bit_index;
                }
            }
            let attack = bishop_attacks_by_rays(sq, blockers);
            flat.push(attack);
        }
    }
    (flat, offsets, sizes)
});

fn bishop_mask_from_square(sq: usize) -> Bitboard {
    let rank = (sq / 8) as isize;
    let file = (sq % 8) as isize;
    let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut mask = 0u64;
    for (dr, df) in directions.iter() {
        let mut r = rank + dr;
        let mut f = file + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            // stop before edge
            if (r == 0 || r == 7) || (f == 0 || f == 7) {
                break;
            }
            let idx = (r as usize) * 8 + (f as usize);
            mask |= 1u64 << idx;
            r += dr;
            f += df;
        }
    }
    mask
}

fn bishop_attacks_by_rays(sq: usize, occupancy: Bitboard) -> Bitboard {
    let rank = (sq / 8) as isize;
    let file = (sq % 8) as isize;
    let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut attacks = 0u64;
    for (dr, df) in directions.iter() {
        let mut r = rank + dr;
        let mut f = file + df;
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let idx = (r as usize) * 8 + (f as usize);
            let mask = 1u64 << idx;
            attacks |= mask;
            if occupancy & mask != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

pub fn bishop_attacks(square: Square, occupancy: Bitboard) -> Bitboard {
    let sq = square.0 * 8 + square.1;
    let mask = BISHOP_MASKS[sq];
    let blockers = occupancy & mask;
    let mut idx = 0usize;
    let mut bit = 0usize;
    for i in 0..64 {
        if (mask >> i) & 1 != 0 {
            if (blockers >> i) & 1 != 0 {
                idx |= 1usize << bit;
            }
            bit += 1;
        }
    }
    let (ref flat, ref offsets, _) = *BISHOP_ATTACKS_FLAT;
    let off = offsets[sq];
    flat[off + idx]
}
