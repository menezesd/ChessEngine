use crate::zobrist::ZOBRIST;

use super::{Board, ALL_CASTLING_RIGHTS};

impl Board {
    pub(crate) fn calculate_initial_hash(&self) -> u64 {
        let mut hash: u64 = 0;

        for (square, contents) in self.mailbox.iter().enumerate() {
            if let Some((color, piece)) = contents {
                hash ^= ZOBRIST.piece_keys[piece.index()][color.index()][square];
            }
        }

        if !self.white_to_move {
            hash ^= ZOBRIST.black_to_move_key;
        }

        hash ^= ZOBRIST.castling_keys[usize::from(self.castling_rights & ALL_CASTLING_RIGHTS)];

        if let Some(ep_square) = self.en_passant_target {
            hash ^= self.en_passant_hash_component(ep_square, self.side_to_move());
        }

        hash
    }
}
