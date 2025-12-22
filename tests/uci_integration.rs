use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(b"uci\nisready\nposition startpos moves e2e4\ngo movetime 50\n")
        .unwrap();

    let mut output = String::new();
    let mut bestmove_line = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).expect("read failed");
        if bytes == 0 {
            break;
        }
        output.push_str(&line);
        if line.starts_with("bestmove") {
            bestmove_line = Some(line);
            break;
        }
    }

    stdin.write_all(b"quit\n").unwrap();
    let _ = child.wait();

    assert!(output.contains("uciok"));
    assert!(output.contains("readyok"));
    assert!(output.contains("info string time"));
    assert!(output.contains("hashfull"));

    let bestmove = bestmove_line.expect("no bestmove found");
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

#[test]
fn uci_reports_options_and_handles_setoption() {
    let exe = env!("CARGO_BIN_EXE_chess_engine");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine binary");

    let input = b"uci\nsetoption name Move Overhead value 0\nsetoption name SoftTime value 75\nsetoption name HardTime value 95\nsetoption name Nodes value 10000\nsetoption name MultiPV value 2\nsetoption name Ponder value true\nisready\nquit\n";
    child.stdin.as_mut().unwrap().write_all(input).unwrap();

    let output = child.wait_with_output().expect("failed to read output");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("option name Move Overhead"));
    assert!(stdout.contains("option name SoftTime"));
    assert!(stdout.contains("option name HardTime"));
    assert!(stdout.contains("option name Nodes"));
    assert!(stdout.contains("option name MultiPV"));
    assert!(stdout.contains("option name Ponder"));
    assert!(stdout.contains("readyok"));
}

#[test]
fn uci_go_depth_returns_legal_move() {
    let exe = env!("CARGO_BIN_EXE_chess_engine");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine binary");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(b"uci\nisready\nposition startpos\ngo depth 2\n")
        .unwrap();

    let mut bestmove_line = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).expect("read failed");
        if bytes == 0 {
            break;
        }
        if line.starts_with("bestmove") {
            bestmove_line = Some(line);
            break;
        }
    }

    stdin.write_all(b"quit\n").unwrap();
    let _ = child.wait();

    let bestmove = bestmove_line.expect("no bestmove found");
    let parts: Vec<&str> = bestmove.split_whitespace().collect();
    assert!(parts.len() >= 2, "bestmove missing move: {}", bestmove);
    let mv = parts[1];
    assert_ne!(mv, "0000", "engine returned null move");

    let mut board = Board::new();
    let parts = ["position", "startpos"];
    parse_position_command(&mut board, &parts);

    let legal = parse_uci_move(&mut board, mv).is_some();
    assert!(legal, "bestmove not legal in position: {}", mv);
}

#[test]
fn uci_perft_command_outputs_nodes() {
    let exe = env!("CARGO_BIN_EXE_chess_engine");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine binary");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(b"uci\nisready\nposition startpos\nperft 1\n")
        .unwrap();

    let mut saw_perft = false;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).expect("read failed");
        if bytes == 0 {
            break;
        }
        if line.contains("perft depth 1") && line.contains("nodes 20") {
            saw_perft = true;
            break;
        }
    }

    stdin.write_all(b"quit\n").unwrap();
    let _ = child.wait();

    assert!(saw_perft, "perft output missing");
}

#[test]
fn uci_stop_interrupts_search() {
    let exe = env!("CARGO_BIN_EXE_chess_engine");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine binary");

    let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    stdin
        .lock()
        .unwrap()
        .write_all(b"uci\nisready\nposition startpos\ngo infinite\n")
        .unwrap();

    let stdin_clone = Arc::clone(&stdin);
    let stop_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let _ = stdin_clone.lock().unwrap().write_all(b"stop\n");
    });

    let mut bestmove_line = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).expect("read failed");
        if bytes == 0 {
            break;
        }
        if line.starts_with("bestmove") {
            bestmove_line = Some(line);
            break;
        }
    }

    let _ = stop_thread.join();
    stdin.lock().unwrap().write_all(b"quit\n").unwrap();
    let _ = child.wait();

    let bestmove = bestmove_line.expect("no bestmove found");
    let parts: Vec<&str> = bestmove.split_whitespace().collect();
    assert!(parts.len() >= 2, "bestmove missing move: {}", bestmove);
    assert_ne!(parts[1], "0000", "engine returned null move");
}

#[test]
fn uci_go_mate_returns_legal_move() {
    let exe = env!("CARGO_BIN_EXE_chess_engine");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine binary");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(b"uci\nisready\nposition startpos\ngo mate 1\n")
        .unwrap();

    let mut bestmove_line = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).expect("read failed");
        if bytes == 0 {
            break;
        }
        if line.starts_with("bestmove") {
            bestmove_line = Some(line);
            break;
        }
    }

    stdin.write_all(b"quit\n").unwrap();
    let _ = child.wait();

    let bestmove = bestmove_line.expect("no bestmove found");
    let parts: Vec<&str> = bestmove.split_whitespace().collect();
    assert!(parts.len() >= 2, "bestmove missing move: {}", bestmove);
    let mv = parts[1];
    assert_ne!(mv, "0000", "engine returned null move");
}
