use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chess_engine::board::Board;
use chess_engine::uci::{parse_position_command, parse_uci_move};

struct UciHarness {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    reader: BufReader<ChildStdout>,
}

impl UciHarness {
    fn spawn() -> Self {
        let exe = env!("CARGO_BIN_EXE_chess_engine");
        let mut child = Command::new(exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn engine binary");

        let stdin = Arc::new(Mutex::new(
            child.stdin.take().expect("engine stdin missing"),
        ));
        let stdout = child.stdout.take().expect("engine stdout missing");
        let reader = BufReader::new(stdout);

        Self {
            child,
            stdin,
            reader,
        }
    }

    fn write(&self, input: &str) {
        self.stdin
            .lock()
            .expect("stdin lock poisoned")
            .write_all(input.as_bytes())
            .expect("failed to write engine input");
    }

    fn read_until(&mut self, predicate: impl Fn(&str) -> bool) -> (String, Option<String>) {
        let mut output = String::new();
        loop {
            let mut line = String::new();
            let bytes = self.reader.read_line(&mut line).expect("read failed");
            if bytes == 0 {
                return (output, None);
            }
            output.push_str(&line);
            if predicate(&line) {
                return (output, Some(line));
            }
        }
    }

    fn read_until_bestmove(&mut self) -> (String, String) {
        let (output, bestmove) = self.read_until(|line| line.starts_with("bestmove"));
        let bestmove = bestmove.expect("no bestmove found");
        (output, bestmove)
    }

    fn send_quit_and_wait(&mut self) {
        self.write("quit\n");
        let _ = self.child.wait();
    }

    fn stdin_handle(&self) -> Arc<Mutex<ChildStdin>> {
        Arc::clone(&self.stdin)
    }
}

fn bestmove_uci(bestmove_line: &str) -> &str {
    let parts: Vec<&str> = bestmove_line.split_whitespace().collect();
    assert!(parts.len() >= 2, "bestmove missing move: {}", bestmove_line);
    parts[1]
}

fn assert_bestmove_legal(bestmove_line: &str, position_parts: &[&str]) {
    let mv = bestmove_uci(bestmove_line);
    assert_ne!(mv, "0000", "engine returned null move");

    let mut board = Board::new();
    parse_position_command(&mut board, position_parts);

    let legal = parse_uci_move(&mut board, mv).is_some();
    assert!(legal, "bestmove not legal in position: {mv}");
}

#[test]
fn uci_smoke_test_returns_legal_move() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos moves e2e4\ngo movetime 50\n");
    let (output, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert!(output.contains("uciok"));
    assert!(output.contains("readyok"));
    assert!(output.contains("info string time"));
    assert_bestmove_legal(&bestmove, &["position", "startpos", "moves", "e2e4"]);
}

#[test]
fn uci_reports_options_and_handles_setoption() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nsetoption name Move Overhead value 0\nsetoption name Soft Time Percent value 75\nsetoption name Hard Time Percent value 95\nsetoption name Max Nodes value 10000\nsetoption name MultiPV value 2\nsetoption name Ponder value true\nisready\n");
    let (stdout, _) = uci.read_until(|line| line.starts_with("readyok"));
    uci.send_quit_and_wait();

    assert!(stdout.contains("option name Move Overhead"));
    assert!(stdout.contains("option name Soft Time Percent"));
    assert!(stdout.contains("option name Hard Time Percent"));
    assert!(stdout.contains("option name Max Nodes"));
    assert!(stdout.contains("option name MultiPV"));
    assert!(stdout.contains("option name Ponder"));
    assert!(stdout.contains("readyok"));
}

#[test]
fn uci_go_depth_returns_legal_move() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\ngo depth 2\n");
    let (_, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_bestmove_legal(&bestmove, &["position", "startpos"]);
}

#[test]
fn uci_perft_command_outputs_nodes() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\nperft 1\n");
    let (_, perft_line) =
        uci.read_until(|line| line.contains("perft depth 1") && line.contains("nodes 20"));
    uci.send_quit_and_wait();

    assert!(perft_line.is_some(), "perft output missing");
}

#[test]
fn uci_stop_interrupts_search() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\ngo infinite\n");

    let stdin_clone = uci.stdin_handle();
    let stop_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let _ = stdin_clone
            .lock()
            .expect("stdin lock poisoned")
            .write_all(b"stop\n");
    });

    let (_, bestmove) = uci.read_until_bestmove();
    let _ = stop_thread.join();
    uci.send_quit_and_wait();

    let mv = bestmove_uci(&bestmove);
    assert!(
        mv == "0000" || mv.len() >= 4,
        "engine returned malformed bestmove: {bestmove}"
    );
}

#[test]
fn uci_go_mate_returns_legal_move() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\ngo mate 1\n");
    let (_, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    let mv = bestmove_uci(&bestmove);
    assert_ne!(mv, "0000", "engine returned null move");
}

#[test]
fn uci_go_ponder_keeps_planned_clock_limits() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\ngo ponder wtime 10000 btime 10000 winc 0 binc 0\n");
    let (_, time_line) = uci.read_until(|line| line.starts_with("info string time"));
    uci.write("stop\n");
    uci.send_quit_and_wait();

    let time_line = time_line.expect("missing ponder time info");
    assert!(time_line.contains("ponder true"), "{time_line}");
    assert!(!time_line.contains("soft 0"), "{time_line}");
    assert!(!time_line.contains("hard 0"), "{time_line}");
    assert!(
        !time_line.contains(&format!("soft {}", u64::MAX)),
        "{time_line}"
    );
    assert!(
        !time_line.contains(&format!("hard {}", u64::MAX)),
        "{time_line}"
    );
}
