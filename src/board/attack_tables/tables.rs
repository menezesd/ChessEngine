pub(crate) fn pop_lsb_u64(bb: &mut u64) -> usize {
    let idx = bb.trailing_zeros() as usize;
    *bb &= *bb - 1;
    idx
}

pub(crate) static KNIGHT_ATTACKS: std::sync::LazyLock<[u64; 64]> = std::sync::LazyLock::new(|| {
    let mut attacks = [0u64; 64];
    let deltas = [
        (2, 1),
        (1, 2),
        (-1, 2),
        (-2, 1),
        (-2, -1),
        (-1, -2),
        (1, -2),
        (2, -1),
    ];
    for (sq, slot) in attacks.iter_mut().enumerate() {
        let r = (sq / 8) as isize;
        let f = (sq % 8) as isize;
        let mut mask = 0u64;
        for (dr, df) in deltas {
            let nr = r + dr;
            let nf = f + df;
            if (0..8).contains(&nr) && (0..8).contains(&nf) {
                let idx = (nr as usize) * 8 + (nf as usize);
                mask |= 1u64 << idx;
            }
        }
        *slot = mask;
    }
    attacks
});

pub(crate) static KING_ATTACKS: std::sync::LazyLock<[u64; 64]> = std::sync::LazyLock::new(|| {
    let mut attacks = [0u64; 64];
    let deltas = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    for (sq, slot) in attacks.iter_mut().enumerate() {
        let r = (sq / 8) as isize;
        let f = (sq % 8) as isize;
        let mut mask = 0u64;
        for (dr, df) in deltas {
            let nr = r + dr;
            let nf = f + df;
            if (0..8).contains(&nr) && (0..8).contains(&nf) {
                let idx = (nr as usize) * 8 + (nf as usize);
                mask |= 1u64 << idx;
            }
        }
        *slot = mask;
    }
    attacks
});

pub(crate) static PAWN_ATTACKS: std::sync::LazyLock<[[u64; 64]; 2]> = std::sync::LazyLock::new(|| {
    let mut attacks = [[0u64; 64]; 2];
    let (white_attacks, black_attacks) = attacks.split_at_mut(1);
    let white_attacks = &mut white_attacks[0];
    let black_attacks = &mut black_attacks[0];
    for (sq, (white_slot, black_slot)) in white_attacks
        .iter_mut()
        .zip(black_attacks.iter_mut())
        .enumerate()
    {
        let r = (sq / 8) as isize;
        let f = (sq % 8) as isize;
        let mut white = 0u64;
        let wr = r + 1;
        if (0..8).contains(&wr) {
            for df in [-1, 1] {
                let wf = f + df;
                if (0..8).contains(&wf) {
                    white |= 1u64 << ((wr as usize) * 8 + (wf as usize));
                }
            }
        }
        *white_slot = white;
        let mut black = 0u64;
        let br = r - 1;
        if (0..8).contains(&br) {
            for df in [-1, 1] {
                let bf = f + df;
                if (0..8).contains(&bf) {
                    black |= 1u64 << ((br as usize) * 8 + (bf as usize));
                }
            }
        }
        *black_slot = black;
    }
    attacks
});

pub(crate) const DIR_N: usize = 0;
pub(crate) const DIR_S: usize = 1;
pub(crate) const DIR_E: usize = 2;
pub(crate) const DIR_W: usize = 3;
pub(crate) const DIR_NE: usize = 4;
pub(crate) const DIR_NW: usize = 5;
pub(crate) const DIR_SE: usize = 6;
pub(crate) const DIR_SW: usize = 7;

static RAYS: std::sync::LazyLock<[[u64; 64]; 8]> = std::sync::LazyLock::new(|| {
    let mut rays = [[0u64; 64]; 8];
    let dirs = [
        (1, 0),   // N
        (-1, 0),  // S
        (0, 1),   // E
        (0, -1),  // W
        (1, 1),   // NE
        (1, -1),  // NW
        (-1, 1),  // SE
        (-1, -1), // SW
    ];
    for sq in 0..64 {
        let r = (sq / 8) as isize;
        let f = (sq % 8) as isize;
        for (d, (dr, df)) in dirs.iter().enumerate() {
            let mut mask = 0u64;
            let mut nr = r + dr;
            let mut nf = f + df;
            while (0..8).contains(&nr) && (0..8).contains(&nf) {
                let idx = (nr as usize) * 8 + (nf as usize);
                mask |= 1u64 << idx;
                nr += dr;
                nf += df;
            }
            rays[d][sq] = mask;
        }
    }
    rays
});

