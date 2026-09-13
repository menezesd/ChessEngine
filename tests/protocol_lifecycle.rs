use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use chess_engine::board::Board;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Every read has a deadline, and Drop kills the child on assertion failure.
struct Engine {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<String>,
}

impl Engine {
    fn spawn(mode: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_chess_engine"))
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            output,
        }
    }

    fn xboard() -> Self {
        let mut engine = Self::spawn("--xboard");
        engine.send("xboard\nprotover 2\nmemory 1\nforce\nsd 30\nping 1\n");
        engine.until("pong 1");
        engine
    }

    fn send(&mut self, commands: &str) {
        let input = self.input.as_mut().unwrap();
        input.write_all(commands.as_bytes()).unwrap();
        input.flush().unwrap();
    }

    fn until(&self, prefix: &str) -> Vec<String> {
        self.until_matching(|line| line.starts_with(prefix), prefix)
    }

    fn until_matching(&self, matches: impl Fn(&str) -> bool, description: &str) -> Vec<String> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut lines = Vec::new();
        loop {
            let line = self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|err| {
                    panic!("waiting for {description:?}: {err}; output: {lines:?}")
                });
            let done = matches(&line);
            lines.push(line);
            if done {
                return lines;
            }
        }
    }

    fn assert_no_bestmove_for(&self, duration: Duration) {
        let deadline = Instant::now() + duration;
        loop {
            match self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(line) => assert!(!line.starts_with("bestmove"), "early result: {line}"),
                Err(RecvTimeoutError::Timeout) => return,
                Err(RecvTimeoutError::Disconnected) => panic!("engine exited unexpectedly"),
            }
        }
    }

    fn wait_for_exit(&mut self) {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "engine exited with {status}");
                return;
            }
            assert!(Instant::now() < deadline, "engine did not exit");
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn assert_one_legal_xboard_move(lines: &[String], board: &mut Board) {
    let moves: Vec<_> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("move "))
        .collect();
    assert_eq!(moves.len(), 1, "expected one move: {lines:?}");
    let mv = board
        .parse_san(moves[0])
        .expect("engine move must be legal");
    assert!(board.is_legal_move(mv));
}

#[test]
fn uci_reports_scores_for_forced_moves_and_resolved_roots() {
    for threads in [1, 2] {
        let mut engine = Engine::spawn("--uci");
        engine.send(&format!(
            "uci\nsetoption name Hash value 1\nsetoption name Threads value {threads}\nisready\n"
        ));
        engine.until("readyok");
        for (fen, expected) in [
            ("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1", "score mate 0"),
            ("7k/5K2/6Q1/8/8/8/8/8 b - - 0 1", "score cp 0"),
            ("7k/8/8/8/8/8/8/K1B5 w - - 0 1", "score cp 0"),
            ("8/8/8/NK6/8/k7/8/2N5 w - - 0 1", "score mate 1"),
            ("7k/5K2/8/8/8/8/8/R7 b - - 0 1", "info depth 2"),
        ] {
            engine.send(&format!("position fen {fen}\ngo depth 2\n"));
            let lines = engine.until("bestmove");
            assert!(
                lines.iter().any(|line| line.contains(expected)),
                "missing {expected} for {fen}, Threads={threads}: {lines:?}"
            );
        }
        engine.send("position fen 8/8/8/NK6/8/8/1k6/2N5 b - - 0 1\ngo searchmoves b2a3 depth 2\n");
        let lines = engine.until("bestmove");
        assert!(
            lines.iter().any(|line| line.contains("score mate -1")),
            "{lines:?}"
        );
        assert!(lines.last().unwrap().starts_with("bestmove b2a3"));
        engine.send("quit\n");
        engine.wait_for_exit();
    }
}

#[test]
fn uci_searchmoves_restricts_the_result_and_reported_variations() {
    for (threads, multipv) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
        let mut engine = Engine::spawn("--uci");
        engine.send(&format!(
            "uci\nsetoption name Hash value 1\nsetoption name UseNNUE value false\nsetoption name Threads value {threads}\nsetoption name MultiPV value {multipv}\nisready\n"
        ));
        engine.until("readyok");
        engine.send("position startpos\ngo searchmoves a2a3 depth 2\n");
        let lines = engine.until("bestmove");
        assert!(
            lines.last().unwrap().starts_with("bestmove a2a3"),
            "{lines:?}"
        );
        let variations: Vec<_> = lines
            .iter()
            .filter_map(|line| line.split_once(" pv "))
            .collect();
        assert!(
            !variations.is_empty(),
            "restricted move was not analyzed: {lines:?}"
        );
        assert!(variations
            .iter()
            .all(|(_, pv)| pv.split_whitespace().next() == Some("a2a3")));
        engine.send("quit\n");
        engine.wait_for_exit();
    }
}

