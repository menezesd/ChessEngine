mod tables;

pub(crate) use tables::{KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};

pub(crate) fn slider_attacks(from_idx: usize, occupancy: u64, bishop: bool) -> u64 {
    let mask = if bishop {
        tables::BISHOP_MASKS[from_idx]
    } else {
        tables::ROOK_MASKS[from_idx]
    };
    let occ = occupancy & mask;
    let index = index_from_occupancy(occ, mask);
    if bishop {
        tables::BISHOP_ATTACKS[from_idx][index]
    } else {
        tables::ROOK_ATTACKS[from_idx][index]
    }
}

fn index_from_occupancy(occ: u64, mut mask: u64) -> usize {
    let mut index = 0usize;
    let mut bit = 0usize;
    while mask != 0 {
        let sq = tables::pop_lsb_u64(&mut mask);
        if occ & (1u64 << sq) != 0 {
            index |= 1usize << bit;
        }
        bit += 1;
    }
    index
}
