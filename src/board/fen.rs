use std::str::FromStr;

use super::error::{FenError, MoveParseError};
use super::{
    file_to_index, rank_to_index, Board, Color, Move, Piece, Square, CASTLE_BLACK_K,
    CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q,
};

impl Board {
    /// Parse a board position from FEN notation.
    ///
    /// Returns an error if the FEN string is invalid.
    pub fn try_from_fen(fen: &str) -> Result<Self, FenError> {
        let mut board = Board::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();

        if parts.len() < 4 {
            return Err(FenError::TooFewParts { found: parts.len() });
        }

        // Parse piece placement
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            if rank_idx >= 8 {
                return Err(FenError::InvalidRank { rank: rank_idx });
            }
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
                    let piece = Piece::from_char(c).ok_or(FenError::InvalidPiece { char: c })?;
                    if file >= 8 {
                        return Err(FenError::TooManyFiles {
                            rank: rank_idx,
                            files: file + 1,
                        });
                    }
                    board.set_piece(Square(7 - rank_idx, file), color, piece);
                    file += 1;
                }
            }
        }

        // Parse side to move
        match parts[1] {
            "w" => board.white_to_move = true,
            "b" => board.white_to_move = false,
            other => {
                return Err(FenError::InvalidSideToMove {
                    found: other.to_string(),
                })
            }
        }

        // Parse castling rights
        for c in parts[2].chars() {
            match c {
                'K' => board.castling_rights |= CASTLE_WHITE_K,
                'Q' => board.castling_rights |= CASTLE_WHITE_Q,
                'k' => board.castling_rights |= CASTLE_BLACK_K,
                'q' => board.castling_rights |= CASTLE_BLACK_Q,
                '-' => {}
                _ => return Err(FenError::InvalidCastling { char: c }),
            }
        }

        // Parse en passant target
        board.en_passant_target = if parts[3] == "-" {
            None
        } else {
            let chars: Vec<char> = parts[3].chars().collect();
            if chars.len() == 2
                && ('a'..='h').contains(&chars[0])
                && ('1'..='8').contains(&chars[1])
            {
                Some(Square(rank_to_index(chars[1]), file_to_index(chars[0])))
            } else {
                return Err(FenError::InvalidEnPassant {
                    found: parts[3].to_string(),
                });
            }
        };

        // Parse halfmove clock (optional)
        if parts.len() >= 5 {
            board.halfmove_clock = parts[4].parse().unwrap_or(0);
        }

        board.hash = board.calculate_initial_hash();
        board.repetition_counts.set(board.hash, 1);
        board.recalculate_incremental_eval();
        Ok(board)
    }

    /// Parse a board position from FEN notation.
    ///
    /// # Panics
    /// Panics if the FEN string is invalid. Use `try_from_fen` for fallible parsing.
    #[must_use]
    pub fn from_fen(fen: &str) -> Self {
        Self::try_from_fen(fen).expect("Invalid FEN string")
    }

    /// Convert the board position to FEN notation.
    #[must_use]
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
            .en_passant_target.map_or_else(|| "-".to_string(), |sq| sq.to_string());

        format!(
            "{} {} {} {} {} 1",
            rows.join("/"),
            active,
            castling,
            ep,
            self.halfmove_clock
        )
    }

    /// Parse a move in UCI long algebraic notation (e.g., "e2e4", "e7e8q").
    ///
    /// Returns the matching legal move if found, or an error describing why parsing failed.
    ///
    /// # Example
    /// ```
    /// use chess_engine::board::Board;
    ///
    /// let mut board = Board::new();
    /// let mv = board.parse_move("e2e4").unwrap();
    /// assert_eq!(mv.to_string(), "e2e4");
    /// ```
    pub fn parse_move(&mut self, uci: &str) -> Result<Move, MoveParseError> {
        if uci.len() < 4 || uci.len() > 5 {
            return Err(MoveParseError::InvalidLength { len: uci.len() });
        }

        let chars: Vec<char> = uci.chars().collect();

        // Validate square characters
        if !('a'..='h').contains(&chars[0])
            || !('1'..='8').contains(&chars[1])
            || !('a'..='h').contains(&chars[2])
            || !('1'..='8').contains(&chars[3])
        {
            return Err(MoveParseError::InvalidSquare {
                notation: uci.to_string(),
            });
        }

        let from_sq = Square(rank_to_index(chars[1]), file_to_index(chars[0]));
        let to_sq = Square(rank_to_index(chars[3]), file_to_index(chars[2]));

        // Parse promotion piece if present
        let promotion = if uci.len() == 5 {
            let piece = Piece::from_char(chars[4]).ok_or(MoveParseError::InvalidPromotion {
                char: chars[4],
            })?;
            if matches!(piece, Piece::Pawn | Piece::King) {
                return Err(MoveParseError::InvalidPromotion { char: chars[4] });
            }
            Some(piece)
        } else {
            None
        };

        // Find matching legal move
        let legal_moves = self.generate_moves();
        for legal_move in &legal_moves {
            if legal_move.from == from_sq
                && legal_move.to == to_sq
                && legal_move.promotion == promotion
            {
                return Ok(*legal_move);
            }
        }

        Err(MoveParseError::IllegalMove {
            notation: uci.to_string(),
        })
    }

    /// Parse a UCI move and make it on the board in one call.
    ///
    /// This is a convenience method combining `parse_move` and `make_move`.
    ///
    /// # Example
    /// ```
    /// use chess_engine::board::Board;
    ///
    /// let mut board = Board::new();
    /// board.make_move_uci("e2e4").unwrap();
    /// board.make_move_uci("e7e5").unwrap();
    /// ```
    pub fn make_move_uci(&mut self, uci: &str) -> Result<Move, MoveParseError> {
        let mv = self.parse_move(uci)?;
        self.make_move(&mv);
        Ok(mv)
    }
}

impl FromStr for Board {
    type Err = FenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Board::try_from_fen(s)
    }
}
