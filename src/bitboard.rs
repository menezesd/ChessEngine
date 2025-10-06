use crate::types::*;

// Castling rights bitmasks
pub const WHITE_KINGSIDE: u8 = 1;
pub const WHITE_QUEENSIDE: u8 = 2;
pub const BLACK_KINGSIDE: u8 = 4;
pub const BLACK_QUEENSIDE: u8 = 8;

// Precomputed attack tables
pub static KNIGHT_ATTACKS: [u64; 64] = generate_knight_attacks();
pub static KING_ATTACKS: [u64; 64] = generate_king_attacks();

// File mask for a given file (0-7)
pub fn file_mask(file: usize) -> u64 {
    let mut mask = 0u64;
    for rank in 0..8 {
        mask |= 1u64 << (rank * 8 + file);
    }
    mask
}

const fn generate_knight_attacks() -> [u64; 64] {
    let mut attacks = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let r = sq / 8;
        let f = sq % 8;
        let mut mask = 0u64;

        // All possible knight moves
        let deltas = [
            (-2, -1),
            (-2, 1),
            (-1, -2),
            (-1, 2),
            (1, -2),
            (1, 2),
            (2, -1),
            (2, 1),
        ];

        let mut i = 0;
        while i < deltas.len() {
            let dr = deltas[i].0;
            let df = deltas[i].1;
            let nr = r as i32 + dr;
            let nf = f as i32 + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let target_sq = (nr * 8 + nf) as usize;
                mask |= 1u64 << target_sq;
            }
            i += 1;
        }

        attacks[sq] = mask;
        sq += 1;
    }
    attacks
}

const fn generate_king_attacks() -> [u64; 64] {
    let mut attacks = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let r = sq / 8;
        let f = sq % 8;
        let mut mask = 0u64;

        // All possible king moves
        let deltas = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ];

        let mut i = 0;
        while i < deltas.len() {
            let dr = deltas[i].0;
            let df = deltas[i].1;
            let nr = r as i32 + dr;
            let nf = f as i32 + df;
            if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                let target_sq = (nr * 8 + nf) as usize;
                mask |= 1u64 << target_sq;
            }
            i += 1;
        }

        attacks[sq] = mask;
        sq += 1;
    }
    attacks
}

// Sliding piece attack generation (simplified - would need magic bitboards for full implementation)
pub fn bishop_attacks(from: Square, occupied: u64) -> u64 {
    let _sq = from.0 * 8 + from.1;
    let mut attacks = 0u64;

    // Northeast
    let mut r = from.0 as i32 + 1;
    let mut f = from.1 as i32 + 1;
    while r < 8 && f < 8 {
        let target_sq = (r * 8 + f) as usize;
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        r += 1;
        f += 1;
    }

    // Northwest
    r = from.0 as i32 + 1;
    f = from.1 as i32 - 1;
    while r < 8 && f >= 0 {
        let target_sq = (r * 8 + f) as usize;
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        r += 1;
        f -= 1;
    }

    // Southeast
    r = from.0 as i32 - 1;
    f = from.1 as i32 + 1;
    while r >= 0 && f < 8 {
        let target_sq = (r * 8 + f) as usize;
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        r -= 1;
        f += 1;
    }

    // Southwest
    r = from.0 as i32 - 1;
    f = from.1 as i32 - 1;
    while r >= 0 && f >= 0 {
        let target_sq = (r * 8 + f) as usize;
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        r -= 1;
        f -= 1;
    }

    attacks
}

pub fn rook_attacks(from: Square, occupied: u64) -> u64 {
    let _sq = from.0 * 8 + from.1;
    let mut attacks = 0u64;

    // North
    let mut r = from.0 + 1;
    let f = from.1;
    while r < 8 {
        let target_sq = r * 8 + f;
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        r += 1;
    }

    // South
    let mut r = from.0 as i32 - 1;
    while r >= 0 {
        let target_sq = (r as usize) * 8 + f;
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        r -= 1;
    }

    // East
    let mut f = from.1 as i32 + 1;
    let r = from.0;
    while f < 8 {
        let target_sq = r * 8 + (f as usize);
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        f += 1;
    }

    // West
    f = from.1 as i32 - 1;
    while f >= 0 {
        let target_sq = r * 8 + (f as usize);
        attacks |= 1u64 << target_sq;
        if (occupied & (1u64 << target_sq)) != 0 {
            break;
        }
        f -= 1;
    }

    attacks
}

pub fn queen_attacks(from: Square, occupied: u64) -> u64 {
    bishop_attacks(from, occupied) | rook_attacks(from, occupied)
}
