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

    fn static_eval(&mut self) -> i32 {
        self.write("eval\n");
        let (_, line) = self.read_until(|line| line.starts_with("info string eval "));
        line.expect("missing eval output")
            .split_whitespace()
            .last()
            .expect("missing static score")
            .parse()
            .expect("invalid static score")
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
fn uci_reports_live_hce_and_nnue_option_values() {
    let mut uci = UciHarness::spawn();
    uci.write(
        "setoption name NnueEvalScale value 125\n\
         setoption name NnueHceBlend value 55\n\
         setoption name NnuePureStaticEval value true\n\
         setoption name UseFullHCE value false\n\
         setoption name UseTunedHCE value false\n\
         setoption name UseFullHCEStaticEval value true\n\
         setoption name NnueStaticEvalScale value 150\n\
         setoption name NnueStaticBlend value 25\n\
         uci\n",
    );
    let (stdout, uciok) = uci.read_until(|line| line.starts_with("uciok"));
    uci.send_quit_and_wait();

    assert!(uciok.is_some());
    for expected in [
        "option name NnueEvalScale type spin default 125",
        "option name NnueHceBlend type spin default 55",
        "option name NnuePureStaticEval type check default true",
        "option name UseFullHCE type check default false",
        "option name UseTunedHCE type check default false",
        "option name UseFullHCEStaticEval type check default true",
        "option name NnueStaticEvalScale type spin default 150",
        "option name NnueStaticBlend type spin default 25",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in {stdout}"
        );
    }
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
fn uci_two_knights_vs_bare_king_keeps_mate_in_one() {
    let mut uci = UciHarness::spawn();
    let position = [
        "position",
        "fen",
        "8/8/8/8/8/2N5/8/k1K1N3",
        "w",
        "-",
        "-",
        "0",
        "1",
    ];
    uci.write("position fen 8/8/8/8/8/2N5/8/k1K1N3 w - - 0 1\ngo depth 12\n");
    let (_, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_eq!(bestmove_uci(&bestmove), "e1c2");
    assert_bestmove_legal(&bestmove, &position);
}

#[test]
fn uci_two_knights_vs_bare_king_avoids_immediate_mate_blunder() {
    let mut uci = UciHarness::spawn();
    let position = [
        "position",
        "fen",
        "8/8/8/NK6/8/8/1k6/2N5",
        "b",
        "-",
        "-",
        "0",
        "1",
    ];
    uci.write("position fen 8/8/8/NK6/8/8/1k6/2N5 b - - 0 1\ngo depth 12\n");
    let (_, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_ne!(bestmove_uci(&bestmove), "b2a3", "...Ka3? permits Nc2#");
    assert_bestmove_legal(&bestmove, &position);
}

#[test]
fn uci_rook_defense_preserves_syzygy_draws_under_node_limit() {
    let mut uci = UciHarness::spawn();
    uci.write("setoption name Hash value 16\nsetoption name Threads value 1\n");

    // The listed moves are all the Syzygy draws from each position.
    for (fen, nodes, drawing_moves) in [
        (
            "7r/R7/4P3/8/8/7k/3K4/8 b - - 0 1",
            120_000,
            &["h8e8", "h3g4"][..],
        ),
        (
            "8/8/8/7r/8/1k4K1/4PR2/8 b - - 0 1",
            26_000,
            &["h5h8", "h5h7", "b3c4", "b3c3"][..],
        ),
    ] {
        uci.write(&format!(
            "ucinewgame\nposition fen {fen}\ngo depth 7 nodes {nodes}\n"
        ));
        let (_, bestmove) = uci.read_until_bestmove();
        let mv = bestmove_uci(&bestmove);
        assert!(drawing_moves.contains(&mv), "{mv} loses in {fen}");
    }

    uci.send_quit_and_wait();
}

#[test]
fn uci_interrupted_iteration_can_return_a_verified_root_move() {
    let mut uci = UciHarness::spawn();
    uci.write(
        "setoption name Hash value 16\n\
         position fen 2r2rk1/1p3p1p/p3b1pP/8/3RQ3/qP3P2/P1P3P1/1K3B1R b - - 0 1\n\
         go nodes 1000\n",
    );
    let (output, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_eq!(bestmove_uci(&bestmove), "e6f5");
    assert!(output
        .lines()
        .filter(|line| line.starts_with("info depth 1 "))
        .filter_map(|line| line.split_once(" pv "))
        .any(|(_, pv)| pv.split_whitespace().next().is_some_and(|mv| mv != "e6f5")));
    assert!(!output.lines().any(|line| line.starts_with("info depth 2 ")));
}

#[test]
fn uci_interrupted_first_iteration_can_return_a_searched_move() {
    let mut uci = UciHarness::spawn();
    uci.write(
        "setoption name Hash value 16\n\
         position fen 2kr4/1p1r1p2/2p1p2p/8/P1P2R2/8/1P3PPP/R5K1 b - - 0 1\n\
         go nodes 16\n",
    );
    let (output, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_eq!(bestmove_uci(&bestmove), "d7d1");
    assert!(!output.lines().any(|line| line.starts_with("info depth ")));
}

#[test]
fn uci_unsearched_fallback_prefers_a_winning_capture() {
    let mut uci = UciHarness::spawn();
    uci.write(
        "setoption name Hash value 4\n\
         position fen 6k1/6pp/8/3q4/3R4/8/PP6/6K1 w - - 0 1\n\
         go nodes 1\n",
    );
    let (output, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_eq!(bestmove_uci(&bestmove), "d4d5");
    assert!(!output.lines().any(|line| line.starts_with("info depth ")));
}

#[test]
fn uci_unsearched_fallback_prefers_mate_then_a_safe_quiet_move() {
    let mut uci = UciHarness::spawn();
    let mating_position = "position fen 6k1/p4ppp/5b2/3q4/Q7/P4NP1/5PKP/8 w - - 0 1";
    for (position, limits, expected) in [
        (mating_position, "nodes 1", "a4e8"),
        (mating_position, "searchmoves a4a7 f3h4 nodes 1", "a4a7"),
        ("position startpos", "nodes 1", "g1f3"),
    ] {
        uci.write(&format!("ucinewgame\n{position}\ngo {limits}\n"));
        let (output, bestmove) = uci.read_until_bestmove();
        assert_eq!(bestmove_uci(&bestmove), expected);
        assert!(!output.lines().any(|line| line.starts_with("info depth ")));
    }
    uci.send_quit_and_wait();
}

#[test]
fn uci_checks_the_quiet_reply_before_giving_up_a_draw_claim() {
    let mut uci = UciHarness::spawn();
    uci.write("setoption name Hash value 4\n");
    let position = "position fen K7/5r2/8/4P1R1/5k2/8/8/8 w - - 99 1";
    let parts: Vec<_> = position.split_whitespace().collect();
    for cap in [16, 32] {
        uci.write(&format!("ucinewgame\n{position}\ngo nodes {cap}\n"));
        let (_, bestmove) = uci.read_until_bestmove();
        assert_bestmove_legal(&bestmove, &parts);
        assert_ne!(
            bestmove_uci(&bestmove),
            "e5e6",
            "...Rf8+ wins after the pawn push"
        );
    }
    uci.send_quit_and_wait();
}

#[test]
fn uci_tiny_movetime_returns_a_legal_fallback_move() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\ngo movetime 1\n");
    let (_, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    assert_bestmove_legal(&bestmove, &["position", "startpos"]);
}

#[test]
fn uci_go_depth_keeps_clock_limits() {
    let mut uci = UciHarness::spawn();
    uci.write("uci\nisready\nposition startpos\ngo depth 2 wtime 10000 btime 10000\n");
    let (output, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();

    let time_line = output
        .lines()
        .find(|line| line.starts_with("info string time"))
        .expect("missing time info");
    assert!(time_line.contains("depth 2"), "{time_line}");
    assert!(!time_line.contains("soft 0"), "{time_line}");
    assert!(
        !time_line.contains(&format!("soft {}", u64::MAX)),
        "{time_line}"
    );
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
fn uci_evalfeatures_reports_tuned_hce_score() {
    let mut uci = UciHarness::spawn();
    uci.write("position startpos\nevalfeatures\n");
    let (_, line) = uci.read_until(|line| line.starts_with("info string evalfeatures"));
    uci.send_quit_and_wait();

    let line = line.expect("missing evalfeatures output");
    assert!(line.contains("full_white "), "{line}");
    assert!(line.contains("tuned_full_white "), "{line}");
}

#[test]
fn uci_eval_reports_the_static_score_used_by_search() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let board = Board::from_fen(fen);
    let mut uci = UciHarness::spawn();
    uci.write(&format!(
        "setoption name UseNNUE value false\nposition fen {fen}\neval\n"
    ));
    let (_, line) = uci.read_until(|line| line.starts_with("info string eval "));
    let line = line.expect("missing eval output");
    assert!(
        line.contains(&format!(" static {}", board.evaluate_simple())),
        "{line}"
    );

    uci.write("setoption name UseFullHceStaticEval value true\neval\n");
    let (_, line) = uci.read_until(|line| line.starts_with("info string eval "));
    uci.send_quit_and_wait();
    let line = line.expect("missing full HCE eval output");
    assert!(
        line.contains(&format!(" static {}", board.evaluate_tuned_hce())),
        "{line}"
    );
}

#[cfg(feature = "embedded_nnue")]
#[test]
fn uci_uses_the_embedded_blend_for_static_evaluation_by_default() {
    let fen = "r1bqk2r/ppp2ppp/2np1n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQ1RK1 w kq - 2 7";
    let board = Board::from_fen(fen);
    let network = chess_engine::board::nnue::NnueNetwork::from_embedded();
    let expected = (60 * board.evaluate_simple() + 40 * board.evaluate_nnue(&network)) / 100;

    let mut uci = UciHarness::spawn();
    uci.write(&format!("uci\nposition fen {fen}\neval\n"));
    let (output, line) = uci.read_until(|line| line.starts_with("info string eval "));
    uci.send_quit_and_wait();

    assert!(output.contains("option name UseNNUE type check default true"));
    assert!(output.contains("option name NnueHceBlend type spin default 40"));
    assert!(line
        .expect("missing eval output")
        .trim_end()
        .ends_with(&format!(" static {expected}")));
}

#[test]
fn uci_loads_external_static_nnue_without_configuring_a_main_file() {
    let fen = "r1bqk2r/ppp2ppp/2np1n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQ1RK1 w kq - 2 7";
    let board = Board::from_fen(fen);
    let network_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/board/nnue/nets/default.nnue");
    let network = chess_engine::board::nnue::NnueNetwork::load(&network_path).unwrap();
    let hce = board.evaluate_simple();
    let nnue = board.evaluate_nnue(&network);
    assert_ne!(hce, nnue);

    let mut uci = UciHarness::spawn();
    uci.write(&format!(
        "position fen {fen}\n\
         setoption name UseNNUE value false\n\
         setoption name UseFullHCE value false\n\
         setoption name UseFullHCEStaticEval value false\n\
         setoption name NnuePureStaticEval value false\n\
         setoption name StaticEvalFile value {}\n\
         setoption name UseNNUE value true\n",
        network_path.display()
    ));
    assert_eq!(uci.static_eval(), nnue);
    uci.write("setoption name UseNNUE value false\n");
    assert_eq!(uci.static_eval(), hce);
    uci.write("setoption name UseNNUE value true\n");
    assert_eq!(uci.static_eval(), nnue);
    uci.write("setoption name StaticEvalFile value\n");
    assert_eq!(uci.static_eval(), hce);

    uci.write(&format!(
        "setoption name EvalFile value {}\n\
         setoption name NnuePureStaticEval value true\n\
         setoption name NnueHceBlend value 100\n",
        network_path.display()
    ));
    assert_eq!(uci.static_eval(), nnue);
    uci.write("setoption name NnueHceBlend value 0\n");
    assert_eq!(uci.static_eval(), hce);
    uci.write("go nodes 64\n");
    let (_, bestmove) = uci.read_until_bestmove();
    uci.send_quit_and_wait();
    let position = format!("position fen {fen}");
    assert_bestmove_legal(&bestmove, &position.split_whitespace().collect::<Vec<_>>());
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
