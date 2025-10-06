use once_cell::sync::Lazy;
use rand::prelude::*;
use crate::types::{Piece, Color, Square};

/// Struct to hold all Zobrist keys
pub struct ZobristKeys {
    pub piece_keys: [[[u64; 64]; 2]; 6],
    pub black_to_move_key: u64,
    pub castling_keys: [[u64; 2]; 2],
    pub en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    pub fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64);
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut castling_keys = [[0; 2]; 2];
        let mut en_passant_keys = [0; 8];

        for p_idx in 0..6 {
            for c_idx in 0..2 {
                for sq_idx in 0..64 {
                    piece_keys[p_idx][c_idx][sq_idx] = rng.gen();
                }
            }
        }

        let black_to_move_key = rng.gen();

        for c_idx in 0..2 {
            for side_idx in 0..2 {
                castling_keys[c_idx][side_idx] = rng.gen();
            }
        }

        for f_idx in 0..8 {
            en_passant_keys[f_idx] = rng.gen();
        }

        ZobristKeys { piece_keys, black_to_move_key, castling_keys, en_passant_keys }
    }
}

/// Global Zobrist keys
pub static ZOBRIST: Lazy<ZobristKeys> = Lazy::new(ZobristKeys::new);

/// Helper to map Piece enum to index
pub fn piece_to_zobrist_index(piece: Piece) -> usize {
    match piece {
        Piece::Pawn => 0,
        Piece::Knight => 1,
        Piece::Bishop => 2,
        Piece::Rook => 3,
        Piece::Queen => 4,
        Piece::King => 5,
    }
}

/// Helper to map Color enum to index
pub fn color_to_zobrist_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

/// Helper to map Square to index (0-63)
pub fn square_to_zobrist_index(sq: Square) -> usize {
    sq.0 * 8 + sq.1
}
