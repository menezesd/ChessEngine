//! Universal Chess Interface (UCI) protocol implementation.
//!
//! Handles communication with chess GUIs following the UCI specification.

use std::fmt;

use crate::board::{Board, FenError, Move, MoveParseError};

pub mod command;
pub mod options;
pub mod print;
pub mod report;
pub mod time;

/// Error type for UCI position command parsing
#[derive(Debug, Clone)]
pub enum UciError {
    /// Invalid FEN string
    InvalidFen(FenError),
    /// Invalid move in the move list
    InvalidMove { move_str: String, error: MoveParseError },
    /// Missing required parts in the command
    MissingParts,
}

impl fmt::Display for UciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UciError::InvalidFen(e) => write!(f, "Invalid FEN: {e}"),
            UciError::InvalidMove { move_str, error } => {
                write!(f, "Invalid move '{move_str}': {error}")
            }
            UciError::MissingParts => write!(f, "Missing required parts in position command"),
        }
    }
}

impl std::error::Error for UciError {}

impl From<FenError> for UciError {
    fn from(e: FenError) -> Self {
        UciError::InvalidFen(e)
    }
}

/// Parse a move in UCI format (e.g., "e2e4", "e7e8q").
///
/// Delegates to `Board::parse_move`. Returns `None` if the move is invalid.
#[must_use]
pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    board.parse_move(uci_string).ok()
}

/// Parse a UCI position command, returning an error on failure.
///
/// Supports both "position startpos" and "position fen <fen>" formats,
/// optionally followed by "moves <move1> <move2> ...".
pub fn try_parse_position_command(board: &mut Board, parts: &[&str]) -> Result<(), UciError> {
    let mut i = 1;

    if i >= parts.len() {
        return Err(UciError::MissingParts);
    }

    if parts[i] == "startpos" {
        *board = Board::new();
        i += 1;
    } else if parts[i] == "fen" {
        if i + 6 >= parts.len() {
            return Err(UciError::MissingParts);
        }
        let fen = parts[i + 1..i + 7].join(" ");
        *board = Board::try_from_fen(&fen)?;
        i += 7;
    } else {
        return Err(UciError::MissingParts);
    }

    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        while i < parts.len() {
            let mv = board.parse_move(parts[i]).map_err(|e| UciError::InvalidMove {
                move_str: parts[i].to_string(),
                error: e,
            })?;
            board.make_move(mv);
            i += 1;
        }
    }

    Ok(())
}

/// Parse a UCI position command, printing errors to stderr on failure.
///
/// This is a convenience wrapper around `try_parse_position_command` for
/// use in the main UCI loop where errors should be logged but not propagated.
pub fn parse_position_command(board: &mut Board, parts: &[&str]) {
    if let Err(e) = try_parse_position_command(board, parts) {
        eprintln!("Error: {e}");
    }
}

#[must_use] 
pub fn format_uci_move(mv: &Move) -> String {
    mv.to_string()
}
