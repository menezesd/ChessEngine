//! Fluent builder for constructing chess positions.
//!
//! Allows creating positions piece by piece rather than parsing FEN strings.
//!
//! # Example
//! ```
//! use chess_engine::board::{BoardBuilder, Color, Piece, Square};
//!
//! let board = BoardBuilder::new()
//!     .piece(Square(0, 4), Color::White, Piece::King)
//!     .piece(Square(7, 4), Color::Black, Piece::King)
//!     .piece(Square(1, 0), Color::White, Piece::Pawn)
//!     .side_to_move(Color::White)
//!     .build();
//! ```

use super::{Board, CastlingRights, Color, Piece, Square};

/// A fluent builder for constructing `Board` positions.
#[derive(Clone, Debug)]
pub struct BoardBuilder {
    pieces: Vec<(Square, Color, Piece)>,
    side_to_move: Color,
    castling_rights: u8,
    en_passant_target: Option<Square>,
    halfmove_clock: u32,
}

impl Default for BoardBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardBuilder {
    /// Create a new empty board builder.
    #[must_use]
    pub fn new() -> Self {
        BoardBuilder {
            pieces: Vec::new(),
            side_to_move: Color::White,
            castling_rights: 0,
            en_passant_target: None,
            halfmove_clock: 0,
        }
    }

    /// Create a builder starting from the standard initial position.
    #[must_use]
    pub fn starting_position() -> Self {
        let mut builder = Self::new();

        // White pieces
        let back_rank = [
            Piece::Rook,
            Piece::Knight,
            Piece::Bishop,
            Piece::Queen,
            Piece::King,
            Piece::Bishop,
            Piece::Knight,
            Piece::Rook,
        ];
        for (file, &piece) in back_rank.iter().enumerate() {
            builder.pieces.push((Square(0, file), Color::White, piece));
            builder.pieces.push((Square(7, file), Color::Black, piece));
        }
        for file in 0..8 {
            builder
                .pieces
                .push((Square(1, file), Color::White, Piece::Pawn));
            builder
                .pieces
                .push((Square(6, file), Color::Black, Piece::Pawn));
        }

        builder.castling_rights = CastlingRights::all().as_u8();
        builder
    }

    /// Place a piece on the board.
    #[must_use]
    pub fn piece(mut self, square: Square, color: Color, piece: Piece) -> Self {
        // Remove any existing piece on this square
        self.pieces.retain(|(sq, _, _)| *sq != square);
        self.pieces.push((square, color, piece));
        self
    }

    /// Remove a piece from a square.
    #[must_use]
    pub fn clear(mut self, square: Square) -> Self {
        self.pieces.retain(|(sq, _, _)| *sq != square);
        self
    }

    /// Set the side to move.
    #[must_use]
    pub const fn side_to_move(mut self, color: Color) -> Self {
        self.side_to_move = color;
        self
    }

    /// Set castling rights from a `CastlingRights` value.
    #[must_use]
    pub const fn castling(mut self, rights: CastlingRights) -> Self {
        self.castling_rights = rights.as_u8();
        self
    }

    /// Enable kingside castling for a color.
    #[must_use]
    pub fn castle_kingside(mut self, color: Color) -> Self {
        let mut rights = CastlingRights::from_u8(self.castling_rights);
        rights.set(color, true);
        self.castling_rights = rights.as_u8();
        self
    }

    /// Enable queenside castling for a color.
    #[must_use]
    pub fn castle_queenside(mut self, color: Color) -> Self {
        let mut rights = CastlingRights::from_u8(self.castling_rights);
        rights.set(color, false);
        self.castling_rights = rights.as_u8();
        self
    }

    /// Enable all castling rights.
    #[must_use]
    pub const fn all_castling_rights(mut self) -> Self {
        self.castling_rights = CastlingRights::all().as_u8();
        self
    }

    /// Disable all castling rights.
    #[must_use]
    pub const fn no_castling_rights(mut self) -> Self {
        self.castling_rights = 0;
        self
    }

    /// Set the en passant target square.
    #[must_use]
    pub const fn en_passant(mut self, target: Square) -> Self {
        self.en_passant_target = Some(target);
        self
    }

    /// Clear the en passant target.
    #[must_use]
    pub const fn clear_en_passant(mut self) -> Self {
        self.en_passant_target = None;
        self
    }

    /// Set the halfmove clock (for 50-move rule).
    #[must_use]
    pub const fn halfmove_clock(mut self, clock: u32) -> Self {
        self.halfmove_clock = clock;
        self
    }

    /// Build the board.
    ///
    /// Creates a Board with all the specified pieces and settings.
    #[must_use]
    pub fn build(self) -> Board {
        let mut board = Board::empty();

        for (square, color, piece) in self.pieces {
            board.set_piece(square, color, piece);
        }

        board.white_to_move = self.side_to_move == Color::White;
        board.castling_rights = self.castling_rights;
        board.en_passant_target = self.en_passant_target;
        board.halfmove_clock = self.halfmove_clock;
        board.hash = board.calculate_initial_hash();
        board.repetition_counts.set(board.hash, 1);
        board.recalculate_incremental_eval();

        board
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position() {
        let built = BoardBuilder::starting_position().build();
        let standard = Board::new();

        // Compare FEN representations
        assert_eq!(built.to_fen(), standard.to_fen());
    }

    #[test]
    fn test_empty_board() {
        let board = BoardBuilder::new()
            .piece(Square(0, 4), Color::White, Piece::King)
            .piece(Square(7, 4), Color::Black, Piece::King)
            .build();

        // Should have only two kings
        assert!(board.piece_at(Square(0, 4)).is_some());
        assert!(board.piece_at(Square(7, 4)).is_some());
        assert!(board.piece_at(Square(0, 0)).is_none());
    }

    #[test]
    fn test_castling_rights() {
        let board = BoardBuilder::starting_position()
            .no_castling_rights()
            .castle_kingside(Color::White)
            .build();

        let rights = CastlingRights::from_u8(board.castling_rights);
        assert!(rights.has(Color::White, true)); // Kingside
        assert!(!rights.has(Color::White, false)); // Queenside
        assert!(!rights.has(Color::Black, true));
        assert!(!rights.has(Color::Black, false));
    }

    #[test]
    fn test_side_to_move() {
        let board = BoardBuilder::new()
            .piece(Square(0, 4), Color::White, Piece::King)
            .piece(Square(7, 4), Color::Black, Piece::King)
            .side_to_move(Color::Black)
            .build();

        assert!(!board.white_to_move());
    }

    #[test]
    fn test_clear_square() {
        let board = BoardBuilder::starting_position()
            .clear(Square(0, 0)) // Remove white rook on a1
            .build();

        assert!(board.piece_at(Square(0, 0)).is_none());
        assert!(board.piece_at(Square(0, 1)).is_some()); // Knight still there
    }
}
