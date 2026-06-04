use crate::board::types::Bitboard;

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
