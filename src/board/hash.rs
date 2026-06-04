use crate::zobrist::{
    color_to_zobrist_index, piece_to_zobrist_index, square_to_zobrist_index, ZOBRIST,
};

use super::{Board, Square, CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q};

impl Board {
    pub(crate) fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        for r in 0..8 {
            for f in 0..8 {
                let sq = Square::new(r, f);
                if let Some((color, piece)) = self.piece_at(sq) {
                    let sq_idx = square_to_zobrist_index(sq);
                    let p_idx = piece_to_zobrist_index(piece);
                    let c_idx = color_to_zobrist_index(color);
                    hash ^= ZOBRIST.piece_keys[p_idx][c_idx][sq_idx];
                }
            }
        }

        if !self.white_to_move {
            hash ^= ZOBRIST.black_to_move_key;
        }

        if self.castling_rights & CASTLE_WHITE_K != 0 {
            hash ^= ZOBRIST.castling_keys[0][0];
        }
        if self.castling_rights & CASTLE_WHITE_Q != 0 {
            hash ^= ZOBRIST.castling_keys[0][1];
        }
        if self.castling_rights & CASTLE_BLACK_K != 0 {
            hash ^= ZOBRIST.castling_keys[1][0];
        }
        if self.castling_rights & CASTLE_BLACK_Q != 0 {
            hash ^= ZOBRIST.castling_keys[1][1];
        }

        if let Some(ep_square) = self.en_passant_target {
            hash ^= ZOBRIST.en_passant_keys[ep_square.file()];
        }

        hash
    }
}
