//! Zobrist hashing for chess positions.
//!
//! Provides incrementally-updatable 64-bit position hashes for transposition tables.

use rand::prelude::*;

use crate::board::{Color, Piece, Square};
pub(crate) struct ZobristKeys {
    // piece_keys[piece_type][color][square_index]
    pub(crate) piece_keys: [[[u64; 64]; 2]; 6], // PieceType(0-5), Color(0-1), Square(0-63)
    pub(crate) black_to_move_key: u64,
    // castling_keys[color][side] : 0=White, 1=Black; 0=Kingside, 1=Queenside
    pub(crate) castling_keys: [[u64; 2]; 2],
    // en_passant_keys[file_index] (only file matters for EP target)
    pub(crate) en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64); // Use a fixed seed for reproducibility
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut castling_keys = [[0; 2]; 2];
        let mut en_passant_keys = [0; 8];

        for piece in &mut piece_keys {
            for color in piece.iter_mut() {
                for key in color.iter_mut() {
                    *key = rng.gen();
                }
            }
        }

        let black_to_move_key = rng.gen();

        for color in &mut castling_keys {
            for key in color.iter_mut() {
                *key = rng.gen();
            }
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
pub(crate) static ZOBRIST: std::sync::LazyLock<ZobristKeys> = std::sync::LazyLock::new(ZobristKeys::new);

// Re-export simple index accessors for Zobrist hashing
// These use the existing index() methods on Piece, Color, and Square
#[inline]
pub(crate) fn piece_to_zobrist_index(piece: Piece) -> usize {
    piece.index()
}

#[inline]
pub(crate) fn color_to_zobrist_index(color: Color) -> usize {
    color.index()
}

#[inline]
pub(crate) fn square_to_zobrist_index(sq: Square) -> usize {
    sq.index().as_usize()
}
