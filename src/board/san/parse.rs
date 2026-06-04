use crate::board::error::SanError;
use crate::board::{Board, Move, Piece, Square};

struct SanMoveParts {
    disambig_file: Option<usize>,
    disambig_rank: Option<usize>,
    dest_notation: String,
    promotion: Option<Piece>,
}

impl Board {
    /// Parse a move in Standard Algebraic Notation.
    ///
    /// Accepts notation like "e4", "Nf3", "Bxc6", "O-O", "e8=Q"
    /// with optional check indicators (+, #).
    pub fn parse_san(&mut self, san: &str) -> Result<Move, SanError> {
        let san = san.trim();
        if san.is_empty() {
            return Err(SanError::Empty);
        }

        let san = san.trim_end_matches(['+', '#']);

        if san == "O-O" || san == "0-0" {
            return self.find_castling_move(true);
        }
        if san == "O-O-O" || san == "0-0-0" {
            return self.find_castling_move(false);
        }

        let first = san.chars().next().ok_or(SanError::Empty)?;
        let (piece, rest) = if first.is_ascii_uppercase() {
            let p = Piece::from_char(first).ok_or(SanError::InvalidPiece { char: first })?;
            (p, &san[first.len_utf8()..])
        } else {
            (Piece::Pawn, san)
        };

        let parts = Board::parse_san_move_str(rest)?;

        if parts.dest_notation.len() != 2 {
            return Err(SanError::InvalidSquare {
                notation: parts.dest_notation,
            });
        }
        let dest = parts
            .dest_notation
            .parse::<Square>()
            .map_err(|_| SanError::InvalidSquare {
                notation: parts.dest_notation.clone(),
            })?;

        self.find_san_move(
            piece,
            dest,
            parts.disambig_file,
            parts.disambig_rank,
            parts.promotion,
            san,
        )
    }

    /// Parse SAN components after the piece letter.
    fn parse_san_move_str(san: &str) -> Result<SanMoveParts, SanError> {
        let mut chars = san.chars().peekable();
        let mut disambig_file = None;
        let mut disambig_rank = None;
        let mut dest = String::new();
        let mut promotion = None;

        while let Some(c) = chars.next() {
            if c == 'x' {
            } else if c == '=' {
                if let Some(promo_char) = chars.next() {
                    promotion = Some(
                        Piece::from_char(promo_char)
                            .ok_or(SanError::InvalidPromotion { char: promo_char })?,
                    );
                }
            } else if c.is_ascii_lowercase() {
                if let Some(next) = chars.peek().copied() {
                    if next.is_ascii_digit() {
                        dest.push(c);
                        dest.push(next);
                        chars.next();
                    } else if next == 'x' || next.is_ascii_lowercase() {
                        disambig_file = Some(c as usize - 'a' as usize);
                    } else {
                        dest.push(c);
                    }
                } else {
                    dest.push(c);
                }
            } else if c.is_ascii_digit() && dest.is_empty() {
                disambig_rank = Some(c as usize - '1' as usize);
            } else if c.is_ascii_digit() {
                dest.push(c);
            }
        }

        Ok(SanMoveParts {
            disambig_file,
            disambig_rank,
            dest_notation: dest,
            promotion,
        })
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
            if mv.to() != dest {
                continue;
            }

            if self.piece_on(mv.from()) != Some(piece) {
                continue;
            }

            if mv.promotion() != promotion {
                continue;
            }

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
