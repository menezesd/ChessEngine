//! Standard Algebraic Notation (SAN) support.
//!
//! SAN is the standard human-readable chess notation used in scoresheets,
//! books, and GUIs. Examples: "e4", "Nf3", "Bxc6+", "O-O", "e8=Q#"
//!
//! # Examples
//! ```
//! use chess_engine::board::Board;
//!
//! let mut board = Board::new();
//! let mv = board.parse_san("e4").unwrap();
//! assert_eq!(board.move_to_san(&mv), "e4");
//! ```

use super::error::SanError;
use super::{Board, Move, Piece, Square};

type SanParseResult = (Option<usize>, Option<usize>, bool, Vec<char>, Option<Piece>);

impl Board {
    /// Format a move in Standard Algebraic Notation.
    ///
    /// Returns notation like "e4", "Nf3", "Bxc6+", "O-O-O", "e8=Q#"
    #[must_use]
    pub fn move_to_san(&self, mv: &Move) -> String {
        let mut san = String::new();

        // Handle castling
        if mv.is_castling() {
            if mv.is_castle_kingside() {
                san.push_str("O-O");
            } else {
                san.push_str("O-O-O");
            }
        } else {
            let piece = self.piece_on(mv.from());

            // Add piece letter (except for pawns)
            if let Some(p) = piece {
                if p != Piece::Pawn {
                    san.push(p.to_char().to_ascii_uppercase());
                }
            }

            // Add disambiguation if needed
            if let Some(p) = piece {
                if p != Piece::Pawn {
                    let (needs_file, needs_rank) = self.needs_disambiguation(*mv, p);
                    if needs_file {
                        san.push((b'a' + mv.from().file() as u8) as char);
                    }
                    if needs_rank {
                        san.push((b'1' + mv.from().rank() as u8) as char);
                    }
                } else if mv.is_capture() {
                    // Pawn captures include the file
                    san.push((b'a' + mv.from().file() as u8) as char);
                }
            }

            // Add capture indicator
            if mv.is_capture() {
                san.push('x');
            }

            // Add destination square
            san.push_str(&mv.to().to_string());

            // Add promotion
            if let Some(promo) = mv.promotion() {
                san.push('=');
                san.push(promo.to_char().to_ascii_uppercase());
            }
        }

        // Check for check/checkmate by making the move temporarily
        let mut test_board = self.clone();
        test_board.make_move(*mv);

        if test_board.is_checkmate() {
            san.push('#');
        } else if test_board.is_in_check(test_board.side_to_move()) {
            san.push('+');
        }

        san
    }

    /// Determine if disambiguation is needed for a piece move.
    /// Returns (`needs_file`, `needs_rank`).
    fn needs_disambiguation(&self, mv: Move, piece: Piece) -> (bool, bool) {
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

        // Check if file alone disambiguates
        let same_file = same_dest_moves
            .iter()
            .any(|m| m.from().file() == mv.from().file());
        let same_rank = same_dest_moves
            .iter()
            .any(|m| m.from().rank() == mv.from().rank());

        match (same_file, same_rank) {
            (false, _) => (true, false),    // File disambiguates
            (true, false) => (false, true), // Rank disambiguates
            (true, true) => (true, true),   // Need both
        }
    }

