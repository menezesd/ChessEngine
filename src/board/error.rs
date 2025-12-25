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