pub(crate) static ROOK_MASKS: std::sync::LazyLock<[u64; 64]> = std::sync::LazyLock::new(|| {
    let mut masks = [0u64; 64];
    for (sq, slot) in masks.iter_mut().enumerate() {
        let mut mask = 0u64;
        for &dir in &[DIR_N, DIR_S, DIR_E, DIR_W] {
            let ray = RAYS[dir][sq];
            let trimmed = match dir {
                DIR_N => ray & !0xff00000000000000u64,
                DIR_S => ray & !0x00000000000000ffu64,
                DIR_E => ray & !0x8080808080808080u64,
                DIR_W => ray & !0x0101010101010101u64,
                _ => ray,
            };
            mask |= trimmed;
        }
        *slot = mask;
    }
    masks
});

pub(crate) static BISHOP_MASKS: std::sync::LazyLock<[u64; 64]> = std::sync::LazyLock::new(|| {
    let mut masks = [0u64; 64];
    for (sq, slot) in masks.iter_mut().enumerate() {
        let mut mask = 0u64;
        for &dir in &[DIR_NE, DIR_NW, DIR_SE, DIR_SW] {
            let ray = RAYS[dir][sq];
            let trimmed = match dir {
                DIR_NE => ray & !0xff00000000000000u64 & !0x8080808080808080u64,
                DIR_NW => ray & !0xff00000000000000u64 & !0x0101010101010101u64,
                DIR_SE => ray & !0x00000000000000ffu64 & !0x8080808080808080u64,
                DIR_SW => ray & !0x00000000000000ffu64 & !0x0101010101010101u64,
                _ => ray,
            };
            mask |= trimmed;
        }
        *slot = mask;
    }
    masks
});

pub(crate) static ROOK_ATTACKS: std::sync::LazyLock<Vec<Vec<u64>>> = std::sync::LazyLock::new(|| {
    let mut tables = Vec::with_capacity(64);
    for sq in 0..64 {
        let mask = ROOK_MASKS[sq];
        let bits = mask.count_ones() as usize;
        let size = 1usize << bits;
        let mut table = vec![0u64; size];
        for (index, entry) in table.iter_mut().enumerate().take(size) {
            let occ = occupancy_from_index(index, mask);
            *entry = gen_slider_attacks(sq, occ, false);
        }
        tables.push(table);
    }
    tables
});

pub(crate) static BISHOP_ATTACKS: std::sync::LazyLock<Vec<Vec<u64>>> = std::sync::LazyLock::new(|| {
    let mut tables = Vec::with_capacity(64);
    for sq in 0..64 {
        let mask = BISHOP_MASKS[sq];
        let bits = mask.count_ones() as usize;
        let size = 1usize << bits;
        let mut table = vec![0u64; size];
        for (index, entry) in table.iter_mut().enumerate().take(size) {
            let occ = occupancy_from_index(index, mask);
            *entry = gen_slider_attacks(sq, occ, true);
        }
        tables.push(table);
    }
    tables
});

fn is_increasing_dir(dir: usize) -> bool {
    matches!(dir, DIR_N | DIR_E | DIR_NE | DIR_NW)
}

fn nearest_blocker_idx(dir: usize, blockers: u64) -> usize {
    if is_increasing_dir(dir) {
        blockers.trailing_zeros() as usize
    } else {
        63 - blockers.leading_zeros() as usize
    }
}

fn ray_attacks(from_idx: usize, dir: usize, occupancy: u64) -> u64 {
    let ray = RAYS[dir][from_idx];
    let blockers = ray & occupancy;
    if blockers == 0 {
        return ray;
    }
    let blocker_idx = nearest_blocker_idx(dir, blockers);
    ray ^ RAYS[dir][blocker_idx]
}

fn occupancy_from_index(mut index: usize, mask: u64) -> u64 {
    let mut result = 0u64;
    let mut m = mask;
    while m != 0 {
        let sq = pop_lsb_u64(&mut m);
        if index & 1 != 0 {
            result |= 1u64 << sq;
        }
        index >>= 1;
    }
    result
}

fn gen_slider_attacks(from_idx: usize, occupancy: u64, bishop: bool) -> u64 {
    let mut attacks = 0u64;
    let dirs: &[usize] = if bishop {
        &[DIR_NE, DIR_NW, DIR_SE, DIR_SW]
    } else {
        &[DIR_N, DIR_S, DIR_E, DIR_W]
    };

    for &dir in dirs {
        let ray = ray_attacks(from_idx, dir, occupancy);
        attacks |= ray;
    }
    attacks
}
