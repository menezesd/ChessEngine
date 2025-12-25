//! `XBoard` protocol output formatting.
//!
//! `XBoard` thinking output format:
//! `<ply> <score> <time> <nodes> <pv>`
//!
//! Where:
//! - ply: search depth
//! - score: score in centipawns (positive = good for engine)
//! - time: time in centiseconds
//! - nodes: nodes searched
//! - pv: principal variation in SAN

use crate::board::{Board, Move};

/// Format a principal variation line for `XBoard` output.
///
/// `XBoard` format: `<ply> <score> <time> <nodes> <pv>`
#[must_use] 
pub fn format_thinking(
    board: &Board,
    depth: u32,
    score: i32,
    time_cs: u64,
    nodes: u64,
    pv: &[Move],
) -> String {
    let pv_str = format_pv_san(board, pv);
    format!("{depth} {score} {time_cs} {nodes} {pv_str}")
}

/// Format a PV as SAN notation.
fn format_pv_san(board: &Board, pv: &[Move]) -> String {
    let mut result = Vec::new();
    let mut temp_board = board.clone();

    for mv in pv {
        let san = temp_board.move_to_san(mv);
        result.push(san);
        temp_board.make_move(mv);
    }

    result.join(" ")
}

/// Format a move announcement for `XBoard`.
#[must_use] 
pub fn format_move(board: &Board, mv: &Move) -> String {
    let san = board.move_to_san(mv);
    format!("move {san}")
}

/// Format feature announcement after protover.
#[must_use]
pub fn format_features() -> String {
    let features = [
        "feature myname=\"ChessEngine 0.1\"",
        "feature setboard=1",
        "feature ping=1",
        "feature san=1",
        "feature usermove=1",
        "feature time=1",
        "feature draw=1",
        "feature sigint=0",
        "feature sigterm=0",
        "feature reuse=1",
        "feature analyze=0",
        "feature colors=0",
        "feature ics=0",
        "feature name=1",
        "feature pause=0",
        "feature nps=0",
        "feature debug=0",
        "feature memory=1",
        "feature smp=0",
        "feature done=1",
    ];
    features.join("\n")
}

/// Format an error message for `XBoard`.
#[must_use] 
pub fn format_error(command: &str, message: &str) -> String {
    format!("Error ({message}): {command}")
}

/// Format illegal move error.
#[must_use] 
pub fn format_illegal_move(mv: &str) -> String {
    format!("Illegal move: {mv}")
}

/// Format a pong response.
#[must_use] 
pub fn format_pong(n: u32) -> String {
    format!("pong {n}")
}

/// Format a hint.
#[must_use] 
pub fn format_hint(board: &Board, mv: &Move) -> String {
    let san = board.move_to_san(mv);
    format!("Hint: {san}")
}

/// Format a result message.
#[must_use]
pub fn format_result(result: &str, reason: &str) -> String {
    format!("{result} {{{reason}}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn test_format_features() {
        let features = format_features();
        assert!(features.contains("myname"));
        assert!(features.contains("setboard=1"));
        assert!(features.contains("done=1"));
    }

    #[test]
    fn test_format_thinking() {
        let board = Board::new();
        let output = format_thinking(&board, 5, 30, 142, 12345, &[]);
        assert_eq!(output, "5 30 142 12345 ");
    }

    #[test]
    fn test_format_error() {
        let err = format_error("badcmd", "unknown command");
        assert_eq!(err, "Error (unknown command): badcmd");
    }

    #[test]
    fn test_format_pong() {
        assert_eq!(format_pong(42), "pong 42");
    }
}
