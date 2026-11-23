//! Compile-time attack tables for knights, kings, and pawns.

pub(crate) static KNIGHT_ATTACKS: [u64; 64] = leaper_attacks(&[
    (2, 1),
    (1, 2),
    (-1, 2),
    (-2, 1),
    (-2, -1),
    (-1, -2),
    (1, -2),
    (2, -1),
]);

pub(crate) static KING_ATTACKS: [u64; 64] = leaper_attacks(&[
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
]);

pub(crate) static PAWN_ATTACKS: [[u64; 64]; 2] = pawn_attack_table();

const fn leaper_attacks(deltas: &[(isize, isize)]) -> [u64; 64] {
    let mut attacks = [0; 64];
    let mut square = 0;
    while square < 64 {
        let rank = (square / 8) as isize;
        let file = (square % 8) as isize;
        let mut i = 0;
        while i < deltas.len() {
            let row = rank + deltas[i].0;
            let column = file + deltas[i].1;
            if row >= 0 && row < 8 && column >= 0 && column < 8 {
                attacks[square] |= 1 << (row * 8 + column);
            }
            i += 1;
        }
        square += 1;
    }
    attacks
}

#[inline]
pub(crate) const fn pawn_attacks_from_bitboard(squares: u64, white: bool) -> u64 {
    use super::FILE_A;
    const FILE_H: u64 = FILE_A << 7;
    if white {
        ((squares << 7) & !FILE_H) | ((squares << 9) & !FILE_A)
    } else {
        ((squares >> 9) & !FILE_H) | ((squares >> 7) & !FILE_A)
    }
}

const fn pawn_attack_table() -> [[u64; 64]; 2] {
    let mut attacks = [[0; 64]; 2];
    let mut square = 0;
    while square < 64 {
        let pawn = 1_u64 << square;
        attacks[0][square] = pawn_attacks_from_bitboard(pawn, true);
        attacks[1][square] = pawn_attacks_from_bitboard(pawn, false);
        square += 1;
    }
    attacks
}
