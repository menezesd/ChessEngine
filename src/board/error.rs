//! Error types for chess board operations.

use std::fmt;

/// Error type for FEN parsing failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenError {
    /// FEN string has too few parts (needs at least 4)
    TooFewParts { found: usize },
    /// Invalid piece character in position string
    InvalidPiece { char: char },
    /// Invalid castling character
    InvalidCastling { char: char },
    /// Invalid side to move (must be 'w' or 'b')
    InvalidSideToMove { found: String },
    /// Invalid en passant square
    InvalidEnPassant { found: String },
    /// Invalid rank in position string
    InvalidRank { rank: usize },
    /// Too many files in a rank
    TooManyFiles { rank: usize, files: usize },
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FenError::TooFewParts { found } => {
                write!(f, "FEN must have at least 4 parts, found {found}")
            }
            FenError::InvalidPiece { char } => {
                write!(f, "Invalid piece character '{char}' in FEN")
            }
            FenError::InvalidCastling { char } => {
                write!(f, "Invalid castling character '{char}' in FEN")
            }
            FenError::InvalidSideToMove { found } => {
                write!(f, "Invalid side to move '{found}', expected 'w' or 'b'")
            }
            FenError::InvalidEnPassant { found } => {
                write!(f, "Invalid en passant square '{found}'")
            }
            FenError::InvalidRank { rank } => {
                write!(f, "Invalid rank index {rank} in FEN")
            }
            FenError::TooManyFiles { rank, files } => {
                write!(f, "Too many files ({files}) in rank {rank}")
            }
        }
    }
}

impl std::error::Error for FenError {}

/// Error type for move parsing failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveParseError {
    /// Move string has invalid length (must be 4-5 characters)
    InvalidLength { len: usize },
    /// Invalid square notation in move
    InvalidSquare { notation: String },
    /// Invalid promotion piece
    InvalidPromotion { char: char },
    /// Move is not legal in the current position
    IllegalMove { notation: String },
}

impl fmt::Display for MoveParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveParseError::InvalidLength { len } => {
                write!(f, "Move must be 4-5 characters, found {len}")
            }
            MoveParseError::InvalidSquare { notation } => {
                write!(f, "Invalid square notation in '{notation}'")
            }
            MoveParseError::InvalidPromotion { char } => {
                write!(f, "Invalid promotion piece '{char}'")
            }
            MoveParseError::IllegalMove { notation } => {
                write!(f, "Illegal move '{notation}'")
            }
        }
    }
}

impl std::error::Error for MoveParseError {}

/// Error type for square parsing failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SquareError {
    /// Rank out of bounds (must be 0-7)
    RankOutOfBounds { rank: usize },
    /// File out of bounds (must be 0-7)
    FileOutOfBounds { file: usize },
    /// Invalid algebraic notation
    InvalidNotation { notation: String },
}

impl fmt::Display for SquareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SquareError::RankOutOfBounds { rank } => {
                write!(f, "Rank {rank} out of bounds (must be 0-7)")
            }
            SquareError::FileOutOfBounds { file } => {
                write!(f, "File {file} out of bounds (must be 0-7)")
            }
            SquareError::InvalidNotation { notation } => {
                write!(f, "Invalid square notation '{notation}'")
            }
        }
    }
}

impl std::error::Error for SquareError {}

/// Error type for SAN (Standard Algebraic Notation) parsing failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanError {
    /// Empty SAN string
    Empty,
    /// Invalid piece character
    InvalidPiece { char: char },
    /// Invalid square in SAN
    InvalidSquare { notation: String },
    /// Ambiguous move (multiple pieces can reach the target)
    AmbiguousMove { san: String },
    /// No matching legal move found
    NoMatchingMove { san: String },
    /// Invalid promotion piece
    InvalidPromotion { char: char },
    /// Invalid castling notation
    InvalidCastling { notation: String },
}

impl fmt::Display for SanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SanError::Empty => write!(f, "Empty SAN string"),
            SanError::InvalidPiece { char } => {
                write!(f, "Invalid piece character '{char}' in SAN")
            }
            SanError::InvalidSquare { notation } => {
                write!(f, "Invalid square in SAN '{notation}'")
            }
            SanError::AmbiguousMove { san } => {
                write!(f, "Ambiguous move '{san}'")
            }
            SanError::NoMatchingMove { san } => {
                write!(f, "No legal move matches '{san}'")
            }
            SanError::InvalidPromotion { char } => {
                write!(f, "Invalid promotion piece '{char}'")
            }
            SanError::InvalidCastling { notation } => {
                write!(f, "Invalid castling notation '{notation}'")
            }
        }
    }
}

impl std::error::Error for SanError {}

#[cfg(test)]
mod tests {
    use super::*;

    // FenError tests
    #[test]
    fn test_fen_error_too_few_parts() {
        let err = FenError::TooFewParts { found: 2 };
        assert!(err.to_string().contains('2'));
        assert!(err.to_string().contains('4'));
    }

    #[test]
    fn test_fen_error_invalid_piece() {
        let err = FenError::InvalidPiece { char: 'z' };
        assert!(err.to_string().contains("'z'"));
    }

    #[test]
    fn test_fen_error_invalid_castling() {
        let err = FenError::InvalidCastling { char: 'x' };
        assert!(err.to_string().contains("'x'"));
    }

    #[test]
    fn test_fen_error_invalid_side() {
        let err = FenError::InvalidSideToMove {
            found: "X".to_string(),
        };
        assert!(err.to_string().contains("'X'"));
    }

    #[test]
    fn test_fen_error_equality() {
        let err1 = FenError::TooFewParts { found: 2 };
        let err2 = FenError::TooFewParts { found: 2 };
        assert_eq!(err1, err2);
    }

    // MoveParseError tests
    #[test]
    fn test_move_error_invalid_length() {
        let err = MoveParseError::InvalidLength { len: 3 };
        assert!(err.to_string().contains('3'));
    }

    #[test]
    fn test_move_error_invalid_square() {
        let err = MoveParseError::InvalidSquare {
            notation: "z9z9".to_string(),
        };
        assert!(err.to_string().contains("z9z9"));
    }

    #[test]
    fn test_move_error_illegal_move() {
        let err = MoveParseError::IllegalMove {
            notation: "e2e5".to_string(),
        };
        assert!(err.to_string().contains("e2e5"));
    }

    // SquareError tests
    #[test]
    fn test_square_error_rank_bounds() {
        let err = SquareError::RankOutOfBounds { rank: 9 };
        assert!(err.to_string().contains('9'));
    }

    #[test]
    fn test_square_error_file_bounds() {
        let err = SquareError::FileOutOfBounds { file: 10 };
        assert!(err.to_string().contains("10"));
    }

    #[test]
    fn test_square_error_invalid_notation() {
        let err = SquareError::InvalidNotation {
            notation: "xyz".to_string(),
        };
        assert!(err.to_string().contains("xyz"));
    }

    // SanError tests
    #[test]
    fn test_san_error_empty() {
        let err = SanError::Empty;
        assert!(err.to_string().contains("Empty"));
    }

    #[test]
    fn test_san_error_ambiguous() {
        let err = SanError::AmbiguousMove {
            san: "Nc3".to_string(),
        };
        assert!(err.to_string().contains("Nc3"));
    }

    #[test]
    fn test_san_error_no_match() {
        let err = SanError::NoMatchingMove {
            san: "Qh7".to_string(),
        };
        assert!(err.to_string().contains("Qh7"));
    }

    #[test]
    fn test_error_clone() {
        let err = FenError::InvalidPiece { char: 'x' };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }
}
