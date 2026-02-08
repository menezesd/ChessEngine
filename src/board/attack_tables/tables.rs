//! Precomputed attack tables for leaper pieces (knights, kings, pawns).

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

pub(crate) static PAWN_ATTACKS: std::sync::LazyLock<[[u64; 64]; 2]> =
    std::sync::LazyLock::new(|| {
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
