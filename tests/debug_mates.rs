use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use chess_engine::board::{find_best_move, search as run_search, Board, SearchConfig, SearchState};

const CASES: [(&str, &str); 5] = [
    ("2b2r1r/8/1BQp3q/8/1k3b2/8/1PP5/K4n2 w - - 0 1", "b6a5"),
    ("q6n/6Q1/2p2p2/2Pk1P2/3P3p/4PPb1/2PK4/8 w - - 0 1", "g7g8"),
    ("1b2n3/1rp5/2kPB3/2P1P3/P2P4/8/4Q3/Kn5q w - - 0 1", "e2h5"),
    ("4RB1n/q7/2p5/8/k1PP2n1/2K5/1P6/8 w - - 0 1", "e8a8"),
    ("8/8/8/3B4/4b2p/3N1p1p/5K1p/7k w - - 0 1", "d5a8"),
];

fn search(fen: &str, configure: impl FnOnce(&mut SearchState)) -> String {
    let mut board = Board::from_fen(fen);
    let mut state = SearchState::new(16);
    configure(&mut state);
    find_best_move(&mut board, &mut state, 6, &AtomicBool::new(false))
        .unwrap()
        .to_string()
}

#[test]
fn diagnose_remaining_mates() {
    for (fen, expected) in CASES {
        let default = search(fen, |_| {});
        let no_rfp = search(fen, |s| s.params.rfp_margin = 1_000_000);
        let no_null = search(fen, |s| s.params.null_min_depth = 100);
        let no_iir = search(fen, |s| s.params.iir_min_depth = 100);
        let no_quiet_pruning = search(fen, |s| {
            s.params.futility_margin = 1_000_000;
            s.params.lmp_min_depth = 0;
            s.params.lmr_min_depth = 100;
        });
        let minimal = search(fen, |s| {
            s.params.rfp_margin = 1_000_000;
            s.params.null_min_depth = 100;
            s.params.iir_min_depth = 100;
            s.params.futility_margin = 1_000_000;
            s.params.lmp_min_depth = 0;
            s.params.lmr_min_depth = 100;
        });
        eprintln!(
            "expected={expected} default={default} no_rfp={no_rfp} no_null={no_null} no_iir={no_iir} no_quiet={no_quiet_pruning} minimal={minimal}"
        );
    }
}

#[test]
fn trace_expected_root() {
    let idx = std::env::var("MATE_CASE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let (fen, expected) = CASES[idx];
    let mut board = Board::from_fen(fen);
    let expected = board.parse_move(expected).unwrap();
    let mut state = SearchState::new(16);
    let result = run_search(
        &mut board,
        &mut state,
        SearchConfig::depth(6).with_root_moves(vec![expected]),
        &AtomicBool::new(false),
    );
    eprintln!(
        "restricted result={:?}",
        result.best_move.map(|m| m.to_string())
    );
}

#[test]
fn current_five() {
    for (fen, expected) in CASES {
        let got = search(fen, |_| {});
        eprintln!("expected={expected} got={got}");
    }
}

fn restricted_score(fen: &str, expected: &str, configure: impl FnOnce(&mut SearchState)) -> i32 {
    let mut board = Board::from_fen(fen);
    let expected = board.parse_move(expected).unwrap();
    let mut state = SearchState::new(16);
    configure(&mut state);
    let scores = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&scores);
    let config = SearchConfig::depth(6)
        .with_root_moves(vec![expected])
        .with_info_callback(Arc::new(move |info| {
            received.lock().unwrap().push((info.depth, info.score));
        }));
    let _ = run_search(&mut board, &mut state, config, &AtomicBool::new(false));
    let score = scores.lock().unwrap().last().unwrap().1;
    score
}

