use crate::board::types::Bitboard;

/// King zone masks - 9 squares around the king (or fewer at edges)
/// Used for king safety evaluation
pub const KING_ZONE: [Bitboard; 64] = {
    let mut masks = [Bitboard(0); 64];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        let mut dr: i32 = -1;
        while dr <= 1 {
            let mut df: i32 = -1;
            while df <= 1 {
                let nr = rank as i32 + dr;
                let nf = file as i32 + df;
                if nr >= 0 && nr < 8 && nf >= 0 && nf < 8 {
                    masks[sq].0 |= 1u64 << (nr * 8 + nf);
                }
                df += 1;
            }
            dr += 1;
        }

        sq += 1;
    }
    masks
};

/// Extended king zone for attack evaluation (includes squares in front of king)
/// This helps evaluate attacks on castled king positions
pub const KING_ZONE_EXTENDED: [[Bitboard; 64]; 2] = {
    let mut masks = [[Bitboard(0); 64]; 2];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        masks[0][sq] = KING_ZONE[sq];
        masks[1][sq] = KING_ZONE[sq];

        if rank < 6 {
            if file > 0 {
                masks[0][sq].0 |= 1u64 << ((rank + 2) * 8 + file - 1);
            }
            masks[0][sq].0 |= 1u64 << ((rank + 2) * 8 + file);
            if file < 7 {
                masks[0][sq].0 |= 1u64 << ((rank + 2) * 8 + file + 1);
            }
        }

        if rank > 1 {
            if file > 0 {
                masks[1][sq].0 |= 1u64 << ((rank - 2) * 8 + file - 1);
            }
            masks[1][sq].0 |= 1u64 << ((rank - 2) * 8 + file);
            if file < 7 {
                masks[1][sq].0 |= 1u64 << ((rank - 2) * 8 + file + 1);
            }
        }

        sq += 1;
    }
    masks
};

/// King attack table: converts attack units to centipawn score
/// Uses formula: 480 * i² / (i² + 4200) for diminishing returns
pub const KING_ATTACK_TABLE: [i32; 256] = {
    let mut table = [0i32; 256];
    let mut i = 0;
    while i < 256 {
        let i_sq = (i * i) as i64;
        table[i] = ((480 * i_sq) / (i_sq + 4200)) as i32;
        i += 1;
    }
    table
};

/// Pawn shield masks - squares in front of king that should have pawns
/// `PAWN_SHIELD_MASK`[color][king_file] = squares where shield pawns should be
pub const PAWN_SHIELD_MASK: [[Bitboard; 8]; 2] = {
    let mut masks = [[Bitboard(0); 8]; 2];

    let mut f = 0;
    while f < 8 {
        if f > 0 {
            masks[0][f].0 |= 1u64 << (8 + f - 1);
            masks[0][f].0 |= 1u64 << (2 * 8 + f - 1);
        }
        masks[0][f].0 |= 1u64 << (8 + f);
        masks[0][f].0 |= 1u64 << (2 * 8 + f);
        if f < 7 {
            masks[0][f].0 |= 1u64 << (8 + f + 1);
            masks[0][f].0 |= 1u64 << (2 * 8 + f + 1);
        }
        f += 1;
    }

    f = 0;
    while f < 8 {
        if f > 0 {
            masks[1][f].0 |= 1u64 << (6 * 8 + f - 1);
            masks[1][f].0 |= 1u64 << (5 * 8 + f - 1);
        }
        masks[1][f].0 |= 1u64 << (6 * 8 + f);
        masks[1][f].0 |= 1u64 << (5 * 8 + f);
        if f < 7 {
            masks[1][f].0 |= 1u64 << (6 * 8 + f + 1);
            masks[1][f].0 |= 1u64 << (5 * 8 + f + 1);
        }
        f += 1;
    }

    masks
};

/// 7th rank for each color (rank where rooks are powerful)
pub const RANK_7TH: [Bitboard; 2] = [Bitboard::RANK_7, Bitboard::RANK_2];
