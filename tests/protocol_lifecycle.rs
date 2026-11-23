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

#[cfg(feature = "syzygy")]
#[test]
fn uci_invalid_syzygy_path_keeps_normal_search_available() {
    let mut engine = Engine::spawn("--uci");
    engine.send("uci\n");
    assert!(engine
        .until("uciok")
        .iter()
        .any(|line| { line.starts_with("option name SyzygyPath type string") }));
    engine.send("setoption name Hash value 1\nsetoption name SyzygyPath value :\n");
    engine.until("info string SyzygyPath:");
    engine.send("position startpos\ngo depth 1\n");
    assert!(engine
        .until("bestmove")
        .iter()
        .any(|line| line.starts_with("info depth 1 ")));
}

#[cfg(feature = "syzygy")]
#[test]
#[ignore = "requires CHESS_ENGINE_TEST_SYZYGY_PATH with KRPvKR and its dependencies"]
fn uci_syzygy_reloads_and_honors_root_domain_and_clock() {
    let path = std::env::var("CHESS_ENGINE_TEST_SYZYGY_PATH").unwrap();
    let mut engine = Engine::spawn("--uci");
    engine.send("setoption name Hash value 1\nsetoption name Threads value 3\n");
    for reload in 0..2 {
        engine.send(&format!(
            "setoption name SyzygyPath value {path}\nisready\n"
        ));
        engine.until("readyok");
        engine.send("position fen 7r/R7/4P3/8/8/7k/3K4/8 b - - 0 1\ngo nodes 48000\n");
        let lines = engine.until("bestmove");
        assert!(lines
            .iter()
            .any(|line| line.contains("depth 0") && line.contains("score cp 0")));
        assert!(lines.last().unwrap().contains("h8e8") || lines.last().unwrap().contains("h3g4"));
        engine.send("go searchmoves h8b8 nodes 1\n");
        let lines = engine.until("bestmove");
        assert!(lines
            .iter()
            .any(|line| line.contains("score cp -1000 pv h8b8")));
        if reload == 0 {
            let missing =
                std::env::temp_dir().join(format!("chess-engine-no-tables-{}", std::process::id()));
            engine.send(&format!(
                "setoption name SyzygyPath value {}\n",
                missing.display()
            ));
            engine.until("info string SyzygyPath:");
            engine.send("go searchmoves h8b8 nodes 1\n");
            assert!(engine
                .until("bestmove")
                .iter()
                .any(|line| line.contains("score cp -1000 pv h8b8")));
        }
        engine.send("setoption name SyzygyPath value <empty>\nisready\n");
        engine.until("readyok");
        engine.send("go nodes 1\n");
        assert!(!engine
            .until("bestmove")
            .iter()
            .any(|line| line.starts_with("info depth 0 ")));
    }
    engine.send(&format!("setoption name SyzygyPath value {path}\n"));
    for (clock, expected) in [(0, "score cp 1000"), (99, "score cp 0")] {
        engine.send(&format!(
            "position fen 1K6/5P2/8/8/R7/1k6/5r2/8 w - - {clock} 1\ngo nodes 1\n"
        ));
        assert!(engine
            .until("bestmove")
            .iter()
            .any(|line| line.contains(expected)));
    }
}

#[cfg(feature = "syzygy")]
#[test]
#[ignore = "requires CHESS_ENGINE_TEST_SYZYGY_PATH with KPvKP and its dependencies"]
fn uci_syzygy_resolves_en_passant_and_underpromotions() {
    let path = std::env::var("CHESS_ENGINE_TEST_SYZYGY_PATH").unwrap();
    let mut engine = Engine::spawn("--uci");
    engine.send(&format!(
        "setoption name Hash value 1\nsetoption name Threads value 3\nsetoption name SyzygyPath value {path}\n"
    ));
    for (fen, moves, best, score) in [
        ("8/8/8/3Pp3/8/4K3/8/7k w - e6 0 2", "d5e6", "d5e6", "1000"),
        ("8/P4k2/8/8/8/8/6Kp/8 w - - 0 1", "a7a8n", "a7a8n", "0"),
        (
            "8/P4k2/8/8/8/8/6Kp/8 w - - 100 1",
            "a7a8r a7a8n",
            "a7a8r",
            "998",
        ),
    ] {
        engine.send(&format!(
            "position fen {fen}\ngo searchmoves {moves} nodes 1\n"
        ));
        let lines = engine.until("bestmove");
        assert_eq!(lines.last().unwrap().split_whitespace().nth(1), Some(best));
        assert!(
            lines.iter().any(|line| line.starts_with("info depth 0 ")
                && line.ends_with(&format!("score cp {score} pv {best}"))),
            "{lines:?}"
        );
    }
}

