use crate::core::board::Board;
use crate::core::types::{Color, Piece, Square};
use crate::core::zobrist::{color_to_zobrist_index, piece_to_zobrist_index};

/// Error type for FEN parsing failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenError {
    /// FEN string doesn't have enough parts (needs at least 4)
    TooFewParts,
    /// Invalid digit in piece placement section
    InvalidDigit,
    /// Invalid piece character in FEN
    InvalidPieceChar(char),
    /// Invalid color indicator (must be 'w' or 'b')
    InvalidColor(String),
    /// Invalid castling rights character
    InvalidCastlingChar(char),
    /// Invalid en-passant square format
    InvalidEnPassantSquare,
}

/// Piece-to-character mapping arrays for FEN and display
const PIECE_CHARS: [[char; 6]; 2] = [
    ['P', 'N', 'B', 'R', 'Q', 'K'], // White pieces
    ['p', 'n', 'b', 'r', 'q', 'k'], // Black pieces
];

/// Character-to-piece mapping for FEN parsing
const CHAR_TO_PIECE: [Option<(Color, Piece)>; 256] = {
    let mut arr = [None; 256];
    arr['P' as usize] = Some((Color::White, Piece::Pawn));
    arr['N' as usize] = Some((Color::White, Piece::Knight));
    arr['B' as usize] = Some((Color::White, Piece::Bishop));
    arr['R' as usize] = Some((Color::White, Piece::Rook));
    arr['Q' as usize] = Some((Color::White, Piece::Queen));
    arr['K' as usize] = Some((Color::White, Piece::King));
    arr['p' as usize] = Some((Color::Black, Piece::Pawn));
    arr['n' as usize] = Some((Color::Black, Piece::Knight));
    arr['b' as usize] = Some((Color::Black, Piece::Bishop));
    arr['r' as usize] = Some((Color::Black, Piece::Rook));
    arr['q' as usize] = Some((Color::Black, Piece::Queen));
    arr['k' as usize] = Some((Color::Black, Piece::King));
    arr
};

impl Board {
    /// Fallible FEN parser. Returns Ok(Board) or Err(FenError) with a specific error.
    pub fn try_from_fen(fen: &str) -> Result<Self, FenError> {
        let mut board = Board::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(FenError::TooFewParts);
        }
        // Piece placement
        Self::parse_piece_placement(&mut board, parts[0])?;
        Self::parse_active_color(&mut board, parts[1])?;
        Self::parse_castling_rights(&mut board, parts[2])?;
        Self::parse_en_passant(&mut board, parts[3])?;

        board.hash = board.calculate_initial_hash(); // Calculate hash after setting up board
        board.halfmove_clock = 0;
        board.position_history.clear();
        board.position_history.push(board.hash);
        Ok(board)
    }

    fn parse_piece_placement(board: &mut Board, placement: &str) -> Result<(), FenError> {
        for (rank_idx, rank_str) in placement.split('/').enumerate() {
            let mut file = 0usize;
            for c in rank_str.chars() {
                if c.is_ascii_digit() {
                    file += c.to_digit(10).ok_or(FenError::InvalidDigit)? as usize;
                } else {
                    let (color, piece) = CHAR_TO_PIECE[c as usize].ok_or(FenError::InvalidPieceChar(c))?;
                    board.place_piece_at(Square(7 - rank_idx, file), (color, piece));
                    file += 1;
                }
            }
        }
        Ok(())
    }

    fn parse_active_color(board: &mut Board, color_part: &str) -> Result<(), FenError> {
        board.white_to_move = match color_part {
            "w" => true,
            "b" => false,
            other => return Err(FenError::InvalidColor(other.to_string())),
        };
        Ok(())
    }

    fn parse_castling_rights(board: &mut Board, castling_part: &str) -> Result<(), FenError> {
        for c in castling_part.chars() {
            match c {
                'K' => board.add_castling_right(Color::White, 'K'),
                'Q' => board.add_castling_right(Color::White, 'Q'),
                'k' => board.add_castling_right(Color::Black, 'K'),
                'q' => board.add_castling_right(Color::Black, 'Q'),
                '-' => (),
                other => return Err(FenError::InvalidCastlingChar(other)),
            }
        }
        Ok(())
    }

    fn parse_en_passant(board: &mut Board, en_passant_part: &str) -> Result<(), FenError> {
        board.en_passant_target = if en_passant_part != "-" {
            let chars: Vec<char> = en_passant_part.chars().collect();
            if chars.len() == 2 {
                Some(Square(crate::core::types::rank_to_index(chars[1]), crate::core::types::file_to_index(chars[0])))
            } else {
                return Err(FenError::InvalidEnPassantSquare);
            }
        } else {
            None
        };
        Ok(())
    }

    /// Convert a piece and color to its FEN/display character
    pub const fn piece_to_char(color: Color, piece: Piece) -> char {
        let color_idx = color_to_zobrist_index(color);
        let piece_idx = piece_to_zobrist_index(piece);
        PIECE_CHARS[color_idx][piece_idx]
    }
}