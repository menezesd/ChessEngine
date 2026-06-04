//! Universal Chess Interface (UCI) protocol implementation.
//!
//! Handles communication with chess GUIs following the UCI specification.

use std::fmt;

use crate::board::{Board, FenError, Move, MoveParseError};

pub mod command;
pub mod options;
pub mod print;
pub mod report;
pub mod session;
pub mod time;

pub use time::TimeControl;

/// Error type for UCI position command parsing
#[derive(Debug, Clone)]
pub enum UciError {
    /// Invalid FEN string
    InvalidFen(FenError),
    /// Invalid move in the move list
    InvalidMove {
        move_str: String,
        error: MoveParseError,
    },
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

const POSITION_FEN_FIELDS: usize = 6;

/// Parse a move in UCI format (e.g., "e2e4", "e7e8q").
///
/// Delegates to `Board::parse_move`. Returns `None` if the move is invalid.
#[must_use]
pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    board.parse_move(uci_string).ok()
}

fn parse_position_source(parts: &[&str]) -> Result<(Board, usize), UciError> {
    let Some(kind) = parts.get(1) else {
        return Err(UciError::MissingParts);
    };

    match *kind {
        "startpos" => Ok((Board::new(), 2)),
        "fen" => {
            let fen_start = 2;
            let fen_end = fen_start + POSITION_FEN_FIELDS;
            if parts.len() < fen_end {
                return Err(UciError::MissingParts);
            }
            let fen = parts[fen_start..fen_end].join(" ");
            Ok((Board::try_from_fen(&fen)?, fen_end))
        }
        _ => Err(UciError::MissingParts),
    }
}

fn apply_position_moves(board: &mut Board, moves: &[&str]) -> Result<(), UciError> {
    for move_str in moves {
        let mv = board
            .parse_move(move_str)
            .map_err(|e| UciError::InvalidMove {
                move_str: (*move_str).to_string(),
                error: e,
            })?;
        board.make_move(mv);
    }
    Ok(())
}

/// Parse a UCI position command, returning an error on failure.
///
/// Supports both "position startpos" and "position fen <fen>" formats,
/// optionally followed by "moves <move1> <move2> ...".
pub fn try_parse_position_command(board: &mut Board, parts: &[&str]) -> Result<(), UciError> {
    let (mut parsed_board, mut i) = parse_position_source(parts)?;

    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        apply_position_moves(&mut parsed_board, &parts[i..])?;
    }

    *board = parsed_board;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_parse_position_startpos_moves() {
        let mut board = Board::empty();
        try_parse_position_command(&mut board, &["position", "startpos", "moves", "e2e4"]).unwrap();

        assert!(!board.white_to_move());
    }

    #[test]
    fn try_parse_position_fen() {
        let mut board = Board::new();
        try_parse_position_command(
            &mut board,
            &[
                "position",
                "fen",
                "8/8/8/8/8/8/8/K1k5",
                "b",
                "-",
                "-",
                "0",
                "1",
            ],
        )
        .unwrap();

        assert!(!board.white_to_move());
    }

    #[test]
    fn try_parse_position_rejects_missing_source() {
        let mut board = Board::new();
        let result = try_parse_position_command(&mut board, &["position"]);
        assert!(matches!(result, Err(UciError::MissingParts)));
    }

    #[test]
    fn try_parse_position_rejects_invalid_move() {
        let mut board = Board::new();
        let result =
            try_parse_position_command(&mut board, &["position", "startpos", "moves", "e2e5"]);
        assert!(matches!(result, Err(UciError::InvalidMove { .. })));
    }

    #[test]
    fn try_parse_position_leaves_board_unchanged_after_invalid_later_move() {
        let mut board = Board::new();
        let original_hash = board.hash;
        let original_side = board.side_to_move();

        let result = try_parse_position_command(
            &mut board,
            &["position", "startpos", "moves", "e2e4", "e2e5"],
        );

        assert!(matches!(result, Err(UciError::InvalidMove { .. })));
        assert_eq!(board.hash, original_hash);
        assert_eq!(board.side_to_move(), original_side);
    }
}