#[test]
fn uci_empty_searchmoves_does_not_fall_back_to_unrestricted_search() {
    let mut engine = Engine::spawn("--uci");
    engine.send("uci\nsetoption name Hash value 1\nsetoption name MultiPV value 3\nisready\n");
    engine.until("readyok");
    for command in ["go searchmoves depth 1", "go searchmoves a1a8 depth 1"] {
        engine.send(&format!("position startpos\n{command}\n"));
        assert_eq!(engine.until("bestmove").last().unwrap(), "bestmove 0000");
    }
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn uci_position_cancels_old_search_without_publishing_bestmove() {
    let mut engine = Engine::spawn("--uci");
    engine.send("uci\nisready\n");
    engine.until("readyok");

    engine.send("position startpos\ngo infinite\n");
    engine.send("position fen 7k/8/8/8/8/8/8/K7 w - - 0 1\nisready\n");
    let lines = engine.until("readyok");

    assert!(
        !lines.iter().any(|line| line.starts_with("bestmove")),
        "cancelled search leaked a result: {lines:?}"
    );
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_move_now_and_ping_work_during_search() {
    let mut engine = Engine::xboard();
    engine.send("go\nping 2\n");
    engine.until("pong 2");
    engine.send("hint\n?\nping 3\n");
    let lines = engine.until("pong 3");
    assert_one_legal_xboard_move(&lines, &mut Board::new());

    engine.send("ping 4\n");
    assert_eq!(engine.until("pong 4"), ["pong 4"]);
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_quit_and_eof_interrupt_search() {
    for quit in [true, false] {
        let mut engine = Engine::xboard();
        engine.send("go\nping 2\n");
        engine.until("pong 2");
        if quit {
            engine.send("quit\n");
        } else {
            engine.input.take();
        }
        engine.wait_for_exit();
    }
}

#[test]
fn xboard_force_discards_search_before_a_new_position() {
    let mut engine = Engine::xboard();
    engine.send("go\nping 2\n");
    engine.until("pong 2");
    engine.send("force\nping 3\n");
    assert_eq!(engine.until("pong 3"), ["pong 3"]);

    let fen = "7k/8/8/8/8/8/8/K7 w - - 0 1";
    engine.send(&format!("setboard {fen}\nsd 1\ngo\n"));
    let lines = engine.until("move ");
    assert_one_legal_xboard_move(&lines, &mut Board::from_fen(fen));
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_pause_preserves_position_and_resume_restarts_search() {
    let mut engine = Engine::xboard();
    engine.send("go\nping 2\n");
    engine.until("pong 2");
    engine.send("pause\nping 3\n");
    assert_eq!(engine.until("pong 3"), ["pong 3"]);
    engine.send("sd 1\nresume\n");
    assert_one_legal_xboard_move(&engine.until("move "), &mut Board::new());
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_depth_limit_applies_alongside_a_clock() {
    let mut engine = Engine::xboard();
    engine.send("st 60\nsd 1\ngo\n");
    assert_one_legal_xboard_move(&engine.until("move "), &mut Board::new());
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_post_reports_search_scores_and_variations() {
    let fen = "7k/8/8/8/3q4/8/3R4/6K1 w - - 0 1";
    for mode in ["analyze", "go"] {
        let mut engine = Engine::xboard();
        engine.send(&format!("setboard {fen}\nsd 3\npost\n{mode}\n"));
        let lines = engine.until("3 ");
        let fields: Vec<_> = lines.last().unwrap().split_whitespace().collect();
        assert!(
            fields[1].parse::<i32>().unwrap() > 0,
            "wrong score: {fields:?}"
        );
        assert_eq!(fields[4], "Rxd4", "wrong PV: {fields:?}");
        let mut board = Board::from_fen(fen);
        for san in &fields[4..] {
            let mv = board.parse_san(san).expect("PV must contain legal moves");
            board.make_move_uci(&mv.to_string()).unwrap();
        }
        if mode == "analyze" {
            assert!(!lines.iter().any(|line| line.starts_with("move ")));
        }
        engine.send("quit\n");
        engine.wait_for_exit();
    }
}

#[test]
fn xboard_can_enable_post_while_analysis_is_running() {
    let mut engine = Engine::xboard();
    engine.send("nopost\nanalyze\nping 2\n");
    assert_eq!(engine.until("pong 2"), ["pong 2"]);
    engine.send("post\n");
    engine.until_matching(
        |line| line.as_bytes().first().is_some_and(u8::is_ascii_digit),
        "thinking output after post",
    );
    engine.send("nopost\npause\nping 3\n");
    engine.until("pong 3");
    engine.send("resume\nping 4\n");
    assert_eq!(engine.until("pong 4"), ["pong 4"]);
    assert!(matches!(
        engine.output.recv_timeout(Duration::from_millis(50)),
        Err(RecvTimeoutError::Timeout)
    ));
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_edit_keeps_the_selected_turn_and_does_not_play_placements() {
    let mut engine = Engine::xboard();
    engine.send("black\nedit\n#\nKa1\nc\nKc1\nc\n.\nsd 1\ngo\n");
    let lines = engine.until("move ");
    assert!(lines
        .iter()
        .all(|line| !line.starts_with("Illegal") && !line.starts_with("Error")));
    assert_one_legal_xboard_move(&lines, &mut Board::from_fen("8/8/8/8/8/8/8/K1k5 b - - 0 1"));
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn uci_infinite_waits_for_stop_even_when_root_is_resolved() {
    for threads in [1, 2] {
        let mut engine = Engine::spawn("--uci");
        engine.send(&format!(
            "uci\nsetoption name Hash value 1\nsetoption name Threads value {threads}\nisready\n"
        ));
        engine.until("readyok");
        let fen = "7k/8/8/8/8/8/8/K7 w - - 0 1";
        engine.send(&format!("position fen {fen}\ngo infinite\n"));
        engine.until("info string time");
        engine.assert_no_bestmove_for(Duration::from_millis(100));
        engine.send("ponderhit\nisready\n");
        assert_eq!(engine.until("readyok"), ["readyok"]);
        engine.assert_no_bestmove_for(Duration::from_millis(50));

        engine.send("stop\n");
        let lines = engine.until("bestmove ");
        let bestmove = lines.last().unwrap().split_whitespace().nth(1).unwrap();
        assert!(Board::from_fen(fen).parse_move(bestmove).is_ok());
        engine.send("isready\n");
        assert_eq!(engine.until("readyok"), ["readyok"]);
        engine.send("quit\n");
        engine.wait_for_exit();
    }
}
