use std::io::Write;
use std::process::{Command, Stdio};

use chess_engine::board::Board;
use chess_engine::uci::{parse_position_command, parse_uci_move};

#[test]
fn uci_smoke_test_returns_legal_move() {
    let exe = env!("CARGO_BIN_EXE_chess_engine");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine binary");

    let input = b"uci\nisready\nposition startpos moves e2e4\ngo movetime 50\nquit\n";
    child.stdin.as_mut().unwrap().write_all(input).unwrap();

    let output = child.wait_with_output().expect("failed to read output");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("uciok"));
    assert!(stdout.contains("readyok"));

    let bestmove = stdout
        .lines()
        .filter(|line| line.starts_with("bestmove"))
        .last()
        .expect("no bestmove found");
    let parts: Vec<&str> = bestmove.split_whitespace().collect();
    assert!(parts.len() >= 2, "bestmove missing move: {}", bestmove);
    let mv = parts[1];
    assert_ne!(mv, "0000", "engine returned null move");

    let mut board = Board::new();
    let parts = ["position", "startpos", "moves", "e2e4"];
    parse_position_command(&mut board, &parts);

    let legal = parse_uci_move(&mut board, mv).is_some();
    assert!(legal, "bestmove not legal in position: {}", mv);
}
