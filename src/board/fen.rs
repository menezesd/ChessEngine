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
        if parts.len() > 6 {
            return Err(FenError::TooManyParts { found: parts.len() });
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

        parse_castling_rights(&mut board, parts[2])?;
        board.en_passant_target = parse_en_passant_target(parts[3], board.white_to_move)?;

        parse_move_counters(&mut board, &parts)?;

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

fn parse_castling_rights(board: &mut Board, castling: &str) -> Result<(), FenError> {
    if castling != "-" && castling.contains('-') {
        return Err(FenError::InvalidCastling { char: '-' });
    }

    for c in castling.chars() {
        let right = match c {
            'K' => CASTLE_WHITE_K,
            'Q' => CASTLE_WHITE_Q,
            'k' => CASTLE_BLACK_K,
            'q' => CASTLE_BLACK_Q,
            '-' => continue,
            _ => return Err(FenError::InvalidCastling { char: c }),
        };
        if board.castling_rights & right != 0 {
            return Err(FenError::InvalidCastling { char: c });
        }
        board.castling_rights |= right;
    }

    Ok(())
}

fn parse_en_passant_target(target: &str, white_to_move: bool) -> Result<Option<Square>, FenError> {
    if target == "-" {
        return Ok(None);
    }

    let bytes = target.as_bytes();
    if bytes.len() != 2 || !(b'a'..=b'h').contains(&bytes[0]) || !matches!(bytes[1], b'3' | b'6') {
        return Err(FenError::InvalidEnPassant {
            found: target.to_string(),
        });
    }

    let rank = (bytes[1] - b'1') as usize;
    let expected_rank = if white_to_move { 5 } else { 2 };
    if rank != expected_rank {
        return Err(FenError::InvalidEnPassant {
            found: target.to_string(),
        });
    }

    Ok(Some(Square::new(rank, (bytes[0] - b'a') as usize)))
}

fn parse_move_counters(board: &mut Board, parts: &[&str]) -> Result<(), FenError> {
    if let Some(halfmove) = parts.get(4) {
        board.halfmove_clock = halfmove
            .parse()
            .map_err(|_| FenError::InvalidHalfmoveClock {
                found: (*halfmove).to_string(),
            })?;
    }

    if let Some(fullmove) = parts.get(5) {
        board.fullmove_number = fullmove
            .parse()
            .ok()
            .filter(|number: &u32| *number > 0)
            .ok_or_else(|| FenError::InvalidFullmoveNumber {
                found: (*fullmove).to_string(),
            })?;
    }

    Ok(())
}

impl FromStr for Board {
    type Err = FenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Board::try_from_fen(s)
    }
}

#[cfg(test)]
mod tests;
