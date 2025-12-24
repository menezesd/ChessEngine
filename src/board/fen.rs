use super::{
    file_to_index, format_square, rank_to_index, Board, Color, Piece, Square, CASTLE_BLACK_K,
    CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q,
};

impl Board {
    pub fn from_fen(fen: &str) -> Self {
        let mut board = Board::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        assert!(parts.len() >= 4, "FEN must have at least 4 parts");
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_ascii_digit() {
                    file += c.to_digit(10).unwrap() as usize;
                } else {
                    let color = if c.is_uppercase() {
                        Color::White
                    } else {
                        Color::Black
                    };
                    let piece = match c.to_ascii_lowercase() {
                        'p' => Piece::Pawn,
                        'n' => Piece::Knight,
                        'b' => Piece::Bishop,
                        'r' => Piece::Rook,
                        'q' => Piece::Queen,
                        'k' => Piece::King,
                        _ => panic!("Invalid piece"),
                    };
                    board.set_piece(Square(7 - rank_idx, file), color, piece);
                    file += 1;
                }
            }
        }

        board.white_to_move = parts[1] == "w";

        for c in parts[2].chars() {
            match c {
                'K' => board.castling_rights |= CASTLE_WHITE_K,
                'Q' => board.castling_rights |= CASTLE_WHITE_Q,
                'k' => board.castling_rights |= CASTLE_BLACK_K,
                'q' => board.castling_rights |= CASTLE_BLACK_Q,
                '-' => {}
                _ => panic!("Invalid castle"),
            }
        }
        board.en_passant_target = if parts[3] != "-" {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2 {
                Some(Square(rank_to_index(chars[1]), file_to_index(chars[0])))
            } else {
                None
            }
        } else {
            None
        };

        if parts.len() >= 5 {
            board.halfmove_clock = parts[4].parse().unwrap_or(0);
        }

        board.hash = board.calculate_initial_hash();
        board.repetition_counts.set(board.hash, 1);
        board
    }

    pub fn to_fen(&self) -> String {
        let mut rows: Vec<String> = Vec::new();
        for rank in (0..8).rev() {
            let mut row = String::new();
            let mut empty = 0;
            for file in 0..8 {
                let sq = Square(rank, file);
                if let Some((color, piece)) = self.piece_at(sq) {
                    if empty > 0 {
                        row.push_str(&empty.to_string());
                        empty = 0;
                    }
                    let ch = match (color, piece) {
                        (Color::White, Piece::Pawn) => 'P',
                        (Color::White, Piece::Knight) => 'N',
                        (Color::White, Piece::Bishop) => 'B',
                        (Color::White, Piece::Rook) => 'R',
                        (Color::White, Piece::Queen) => 'Q',
                        (Color::White, Piece::King) => 'K',
                        (Color::Black, Piece::Pawn) => 'p',
                        (Color::Black, Piece::Knight) => 'n',
                        (Color::Black, Piece::Bishop) => 'b',
                        (Color::Black, Piece::Rook) => 'r',
                        (Color::Black, Piece::Queen) => 'q',
                        (Color::Black, Piece::King) => 'k',
                    };
                    row.push(ch);
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
            .map(format_square)
            .unwrap_or_else(|| "-".to_string());

        format!(
            "{} {} {} {} {} 1",
            rows.join("/"),
            active,
            castling,
            ep,
            self.halfmove_clock
        )
    }
}
