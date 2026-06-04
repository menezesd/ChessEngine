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