#[cfg(feature = "syzygy")]
#[test]
#[ignore = "requires CHESS_ENGINE_TEST_SYZYGY_PATH with KRPvKR and its dependencies"]
fn uci_syzygy_sees_the_five_piece_result_after_a_capture() {
    let path = std::env::var("CHESS_ENGINE_TEST_SYZYGY_PATH").unwrap();
    let mut engine = Engine::spawn("--uci");
    engine.send(&format!(
        "setoption name Hash value 4\nsetoption name SyzygyPath value {path}\n"
    ));
    for (threads, limits) in [(1, "nodes 32"), (4, "depth 4")] {
        engine.send(&format!(
            "setoption name Threads value {threads}\n\
             ucinewgame\n\
             position fen 6r1/1K5k/2p5/8/7P/8/2R5/8 w - - 10 1\n\
             go searchmoves b7c6 c2c6 {limits}\n"
        ));
        let lines = engine.until("bestmove");
        assert!(
            lines.iter().any(|line| line.contains("score cp 0 pv c2c6")),
            "{lines:?}"
        );
        assert!(lines.last().unwrap().starts_with("bestmove c2c6"));
    }
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
            (
                "6k1/5ppp/8/8/8/8/8/4Q2K w - - 100 1",
                "score mate 1 pv e1e8",
            ),
            ("6k1/5ppp/8/8/8/8/8/4Q2K w - - 150 1", "score cp 0"),
            (
                "7k/5K2/8/8/8/8/8/R7 b - - 0 1",
                "score mate -1 pv h8h7 a1h1",
            ),
            ("7k/5K2/8/8/8/8/P7/R7 b - - 0 1", "info depth 2"),
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

        engine.send(concat!(
            "position fen 7k/8/8/8/8/8/8/1Q4K1 w - - 0 1 ",
            "moves b1b2 h8h7 b2b1 h7h8 b1b2 h8h7\n",
            "go searchmoves b2b1 depth 2\n"
        ));
        let lines = engine.until("bestmove");
        assert!(
            lines.iter().any(|line| line.contains("score cp 0 pv b2b1")),
            "{lines:?}"
        );
        assert!(lines.last().unwrap().starts_with("bestmove b2b1"));

        for (cycles, score) in [(2, "score mate 1 pv e1e8"), (4, "score cp 0")] {
            let moves = " h1g1 g8f8 g1h1 f8g8".repeat(cycles);
            engine.send(&format!(
                "position fen 6k1/5ppp/8/8/8/8/8/4Q2K w - - 0 1 moves{moves}\ngo depth 2\n"
            ));
            let lines = engine.until("bestmove");
            assert!(lines.iter().any(|line| line.contains(score)), "{lines:?}");
        }
        engine.send("position fen 8/8/7P/8/8/6k1/8/4K3 w - - 100 1\ngo nodes 1\n");
        let lines = engine.until("bestmove");
        assert!(
            lines.last().unwrap().starts_with("bestmove h6h7"),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("score mate")),
            "{lines:?}"
        );
        engine.send("quit\n");
        engine.wait_for_exit();
    }
}

#[test]
fn interrupted_search_keeps_a_proven_draw_two_plies_before_the_claim() {
    let mut engine = Engine::spawn("--uci");
    engine.send(concat!(
        "setoption name Hash value 1\n",
        "position fen 8/2PR4/8/8/8/2K5/8/k2r4 b - - 98 1\n",
        "go nodes 1\n"
    ));
    assert!(engine
        .until("bestmove")
        .last()
        .unwrap()
        .starts_with("bestmove d1c1"));

    engine.send("go searchmoves d1d7 nodes 1\n");
    assert!(engine
        .until("bestmove")
        .last()
        .unwrap()
        .starts_with("bestmove d1d7"));
}

#[test]
fn uci_preserves_fifty_move_draws_and_can_search_a_winning_clock_reset() {
    for threads in [1, 3] {
        let mut engine = Engine::spawn("--uci");
        engine.send(&format!(
            "setoption name Hash value 1\nsetoption name Threads value {threads}\n"
        ));
        for clock in [99, 100] {
            engine.send(&format!(
                "position fen 8/6R1/8/6r1/3K4/8/1P6/4k3 w - - {clock} 1\ngo nodes 1\n"
            ));
            let lines = engine.until("bestmove");
            assert!(lines.last().unwrap().starts_with("bestmove g7g6"));

            engine.send("go depth 3\n");
            let lines = engine.until("bestmove");
            assert!(lines.last().unwrap().starts_with("bestmove g7g5"));
        }
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
    let fen = "7k/8/8/8/3q4/8/P2R4/6K1 w - - 0 1";
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
fn xboard_ponder_variation_starts_with_the_predicted_reply() {
    let mut engine = Engine::xboard();
    engine.send("sd 4\npost\nhard\ngo\n");
    let search = engine.until("move ");
    let played = search.last().unwrap().strip_prefix("move ").unwrap();
    let expected_reply = search
        .iter()
        .find(|line| line.starts_with("4 "))
        .and_then(|line| line.split_whitespace().nth(5))
        .expect("the final search PV should predict a reply");
    let ponder = engine.until("4 ");
    let mut board = Board::new();
    let mv = board.parse_san(played).unwrap();
    board.make_move_uci(&mv.to_string()).unwrap();
    for line in ponder {
        let mut fields = line.split_whitespace().skip(4);
        let reply = fields.next().expect("ponder PV should show a reply");
        assert_eq!(reply, expected_reply, "{line}");
        let mut position = board.clone();
        for san in std::iter::once(reply).chain(fields) {
            let mv = position.parse_san(san).expect("ponder PV must be legal");
            position.make_move_uci(&mv.to_string()).unwrap();
        }
    }
    engine.send("quit\n");
    engine.wait_for_exit();
}

#[test]
fn xboard_reports_mate_in_the_protocol_score_range() {
    let mut engine = Engine::xboard();
    let fen = "7k/5Q2/6K1/8/8/8/8/8 w - - 0 1";
    engine.send(&format!("setboard {fen}\nsd 1\npost\ngo\n"));
    let lines = engine.until("move ");
    assert!(lines.iter().any(|line| line.starts_with("0 100001 ")));
    assert_one_legal_xboard_move(&lines, &mut Board::from_fen(fen));
    engine.send("quit\n");
    engine.wait_for_exit();
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
