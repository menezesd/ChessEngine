use crate::board::{Board, Square, CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q};

impl Board {
    /// Convert the board position to FEN notation.
    #[must_use]
    pub fn to_fen(&self) -> String {
        let mut rows: Vec<String> = Vec::new();
        for rank in (0..8).rev() {
            let mut row = String::new();
            let mut empty = 0;
            for file in 0..8 {
                let sq = Square::new(rank, file);
                if let Some((color, piece)) = self.piece_at(sq) {
                    if empty > 0 {
                        row.push_str(&empty.to_string());
                        empty = 0;
                    }
                    row.push(piece.to_fen_char(color));
                } else {
                    empty += 1;
                }
            }
            if empty > 0 {
                row.push_str(&empty.to_string());
            }
            rows.push(row);
        }

        let active = if self.white_to_move { "w" } else { "b" };
        let mut castling = String::new();
        if self.castling_rights & CASTLE_WHITE_K != 0 {
            castling.push('K');
        }
        if self.castling_rights & CASTLE_WHITE_Q != 0 {
            castling.push('Q');
        }
        if self.castling_rights & CASTLE_BLACK_K != 0 {
            castling.push('k');
        }
        if self.castling_rights & CASTLE_BLACK_Q != 0 {
            castling.push('q');
        }
        if castling.is_empty() {
            castling.push('-');
        }
        let ep = self
            .en_passant_target
            .map_or_else(|| "-".to_string(), |sq| sq.to_string());

        format!(
            "{} {} {} {} {} {}",
            rows.join("/"),
            active,
            castling,
            ep,
            self.halfmove_clock,
            self.fullmove_number
        )
    }
}