#[test]
fn diagnose_e2h5_groups() {
    let (fen, expected) = CASES[2];
    let no_rfp = restricted_score(fen, expected, |s| s.params.rfp_margin = 1_000_000);
    let no_rfp_null_iir = restricted_score(fen, expected, |s| {
        s.params.rfp_margin = 1_000_000;
        s.params.null_min_depth = 100;
        s.params.iir_min_depth = 100;
    });
    let no_rfp_futility_lmp = restricted_score(fen, expected, |s| {
        s.params.rfp_margin = 1_000_000;
        s.params.futility_margin = 1_000_000;
        s.params.lmp_min_depth = 0;
    });
    let no_rfp_lmr = restricted_score(fen, expected, |s| {
        s.params.rfp_margin = 1_000_000;
        s.params.lmr_min_depth = 100;
    });
    eprintln!(
        "e2h5 scores no_rfp={no_rfp} no_rfp+null+iir={no_rfp_null_iir} no_rfp+futility+lmp={no_rfp_futility_lmp} no_rfp+lmr={no_rfp_lmr}"
    );
}

#[test]
fn trace_full_case() {
    let idx = std::env::var("MATE_CASE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2);
    let (fen, expected) = CASES[idx];
    let mode = std::env::var("MATE_MODE").unwrap_or_default();
    let got = search(fen, |s| match mode.as_str() {
        "futility_env" => {
            if let Ok(v) = std::env::var("FUTILITY_MARGIN") {
                s.params.futility_margin = v.parse().unwrap();
            }
        }
        "no_rfp" => s.params.rfp_margin = 1_000_000,
        "no_rfp_quiet" => {
            s.params.rfp_margin = 1_000_000;
            s.params.futility_margin = 1_000_000;
            s.params.lmp_min_depth = 0;
        }
        "no_rfp_futility" => {
            s.params.rfp_margin = 1_000_000;
            s.params.futility_margin = 1_000_000;
        }
        "no_rfp_lmp" => {
            s.params.rfp_margin = 1_000_000;
            s.params.lmp_min_depth = 0;
        }
        "no_rfp_lmr" => {
            s.params.rfp_margin = 1_000_000;
            s.params.lmr_min_depth = 100;
        }
        "no_rfp_null" => {
            s.params.rfp_margin = 1_000_000;
            s.params.null_min_depth = 100;
        }
        "no_rfp_iir" => {
            s.params.rfp_margin = 1_000_000;
            s.params.iir_min_depth = 100;
        }
        _ => {}
    });
    eprintln!("case={idx} expected={expected} got={got}");
}

#[test]
fn diagnose_last_two() {
    for idx in [2usize, 3] {
        let (fen, expected) = CASES[idx];
        let plain = search(fen, |_| {});
        let no_rfp = search(fen, |s| s.params.rfp_margin = 1_000_000);
        let rfp_null = search(fen, |s| {
            s.params.rfp_margin = 1_000_000;
            s.params.null_min_depth = 100;
        });
        let rfp_iir = search(fen, |s| {
            s.params.rfp_margin = 1_000_000;
            s.params.iir_min_depth = 100;
        });
        let rfp_futility = search(fen, |s| {
            s.params.rfp_margin = 1_000_000;
            s.params.futility_margin = 1_000_000;
        });
        let rfp_lmp = search(fen, |s| {
            s.params.rfp_margin = 1_000_000;
            s.params.lmp_min_depth = 0;
        });
        let rfp_lmr = search(fen, |s| {
            s.params.rfp_margin = 1_000_000;
            s.params.lmr_min_depth = 100;
        });
        let min4_null = search(fen, |s| s.params.null_min_depth = 100);
        let min4_iir = search(fen, |s| s.params.iir_min_depth = 100);
        let min4_futility = search(fen, |s| s.params.futility_margin = 1_000_000);
        let min4_lmp = search(fen, |s| s.params.lmp_min_depth = 0);
        let min4_lmr = search(fen, |s| s.params.lmr_min_depth = 100);
        eprintln!(
            "idx={idx} expected={expected} plain={plain} no_rfp={no_rfp} rfp+null={rfp_null} rfp+iir={rfp_iir} rfp+futility={rfp_futility} rfp+lmp={rfp_lmp} rfp+lmr={rfp_lmr} min4+null={min4_null} min4+iir={min4_iir} min4+futility={min4_futility} min4+lmp={min4_lmp} min4+lmr={min4_lmr}"
        );
    }
}
