//! Zobrist hashing for chess positions.
//!
//! Provides incrementally-updatable 64-bit position hashes for transposition tables.

use rand::prelude::*;

pub(crate) struct ZobristKeys {
    // piece_keys[piece_type][color][square_index]
    pub(crate) piece_keys: [[[u64; 64]; 2]; 6], // PieceType(0-5), Color(0-1), Square(0-63)
    pub(crate) black_to_move_key: u64,
    // Combined key indexed by the four castling rights bits.
    pub(crate) castling_keys: [u64; 16],
    // en_passant_keys[file_index] (only file matters for EP target)
    pub(crate) en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64); // Use a fixed seed for reproducibility
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut en_passant_keys = [0; 8];

        for piece in &mut piece_keys {
            for color in piece.iter_mut() {
                for key in color.iter_mut() {
                    *key = rng.gen();
                }
            }
        }

        let black_to_move_key = rng.gen();

        // Draw individual keys in the same order as the rights bits.
        let right_keys: [u64; 4] = std::array::from_fn(|_| rng.gen());
        let mut castling_keys = [0; 16];
        for mask in 1..16usize {
            castling_keys[mask] =
                castling_keys[mask & (mask - 1)] ^ right_keys[mask.trailing_zeros() as usize];
        }

        for key in &mut en_passant_keys {
            *key = rng.gen();
        }

        ZobristKeys {
            piece_keys,
            black_to_move_key,
            castling_keys,
            en_passant_keys,
        }
    }
}

// Initialize Zobrist keys lazily and globally
pub(crate) static ZOBRIST: std::sync::LazyLock<ZobristKeys> =
    std::sync::LazyLock::new(ZobristKeys::new);
