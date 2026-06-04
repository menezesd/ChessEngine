use crate::board::error::MoveParseError;
use crate::board::{Board, Move, Piece, Square};

fn parse_uci_square(file: u8, rank: u8, notation: &str) -> Result<Square, MoveParseError> {
    let file_idx = match file {
        b'a'..=b'h' => usize::from(file - b'a'),
        _ => {
            return Err(MoveParseError::InvalidSquare {
                notation: notation.to_string(),
            });
        }
    };
    let rank_idx = match rank {
        b'1'..=b'8' => usize::from(rank - b'1'),
        _ => {
            return Err(MoveParseError::InvalidSquare {
                notation: notation.to_string(),
            });
        }
    };

    Square::try_new(rank_idx, file_idx).ok_or_else(|| MoveParseError::InvalidSquare {
        notation: notation.to_string(),
    })
}

impl Board {
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

        let bytes = uci.as_bytes();

        let from_sq = parse_uci_square(bytes[0], bytes[1], uci)?;
        let to_sq = parse_uci_square(bytes[2], bytes[3], uci)?;

        let promotion = if uci.len() == 5 {
            let promotion_char = char::from(bytes[4]);
            let piece =
                Piece::from_char(promotion_char).ok_or(MoveParseError::InvalidPromotion {
                    char: promotion_char,
                })?;
            if matches!(piece, Piece::Pawn | Piece::King) {
                return Err(MoveParseError::InvalidPromotion {
                    char: promotion_char,
                });
            }
            Some(piece)
        } else {
            None
        };

        let legal_moves = self.generate_moves();
        for legal_move in &legal_moves {
            if legal_move.from() == from_sq
                && legal_move.to() == to_sq
                && legal_move.promotion() == promotion
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
        self.make_move(mv);
        Ok(mv)
    }
}
