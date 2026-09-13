use crate::board::error::SanError;
use crate::board::{Board, Move, Piece, Square};

struct SanMoveParts {
    disambig_file: Option<usize>,
    disambig_rank: Option<usize>,
    dest_notation: String,
    promotion: Option<Piece>,
    capture_marker: bool,
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

        let san = san.trim_end_matches(['+', '#', '!', '?']);

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

        self.find_san_move(piece, dest, &parts, san)
    }

    /// Parse SAN components after the piece letter.
    fn parse_san_move_str(san: &str) -> Result<SanMoveParts, SanError> {
        let invalid_square = || SanError::InvalidSquare {
            notation: san.to_string(),
        };
        let (move_text, promotion) = if let Some((move_text, suffix)) = san.split_once('=') {
            let mut chars = suffix.chars();
            let promo_char = chars.next().ok_or_else(invalid_square)?;
            let piece = Piece::from_char(promo_char)
                .filter(|piece| {
                    matches!(
                        piece,
                        Piece::Knight | Piece::Bishop | Piece::Rook | Piece::Queen
                    )
                })
                .ok_or(SanError::InvalidPromotion { char: promo_char })?;
            if chars.next().is_some() {
                return Err(SanError::InvalidPromotion { char: promo_char });
            }
            (move_text, Some(piece))
        } else {
            (san, None)
        };

        if !move_text.is_ascii() || move_text.len() < 2 {
            return Err(invalid_square());
        }
        // The destination is always the final square. Earlier file/rank
        // characters identify the origin, including full disambiguation
        // such as Qb1c2 when neither the file nor the rank alone is enough.
        let (prefix, dest) = move_text.split_at(move_text.len() - 2);
        // Crafty-style extended algebraic also permits e2-e4 / Ng1-f3.
        let prefix = prefix.strip_suffix('-').unwrap_or(prefix);
        let (origin, capture_marker) = prefix
            .strip_suffix('x')
            .map_or((prefix, false), |origin| (origin, true));

        let (disambig_file, disambig_rank) = match origin.as_bytes() {
            [] => (None, None),
            [file @ b'a'..=b'h'] => (Some(usize::from(file - b'a')), None),
            [rank @ b'1'..=b'8'] => (None, Some(usize::from(rank - b'1'))),
            [file @ b'a'..=b'h', rank @ b'1'..=b'8'] => (
                Some(usize::from(file - b'a')),
                Some(usize::from(rank - b'1')),
            ),
            _ => return Err(invalid_square()),
        };

        Ok(SanMoveParts {
            disambig_file,
            disambig_rank,
            dest_notation: dest.to_string(),
            promotion,
            capture_marker,
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
        parts: &SanMoveParts,
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

            if mv.promotion() != parts.promotion {
                continue;
            }

            // A written 'x' must describe a capture.  Its omission is
            // tolerated, matching Crafty's relaxed algebraic parser (for
            // example exd5, ed5, and xd5 can name the same unique move).
            if parts.capture_marker && !mv.is_capture() {
                continue;
            }

            if let Some(file) = parts.disambig_file {
                if mv.from().file() != file {
                    continue;
                }
            }
            if let Some(rank) = parts.disambig_rank {
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
