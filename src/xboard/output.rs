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

use crate::board::{Board, Move, SearchIterationInfo};

const MATE_SCORE_BASE: i32 = 100_000;

/// Convert a completed search's coordinate PV into CECP thinking output.
pub(super) fn format_search_info(
    board: &Board,
    info: &SearchIterationInfo,
    ponder_move: Option<Move>,
) -> String {
    let mut position = board.clone();
    let mut pv = Vec::new();
    if let Some(mv) = ponder_move {
        pv.push(position.move_to_san(&mv));
        position.make_move(mv);
    }
    for text in info.pv.split_whitespace() {
        let Ok(mv) = position.parse_move(text) else {
            break;
        };
        pv.push(position.move_to_san(&mv));
        position.make_move(mv);
    }
    let score = info.mate_in.map_or(info.score, |moves| {
        info.score.signum() * (MATE_SCORE_BASE + moves.abs())
    });
    thinking_line(
        info.depth,
        score,
        info.time_ms / 10,
        info.nodes,
        &pv.join(" "),
    )
}

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
    thinking_line(depth, score, time_cs, nodes, &format_pv_san(board, pv))
}

fn thinking_line(depth: u32, score: i32, time_cs: u64, nodes: u64, pv: &str) -> String {
    format!("{depth} {score} {time_cs} {nodes} {pv}")
}

/// Format a PV as SAN notation.
fn format_pv_san(board: &Board, pv: &[Move]) -> String {
    let mut result = Vec::new();
    let mut temp_board = board.clone();

    for mv in pv {
        let san = temp_board.move_to_san(mv);
        result.push(san);
        temp_board.make_move(*mv);
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
        "feature analyze=1",
        "feature colors=0",
        "feature ics=0",
        "feature name=1",
        "feature pause=1",
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
mod tests;
