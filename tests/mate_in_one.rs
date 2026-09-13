use serde::Deserialize;

use chess_engine::board::{Board, Color};
use chess_engine::uci::parse_position_command;

#[derive(Deserialize)]
struct ProblemSet {
    problems: Vec<Problem>,
}

#[derive(Deserialize)]
struct Problem {
    #[serde(rename = "type")]
    kind: String,
    fen: String,
    moves: String,
}

fn uci_from_problem_moves(moves: &str) -> String {
    moves.replace('-', "")
}

fn first_uci_from_line(moves: &str) -> String {
    let first = moves.split(';').next().unwrap_or(moves);
    first.replace('-', "")
}

fn forces_mate_within(board: &mut Board, attacker: Color, plies: u32) -> bool {
    if board.is_checkmate() {
        return board.side_to_move() != attacker;
    }
    if plies == 0 || board.is_stalemate() || board.is_theoretical_draw() {
        return false;
    }

    let moves = board.generate_moves();
    if board.side_to_move() == attacker {
        moves.iter().copied().any(|mv| {
            let mut child = board.clone();
            child.make_move_uci(&mv.to_string()).unwrap();
            forces_mate_within(&mut child, attacker, plies - 1)
        })
    } else {
        moves.iter().copied().all(|mv| {
            let mut child = board.clone();
            child.make_move_uci(&mv.to_string()).unwrap();
            forces_mate_within(&mut child, attacker, plies - 1)
        })
    }
}

fn move_forces_mate_within(board: &mut Board, mv: chess_engine::board::Move, plies: u32) -> bool {
    let attacker = board.side_to_move();
    let mut child = board.clone();
    child.make_move_uci(&mv.to_string()).unwrap();
    forces_mate_within(&mut child, attacker, plies)
}

#[test]
fn mate_in_one_suite() {
    let data = include_str!("data/problems.json");
    let set: ProblemSet = serde_json::from_str(data).expect("invalid problems.json");

    for problem in set.problems.iter().filter(|p| p.kind == "Mate in One") {
        let uci = uci_from_problem_moves(&problem.moves);

        let mut parts: Vec<String> = Vec::new();
        parts.push("position".to_string());
        parts.push("fen".to_string());
        parts.extend(problem.fen.split_whitespace().map(str::to_string));
        parts.push("moves".to_string());
        parts.push(uci);

        let parts_ref: Vec<&str> = parts.iter().map(String::as_str).collect();
        let mut board = Board::new();
        parse_position_command(&mut board, &parts_ref);

        assert!(
            board.is_checkmate(),
            "mate in one failed for fen: {} move: {}",
            problem.fen,
            problem.moves
        );
    }
}

#[test]
#[ignore]
fn mate_search_suite() {
    let data = include_str!("data/problems.json");
    let set: ProblemSet = serde_json::from_str(data).expect("invalid problems.json");
    let mut failures = 0;
    let limit = std::env::var("MATE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok());
    let progress_every = std::env::var("MATE_PROGRESS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);
    let mut checked = 0usize;

    for problem in &set.problems {
        let depth = match problem.kind.as_str() {
            "Mate in One" => 2,
            "Mate in Two" => 4,
            "Mate in Three" => 6,
            _ => continue,
        };

        if let Some(limit) = limit {
            if checked >= limit {
                break;
            }
        }

        let mut board = Board::from_fen(&problem.fen);
        let mut state = chess_engine::board::SearchState::new(16);
        let stop = std::sync::atomic::AtomicBool::new(false);

        let best = chess_engine::board::find_best_move(&mut board, &mut state, depth, &stop);
        let best_uci = best.map(|m| chess_engine::uci::format_uci_move(&m));

        if problem.kind == "Mate in One" {
            if let Some(mv) = best {
                let mv_uci = chess_engine::uci::format_uci_move(&mv);
                let mut parts: Vec<String> = Vec::new();
                parts.push("position".to_string());
                parts.push("fen".to_string());
                parts.extend(problem.fen.split_whitespace().map(str::to_string));
                parts.push("moves".to_string());
                parts.push(mv_uci);

                let parts_ref: Vec<&str> = parts.iter().map(String::as_str).collect();
                let mut board = Board::new();
                parse_position_command(&mut board, &parts_ref);
                if !board.is_checkmate() {
                    failures += 1;
                    eprintln!(
                        "Mismatch: type={} fen={} move={} not checkmate",
                        problem.kind,
                        problem.fen,
                        best_uci.as_deref().unwrap_or("0000")
                    );
                }
            } else {
                failures += 1;
                eprintln!(
                    "Mismatch: type={} fen={} no move",
                    problem.kind, problem.fen
                );
            }
        } else {
            let expected = first_uci_from_line(&problem.moves);
            if best_uci.as_deref() != Some(expected.as_str()) {
                let remaining_plies = depth - 1;
                if best.is_none_or(|mv| !move_forces_mate_within(&mut board, mv, remaining_plies)) {
                    failures += 1;
                    eprintln!(
                        "Mismatch: type={} fen={} expected={} got={:?}",
                        problem.kind, problem.fen, expected, best_uci
                    );
                }
            }
        }

        checked += 1;
        if progress_every > 0 && checked.is_multiple_of(progress_every) {
            eprintln!("checked {}", checked);
        }
    }

    assert_eq!(failures, 0, "mate search mismatches: {}", failures);
}

#[test]
fn short_forced_mates_survive_pruning() {
    const CASES: [&str; 5] = [
        "2b2r1r/8/1BQp3q/8/1k3b2/8/1PP5/K4n2 w - - 0 1",
        "q6n/6Q1/2p2p2/2Pk1P2/3P3p/4PPb1/2PK4/8 w - - 0 1",
        "1b2n3/1rp5/2kPB3/2P1P3/P2P4/8/4Q3/Kn5q w - - 0 1",
        "4RB1n/q7/2p5/8/k1PP2n1/2K5/1P6/8 w - - 0 1",
        "8/8/8/3B4/4b2p/3N1p1p/5K1p/7k w - - 0 1",
    ];

    for fen in CASES {
        let mut board = Board::from_fen(fen);
        let mut state = chess_engine::board::SearchState::new(16);
        let stop = std::sync::atomic::AtomicBool::new(false);
        let best = chess_engine::board::find_best_move(&mut board, &mut state, 6, &stop)
            .expect("mate position must have a legal move");
        assert!(
            move_forces_mate_within(&mut board, best, 5),
            "search missed forced mate for {fen}: {best}"
        );
    }
}
