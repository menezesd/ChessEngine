use crate::board::types::{Bitboard, Color};

/// Passed pawn masks - squares that would block a pawn from being passed
/// `PASSED_PAWN_MASK`[color][square] = enemy pawns on these squares block passed status
pub const PASSED_PAWN_MASK: [[Bitboard; 64]; 2] = {
    let mut masks = [[Bitboard(0); 64]; 2];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        if rank < 7 {
            let mut r = rank + 1;
            while r < 8 {
                masks[0][sq].0 |= 1u64 << (r * 8 + file);
                if file > 0 {
                    masks[0][sq].0 |= 1u64 << (r * 8 + file - 1);
                }
                if file < 7 {
                    masks[0][sq].0 |= 1u64 << (r * 8 + file + 1);
                }
                r += 1;
            }
        }

        if rank > 0 {
            let mut r = 0;
            while r < rank {
                masks[1][sq].0 |= 1u64 << (r * 8 + file);
                if file > 0 {
                    masks[1][sq].0 |= 1u64 << (r * 8 + file - 1);
                }
                if file < 7 {
                    masks[1][sq].0 |= 1u64 << (r * 8 + file + 1);
                }
                r += 1;
            }
        }

        sq += 1;
    }
    masks
};

/// Pawn support masks - squares where friendly pawns can support this pawn
/// `PAWN_SUPPORT_MASK`[color][square] = pawns on these squares support the pawn
pub const PAWN_SUPPORT_MASK: [[Bitboard; 64]; 2] = {
    let mut masks = [[Bitboard(0); 64]; 2];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        if file > 0 {
            masks[0][sq].0 |= 1u64 << (rank * 8 + file - 1);
            if rank > 0 {
                masks[0][sq].0 |= 1u64 << ((rank - 1) * 8 + file - 1);
            }
        }
        if file < 7 {
            masks[0][sq].0 |= 1u64 << (rank * 8 + file + 1);
            if rank > 0 {
                masks[0][sq].0 |= 1u64 << ((rank - 1) * 8 + file + 1);
            }
        }

        if file > 0 {
            masks[1][sq].0 |= 1u64 << (rank * 8 + file - 1);
            if rank < 7 {
                masks[1][sq].0 |= 1u64 << ((rank + 1) * 8 + file - 1);
            }
        }
        if file < 7 {
            masks[1][sq].0 |= 1u64 << (rank * 8 + file + 1);
            if rank < 7 {
                masks[1][sq].0 |= 1u64 << ((rank + 1) * 8 + file + 1);
            }
        }

        sq += 1;
    }
    masks
};

/// Helper: fill north from a bitboard (flood fill)
#[inline]
pub const fn fill_north(mut bb: u64) -> u64 {
    bb |= bb << 8;
    bb |= bb << 16;
    bb |= bb << 32;
    bb
}

/// Helper: fill south from a bitboard (flood fill)
#[inline]
pub const fn fill_south(mut bb: u64) -> u64 {
    bb |= bb >> 8;
    bb |= bb >> 16;
    bb |= bb >> 32;
    bb
}

/// Get forward fill for a color
#[inline]
pub fn fill_forward(bb: Bitboard, color: Color) -> Bitboard {
    match color {
        Color::White => Bitboard(fill_north(bb.0)),
        Color::Black => Bitboard(fill_south(bb.0)),
    }
}

/// Get backward fill for a color (opposite of forward)
#[inline]
pub fn fill_backward(bb: Bitboard, color: Color) -> Bitboard {
    match color {
        Color::White => Bitboard(fill_south(bb.0)),
        Color::Black => Bitboard(fill_north(bb.0)),
    }
}

/// Passed pawn bonus by rank (from the pawn's perspective)
/// Index 0 = rank 1 (impossible for pawn), 7 = rank 8 (promotion)
pub const PASSED_PAWN_BONUS_MG: [i32; 8] = [0, 5, 10, 20, 35, 60, 100, 0];
pub const PASSED_PAWN_BONUS_EG: [i32; 8] = [0, 10, 20, 40, 70, 120, 200, 0];

/// Get the relative rank for a color (0-7 from that color's perspective)
#[inline]
pub const fn relative_rank(rank: usize, color: Color) -> usize {
    match color {
        Color::White => rank,
        Color::Black => 7 - rank,
    }
}
