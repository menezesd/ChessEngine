use crate::board::{Board, Move, Piece};

impl Board {
    /// Format a move in Standard Algebraic Notation.
    ///
    /// Returns notation like "e4", "Nf3", "Bxc6+", "O-O-O", "e8=Q#".
    #[must_use]
    pub fn move_to_san(&self, mv: &Move) -> String {
        let mut san = String::new();

        if mv.is_castling() {
            if mv.is_castle_kingside() {
                san.push_str("O-O");
            } else {
                san.push_str("O-O-O");
            }
        } else {
            let piece = self.piece_on(mv.from());

            if let Some(p) = piece {
                self.append_san_piece_prefix(&mut san, *mv, p);
            }

            if mv.is_capture() {
                san.push('x');
            }

            san.push_str(&mv.to().to_string());

            if let Some(promo) = mv.promotion() {
                san.push('=');
                san.push(promo.to_char().to_ascii_uppercase());
            }
        }

        let mut test_board = self.clone();
        test_board.make_move(*mv);

        if test_board.is_checkmate() {
            san.push('#');
        } else if test_board.is_in_check(test_board.side_to_move()) {
            san.push('+');
        }

        san
    }

    fn append_san_piece_prefix(&self, san: &mut String, mv: Move, piece: Piece) {
        if piece == Piece::Pawn {
            if mv.is_capture() {
                san.push((b'a' + mv.from().file() as u8) as char);
            }
            return;
        }

        san.push(piece.to_char().to_ascii_uppercase());
        let (needs_file, needs_rank) = self.needs_disambiguation(mv, piece);
        if needs_file {
            san.push((b'a' + mv.from().file() as u8) as char);
        }
        if needs_rank {
            san.push((b'1' + mv.from().rank() as u8) as char);
        }
    }

    /// Determine if disambiguation is needed for a piece move.
    /// Returns (`needs_file`, `needs_rank`).
    pub(super) fn needs_disambiguation(&self, mv: Move, piece: Piece) -> (bool, bool) {
        let mut board_copy = self.clone();
        let moves = board_copy.generate_moves();
        let same_dest_moves: Vec<&Move> = moves
            .iter()
            .filter(|m| {
                m.to() == mv.to() && self.piece_on(m.from()) == Some(piece) && m.from() != mv.from()
            })
            .collect();

        if same_dest_moves.is_empty() {
            return (false, false);
        }

        let same_file = same_dest_moves
            .iter()
            .any(|m| m.from().file() == mv.from().file());
        let same_rank = same_dest_moves
            .iter()
            .any(|m| m.from().rank() == mv.from().rank());

        match (same_file, same_rank) {
            (false, _) => (true, false),
            (true, false) => (false, true),
            (true, true) => (true, true),
        }
    }
}
