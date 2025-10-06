use once_cell::sync::Lazy;
use rand::prelude::*;

use crate::types::{Color, Piece, Square};

pub struct ZobristKeys {
    pub piece_keys: [[[u64; 64]; 2]; 6],
    pub black_to_move_key: u64,
    pub castling_keys: [[u64; 2]; 2],
    pub en_passant_keys: [u64; 8],
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = StdRng::seed_from_u64(1234567890_u64);
        let mut piece_keys = [[[0; 64]; 2]; 6];
        let mut castling_keys = [[0; 2]; 2];
        let mut en_passant_keys = [0; 8];

        for p in piece_keys.iter_mut() {
            for c in p.iter_mut() {
                for sq in c.iter_mut() {
                    *sq = rng.gen();
                }
            }
        }

        let black_to_move_key = rng.gen();

        for row in castling_keys.iter_mut() {
            for slot in row.iter_mut() {
                *slot = rng.gen();
            }
        }

        for slot in en_passant_keys.iter_mut() {
            *slot = rng.gen();
        }

        ZobristKeys {
            piece_keys,
            black_to_move_key,
            castling_keys,
            en_passant_keys,
        }
    }
}

pub static ZOBRIST: Lazy<ZobristKeys> = Lazy::new(ZobristKeys::new);

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

pub fn color_to_zobrist_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

pub fn square_to_zobrist_index(sq: Square) -> usize {
    sq.0 * 8 + sq.1
}