    /// Parse a move in Standard Algebraic Notation.
    ///
    /// Accepts notation like "e4", "Nf3", "Bxc6", "O-O", "e8=Q"
    /// with optional check indicators (+, #).
    pub fn parse_san(&mut self, san: &str) -> Result<Move, SanError> {
        let san = san.trim();
        if san.is_empty() {
            return Err(SanError::Empty);
        }

        // Remove check/checkmate indicators
        let san = san.trim_end_matches(['+', '#']);

        // Handle castling
        if san == "O-O" || san == "0-0" {
            return self.find_castling_move(true);
        }
        if san == "O-O-O" || san == "0-0-0" {
            return self.find_castling_move(false);
        }

        let chars: Vec<char> = san.chars().collect();
        if chars.is_empty() {
            return Err(SanError::Empty);
        }

        // Parse the SAN components
        let (piece, rest) = if chars[0].is_ascii_uppercase() {
            let p = Piece::from_char(chars[0]).ok_or(SanError::InvalidPiece { char: chars[0] })?;
            (p, &chars[1..])
        } else {
            (Piece::Pawn, &chars[..])
        };

        // Parse remaining: [file][rank][x]<dest>[=promotion]
        let (disambig_file, disambig_rank, _is_capture, dest_str, promotion) =
            Board::parse_san_move_str(rest)?;

        // Parse destination square
        if dest_str.len() != 2 {
            return Err(SanError::InvalidSquare {
                notation: dest_str.iter().collect(),
            });
        }
        let dest_file = dest_str[0] as usize - 'a' as usize;
        let dest_rank = dest_str[1] as usize - '1' as usize;
        if dest_file >= 8 || dest_rank >= 8 {
            return Err(SanError::InvalidSquare {
                notation: dest_str.iter().collect(),
            });
        }
        let dest = Square::new(dest_rank, dest_file);

        // Find matching move
        self.find_san_move(piece, dest, disambig_file, disambig_rank, promotion, san)
    }

    /// Parse SAN components after the piece letter.
    /// Returns (`disambig_file`, `disambig_rank`, `is_capture`, `dest_chars`, promotion)
    fn parse_san_move_str(chars: &[char]) -> Result<SanParseResult, SanError> {
        let mut idx = 0;
        let mut disambig_file = None;
        let mut disambig_rank = None;
        let mut is_capture = false;
        let mut dest = Vec::new();
        let mut promotion = None;

        // Check for disambiguation and capture marker
        // Patterns: "e4", "xe4", "1e4", "exe4", "R1d2", "Raxd2"
        while idx < chars.len() {
            let c = chars[idx];

            if c == 'x' {
                is_capture = true;
                idx += 1;
            } else if c == '=' {
                // Promotion
                idx += 1;
                if idx < chars.len() {
                    let promo_char = chars[idx];
                    promotion = Some(
                        Piece::from_char(promo_char)
                            .ok_or(SanError::InvalidPromotion { char: promo_char })?,
                    );
                    idx += 1;
                }
            } else if c.is_ascii_lowercase() && idx + 1 < chars.len() {
                // Could be disambiguation file or destination file
                let next = chars[idx + 1];
                if next.is_ascii_digit() {
                    // This is the destination
                    dest.push(c);
                    dest.push(next);
                    idx += 2;
                } else if next == 'x' || next.is_ascii_lowercase() {
                    // This is disambiguation file
                    disambig_file = Some(c as usize - 'a' as usize);
                    idx += 1;
                } else {
                    dest.push(c);
                    idx += 1;
                }
            } else if c.is_ascii_digit() && dest.is_empty() {
                // Disambiguation rank
                disambig_rank = Some(c as usize - '1' as usize);
                idx += 1;
            } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
                dest.push(c);
                idx += 1;
            } else {
                idx += 1;
            }
        }

