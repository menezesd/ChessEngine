use std::str::FromStr;

use super::error::FenError;
use super::{
    Board, Color, Piece, Square, CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q,
};

mod format;
mod uci_move;

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
        let mut rank_count = 0;
        for (rank_idx, rank_str) in parts[0].split('/').enumerate() {
            rank_count += 1;
            if rank_idx >= 8 {
                return Err(FenError::InvalidRank { rank: rank_idx });
            }
            let mut file = 0;
            for c in rank_str.chars() {
                if c.is_ascii_digit() {
                    let empty_squares = c.to_digit(10).expect("ASCII digit") as usize;
                    if !(1..=8).contains(&empty_squares) {
                        return Err(FenError::InvalidEmptySquareCount { char: c });
                    }
                    file += empty_squares;
                    if file > 8 {
                        return Err(FenError::TooManyFiles {
                            rank: rank_idx,
                            files: file,
                        });
                    }
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
                    board.set_piece(Square::new(7 - rank_idx, file), color, piece);
                    file += 1;
                }
            }
            if file < 8 {
                return Err(FenError::TooFewFiles {
                    rank: rank_idx,
                    files: file,
                });
            }
        }
        if rank_count < 8 {
            return Err(FenError::TooFewRanks { found: rank_count });
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
            let bytes = parts[3].as_bytes();
            if bytes.len() == 2
                && (b'a'..=b'h').contains(&bytes[0])
                && (b'1'..=b'8').contains(&bytes[1])
            {
                let file = (bytes[0] - b'a') as usize;
                let rank = (bytes[1] - b'1') as usize;
                Some(Square::new(rank, file))
            } else {
                return Err(FenError::InvalidEnPassant {
                    found: parts[3].to_string(),
                });
            }
        };

        // Parse halfmove clock (optional)
        if parts.len() >= 5 {
            board.halfmove_clock =
                parts[4]
                    .parse()
                    .map_err(|_| FenError::InvalidHalfmoveClock {
                        found: parts[4].to_string(),
                    })?;
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
}

impl FromStr for Board {
    type Err = FenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Board::try_from_fen(s)
    }
}

#[cfg(test)]
mod tests;
