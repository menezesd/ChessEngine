use super::tables::{pop_lsb_u64, BISHOP_ATTACKS, BISHOP_MASKS, ROOK_ATTACKS, ROOK_MASKS};

fn index_from_occupancy(occ: u64, mask: u64) -> usize {
    let mut index = 0usize;
    let mut bit = 0usize;
    let mut m = mask;
    while m != 0 {
        let sq = pop_lsb_u64(&mut m);
        if occ & (1u64 << sq) != 0 {
            index |= 1usize << bit;
        }
        bit += 1;
    }
    index
}

pub(crate) fn slider_attacks(from_idx: usize, occupancy: u64, bishop: bool) -> u64 {
    if bishop {
        let mask = BISHOP_MASKS[from_idx];
        let index = index_from_occupancy(occupancy, mask);
        BISHOP_ATTACKS[from_idx][index]
    } else {
        let mask = ROOK_MASKS[from_idx];
        let index = index_from_occupancy(occupancy, mask);
        ROOK_ATTACKS[from_idx][index]
    }
}