        Ok((disambig_file, disambig_rank, is_capture, dest, promotion))
    }

    /// Find the castling move.
    fn find_castling_move(&mut self, kingside: bool) -> Result<Move, SanError> {
        let moves = self.generate_moves();
        for mv in &moves {
            if mv.is_castling() {
                if kingside && mv.is_castle_kingside() {
                    return Ok(*mv);
                }
                if !kingside && mv.is_castle_queenside() {
                    return Ok(*mv);
                }
            }
        }
        Err(SanError::NoMatchingMove {
            san: if kingside { "O-O" } else { "O-O-O" }.to_string(),
        })
    }

    /// Find the move matching the parsed SAN components.
    fn find_san_move(
        &mut self,
        piece: Piece,
        dest: Square,
        disambig_file: Option<usize>,
        disambig_rank: Option<usize>,
        promotion: Option<Piece>,
        san: &str,
    ) -> Result<Move, SanError> {
        let moves = self.generate_moves();
        let mut matching: Vec<Move> = Vec::new();

        for mv in &moves {
            // Check destination
            if mv.to() != dest {
                continue;
            }

            // Check piece type
            if self.piece_on(mv.from()) != Some(piece) {
                continue;
            }

            // Check promotion
            if mv.promotion() != promotion {
                continue;
            }

            // Check disambiguation
            if let Some(file) = disambig_file {
                if mv.from().file() != file {
                    continue;
                }
            }
            if let Some(rank) = disambig_rank {
                if mv.from().rank() != rank {
                    continue;
                }
            }

            matching.push(*mv);
        }

        match matching.len() {
            0 => Err(SanError::NoMatchingMove {
                san: san.to_string(),
            }),
            1 => Ok(matching[0]),
            _ => Err(SanError::AmbiguousMove {
                san: san.to_string(),
            }),
        }
    }

    /// Parse a SAN move and make it on the board in one call.
    pub fn make_move_san(&mut self, san: &str) -> Result<Move, SanError> {
        let mv = self.parse_san(san)?;
        self.make_move(mv);
        Ok(mv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pawn_moves() {
        let mut board = Board::new();

        // e4
        let mv = board.parse_san("e4").unwrap();
        assert_eq!(mv.from(), Square::new(1, 4));
        assert_eq!(mv.to(), Square::new(3, 4));
        assert_eq!(board.move_to_san(&mv), "e4");
    }

    #[test]
    fn test_knight_moves() {
        let mut board = Board::new();

        // Nf3
        let mv = board.parse_san("Nf3").unwrap();
        assert_eq!(mv.from(), Square::new(0, 6));
        assert_eq!(mv.to(), Square::new(2, 5));
        assert_eq!(board.move_to_san(&mv), "Nf3");
    }

    #[test]
    fn test_castling() {
        let mut board = Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1");

        // O-O
        let mv = board.parse_san("O-O").unwrap();
        assert!(mv.is_castle_kingside());
        assert_eq!(board.move_to_san(&mv), "O-O");

        // O-O-O
        let mv = board.parse_san("O-O-O").unwrap();
        assert!(mv.is_castle_queenside());
        assert_eq!(board.move_to_san(&mv), "O-O-O");
    }

    #[test]
    fn test_captures() {
        let mut board =
            Board::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2");

        // exd5
        let mv = board.parse_san("exd5").unwrap();
        assert!(mv.is_capture());
        assert_eq!(board.move_to_san(&mv), "exd5");
    }

    #[test]
    fn test_promotion() {
        let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");

        // a8=Q
        let mv = board.parse_san("a8=Q").unwrap();
        assert_eq!(mv.promotion(), Some(Piece::Queen));
        assert_eq!(board.move_to_san(&mv), "a8=Q");
    }

    #[test]
    fn test_disambiguation() {
        // Position with two rooks that can go to d4
        let mut board = Board::from_fen("3k4/8/8/8/R6R/8/8/4K3 w - - 0 1");

        // Rad4 vs Rhd4
        let mv = board.parse_san("Rad4").unwrap();
        assert_eq!(mv.from().file(), 0); // a-file

        let mv = board.parse_san("Rhd4").unwrap();
        assert_eq!(mv.from().file(), 7); // h-file
    }

    #[test]
    fn test_check() {
        let mut board = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1");

        // Rh8+ gives check
        let mv = board.parse_san("Rh8").unwrap();
        let san = board.move_to_san(&mv);
        assert_eq!(san, "Rh8+");
    }

    #[test]
    fn test_checkmate() {
        // Fool's mate position
        let mut board =
            Board::from_fen("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2");

        // Qh4#
        let mv = board.parse_san("Qh4").unwrap();
        let san = board.move_to_san(&mv);
        assert_eq!(san, "Qh4#");
    }

    #[test]
    fn test_round_trip() {
        let mut board = Board::new();
        let moves = board.generate_moves();

        for mv in moves.iter() {
            let san = board.move_to_san(mv);
            let parsed = board.parse_san(&san).unwrap();
            assert_eq!(mv.from(), parsed.from());
            assert_eq!(mv.to(), parsed.to());
        }
    }
}
