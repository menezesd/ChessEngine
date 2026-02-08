//! Pre-computed bitboard masks for evaluation.
//!
//! Contains masks for pawn structure evaluation, king safety zones,
//! and attack unit conversion tables.

use super::types::{Bitboard, Color};

/// Files adjacent to each file (0-7)
/// e.g., `ADJACENT_FILES`[3] = files c and e for file d
pub const ADJACENT_FILES: [Bitboard; 8] = {
    let mut masks = [Bitboard(0); 8];
    let mut f = 0;
    while f < 8 {
        let mut adj = 0u64;
        if f > 0 {
            adj |= Bitboard::FILE_A.0 << (f - 1);
        }
        if f < 7 {
            adj |= Bitboard::FILE_A.0 << (f + 1);
        }
        masks[f] = Bitboard(adj);
        f += 1;
    }
    masks
};

/// Files for a given file index (convenience array)
pub const FILES: [Bitboard; 8] = [
    Bitboard::FILE_A,
    Bitboard::FILE_B,
    Bitboard::FILE_C,
    Bitboard::FILE_D,
    Bitboard::FILE_E,
    Bitboard::FILE_F,
    Bitboard::FILE_G,
    Bitboard::FILE_H,
];

/// Passed pawn masks - squares that would block a pawn from being passed
/// `PASSED_PAWN_MASK`[color][square] = enemy pawns on these squares block passed status
pub const PASSED_PAWN_MASK: [[Bitboard; 64]; 2] = {
    let mut masks = [[Bitboard(0); 64]; 2];

    // White pawns advance north (increasing rank)
    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        // White: check all ranks ahead
        if rank < 7 {
            let mut r = rank + 1;
            while r < 8 {
                // Same file
                masks[0][sq].0 |= 1u64 << (r * 8 + file);
                // Adjacent files
                if file > 0 {
                    masks[0][sq].0 |= 1u64 << (r * 8 + file - 1);
                }
                if file < 7 {
                    masks[0][sq].0 |= 1u64 << (r * 8 + file + 1);
                }
                r += 1;
            }
        }

        // Black: check all ranks behind (from black's perspective, lower ranks)
        if rank > 0 {
            let mut r = 0;
            while r < rank {
                // Same file
                masks[1][sq].0 |= 1u64 << (r * 8 + file);
                // Adjacent files
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

        // White: support comes from same rank or behind (lower ranks) on adjacent files
        if file > 0 {
            // Pawn to the left on same rank (phalanx)
            masks[0][sq].0 |= 1u64 << (rank * 8 + file - 1);
            // Pawns behind and to the left
            if rank > 0 {
                masks[0][sq].0 |= 1u64 << ((rank - 1) * 8 + file - 1);
            }
        }
        if file < 7 {
            // Pawn to the right on same rank (phalanx)
            masks[0][sq].0 |= 1u64 << (rank * 8 + file + 1);
            // Pawns behind and to the right
            if rank > 0 {
                masks[0][sq].0 |= 1u64 << ((rank - 1) * 8 + file + 1);
            }
        }

        // Black: support comes from same rank or behind (higher ranks) on adjacent files
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

/// King zone masks - 9 squares around the king (or fewer at edges)
/// Used for king safety evaluation
pub const KING_ZONE: [Bitboard; 64] = {
    let mut masks = [Bitboard(0); 64];

    let mut sq = 0;
    while sq < 64 {
        let rank = sq / 8;
        let file = sq % 8;

        // Add all 8 surrounding squares + the king square itself
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

        // Start with basic king zone
        masks[0][sq] = KING_ZONE[sq];
        masks[1][sq] = KING_ZONE[sq];

        // For White king, extend upward (more squares in front)
        if rank < 6 {
            if file > 0 {
                masks[0][sq].0 |= 1u64 << ((rank + 2) * 8 + file - 1);
            }
            masks[0][sq].0 |= 1u64 << ((rank + 2) * 8 + file);
            if file < 7 {
                masks[0][sq].0 |= 1u64 << ((rank + 2) * 8 + file + 1);
            }
        }

        // For Black king, extend downward
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

    // White king shield (on ranks 2-3)
    let mut f = 0;
    while f < 8 {
        // Shield on ranks 2 and 3
        if f > 0 {
            masks[0][f].0 |= 1u64 << (8 + f - 1); // Rank 2, left
            masks[0][f].0 |= 1u64 << (2 * 8 + f - 1); // Rank 3, left
        }
        masks[0][f].0 |= 1u64 << (8 + f); // Rank 2, same file
        masks[0][f].0 |= 1u64 << (2 * 8 + f); // Rank 3, same file
        if f < 7 {
            masks[0][f].0 |= 1u64 << (8 + f + 1); // Rank 2, right
            masks[0][f].0 |= 1u64 << (2 * 8 + f + 1); // Rank 3, right
        }
        f += 1;
    }

    // Black king shield (on ranks 7 and 6)
    f = 0;
    while f < 8 {
        if f > 0 {
            masks[1][f].0 |= 1u64 << (6 * 8 + f - 1); // Rank 7, left
            masks[1][f].0 |= 1u64 << (5 * 8 + f - 1); // Rank 6, left
        }
        masks[1][f].0 |= 1u64 << (6 * 8 + f); // Rank 7, same file
        masks[1][f].0 |= 1u64 << (5 * 8 + f); // Rank 6, same file
        if f < 7 {
            masks[1][f].0 |= 1u64 << (6 * 8 + f + 1); // Rank 7, right
            masks[1][f].0 |= 1u64 << (5 * 8 + f + 1); // Rank 6, right
        }
        f += 1;
    }

    masks
};

/// 7th rank for each color (rank where rooks are powerful)
pub const RANK_7TH: [Bitboard; 2] = [Bitboard::RANK_7, Bitboard::RANK_2];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacent_files() {
        // File A should only have file B adjacent
        assert_eq!(ADJACENT_FILES[0], Bitboard::FILE_B);
        // File D should have files C and E adjacent
        assert_eq!(ADJACENT_FILES[3].0, Bitboard::FILE_C.0 | Bitboard::FILE_E.0);
        // File H should only have file G adjacent
        assert_eq!(ADJACENT_FILES[7], Bitboard::FILE_G);
    }

    #[test]
    fn test_king_zone() {
        // Corner king (a1) should have 4 squares
        assert_eq!(KING_ZONE[0].popcount(), 4);
        // Edge king (a4) should have 6 squares
        assert_eq!(KING_ZONE[24].popcount(), 6);
        // Center king (d4) should have 9 squares
        assert_eq!(KING_ZONE[27].popcount(), 9);
    }

    #[test]
    fn test_king_attack_table() {
        // Low attack units = low score
        assert!(KING_ATTACK_TABLE[0] == 0);
        assert!(KING_ATTACK_TABLE[5] < 50);
        // Formula: 480 * i² / (i² + 4200)
        // i=50: 480*2500/6700 ≈ 179
        assert!(KING_ATTACK_TABLE[50] > 150);
        // i=100: 480*10000/14200 ≈ 338
        assert!(KING_ATTACK_TABLE[100] > 300);
        // Should never exceed ~480 (asymptote)
        assert!(KING_ATTACK_TABLE[255] < 480);
    }

    #[test]
    fn test_passed_pawn_mask_white() {
        // White pawn on e4 - check that mask includes e5-e7 and d5-d7, f5-f7
        let mask = PASSED_PAWN_MASK[0][28]; // e4
                                            // Should include e5 (36)
        assert!((mask.0 & (1u64 << 36)) != 0, "e5 should be in mask");
        // Should include d5 (35)
        assert!((mask.0 & (1u64 << 35)) != 0, "d5 should be in mask");
        // Should include f5 (37)
        assert!((mask.0 & (1u64 << 37)) != 0, "f5 should be in mask");
        // Should NOT include e4 itself
        assert!((mask.0 & (1u64 << 28)) == 0, "e4 should not be in mask");
    }

    #[test]
    fn test_passed_pawn_mask_black() {
        // Black pawn on e5 - check that mask includes e4-e2 and d4-d2, f4-f2
        let mask = PASSED_PAWN_MASK[1][36]; // e5
                                            // Should include e4 (28)
        assert!((mask.0 & (1u64 << 28)) != 0, "e4 should be in mask");
        // Should include d4 (27)
        assert!((mask.0 & (1u64 << 27)) != 0, "d4 should be in mask");
    }

    #[test]
    fn test_pawn_shield_mask() {
        // White king on g-file - shield should include f2, g2, h2, f3, g3, h3
        let mask = PAWN_SHIELD_MASK[0][6]; // g-file
        assert!((mask.0 & (1u64 << 13)) != 0, "f2 should be in shield");
        assert!((mask.0 & (1u64 << 14)) != 0, "g2 should be in shield");
        assert!((mask.0 & (1u64 << 15)) != 0, "h2 should be in shield");
    }

    #[test]
    fn test_fill_north() {
        // Single bit on a1 should fill the entire a-file
        let filled = fill_north(1);
        assert_eq!(filled, Bitboard::FILE_A.0);
    }

    #[test]
    fn test_fill_south() {
        // Single bit on a8 should fill the entire a-file
        let filled = fill_south(1u64 << 56);
        assert_eq!(filled, Bitboard::FILE_A.0);
    }

    #[test]
    fn test_relative_rank() {
        // White's rank 0 is their first rank
        assert_eq!(relative_rank(0, Color::White), 0);
        // Black's rank 7 is their first rank
        assert_eq!(relative_rank(7, Color::Black), 0);
        // White's rank 7 is promotion rank
        assert_eq!(relative_rank(7, Color::White), 7);
        // Black's rank 0 is promotion rank
        assert_eq!(relative_rank(0, Color::Black), 7);
    }

    #[test]
    fn test_files_array() {
        assert_eq!(FILES[0], Bitboard::FILE_A);
        assert_eq!(FILES[7], Bitboard::FILE_H);
    }
}
