use std::io;

use crate::board::Board;
use crate::search::{find_best_move_at_depth, find_best_move_timed};
use crate::types::TranspositionTable;
use crate::utils::{move_to_uci, parse_uci_move};

/// Main UCI protocol loop
pub fn run_uci_loop() {
    let stdin = io::stdin();
    let mut board = Board::new();
    let mut tt_size_mb = 64; // Default TT size in MB
    let mut tt = TranspositionTable::new(tt_size_mb);

    println!("Chess Engine - Bitboard Version");
    println!("Type 'uci' to start UCI mode");

    for line in stdin.lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "uci" => {
                println!("id name ChessEngine Bitboard");
                println!("id author Dean Menezes");
                println!("option name Hash type spin default 64 min 1 max 1024");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "setoption" => {
                if parts.len() >= 5 && parts[1] == "name" && parts[2] == "Hash" && parts[3] == "value" {
                    if let Ok(size) = parts[4].parse::<usize>() {
                        if size >= 1 && size <= 1024 {
                            tt_size_mb = size;
                            tt = TranspositionTable::new(tt_size_mb);
                            println!("info string Hash set to {} MB", tt_size_mb);
                        }
                    }
                }
            }
            "ucinewgame" => {
                tt.clear();
                board = Board::new();
            }
            "position" => {
                if parts.len() > 1 {
                    if parts[1] == "startpos" {
                        board = Board::new();
                    } else if parts[1] == "fen" && parts.len() > 2 {
                        let fen = parts[2..].join(" ");
                        board = Board::from_fen(&fen);
                    }
                    // Handle moves
                    if let Some(move_idx) = parts.iter().position(|&x| x == "moves") {
                        for move_str in &parts[move_idx + 1..] {
                            if let Some(m) = parse_uci_move(&board, move_str) {
                                let _info = board.make_move(&m);
                            }
                        }
                    }
                }
            }
            "go" => {
                let mut wtime = 0;
                let mut btime = 0;
                let mut movestogo = 40; // Default for tournament time controls
                let mut movetime = 0; // Exact time for this move in ms
                let mut depth = 0; // Maximum search depth

                // Parse time controls
                let mut i = 1;
                while i < parts.len() {
                    match parts[i] {
                        "wtime" => {
                            if i + 1 < parts.len() {
                                wtime = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "btime" => {
                            if i + 1 < parts.len() {
                                btime = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "movestogo" => {
                            if i + 1 < parts.len() {
                                movestogo = parts[i + 1].parse().unwrap_or(40);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "movetime" => {
                            if i + 1 < parts.len() {
                                movetime = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "depth" => {
                            if i + 1 < parts.len() {
                                depth = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        _ => i += 1,
                    }
                }

                // Determine search mode and parameters
                let best_move = if depth > 0 {
                    // Fixed depth search
                    let (move_result, _score) = find_best_move_at_depth(&mut board, &mut tt, depth);
                    if let Some(_m) = move_result {
                        // Output info line with score and PV
                        let pv = crate::search::extract_pv(&mut board, &mut tt, depth);
                        let pv_string = pv.iter().map(|mv| move_to_uci(mv)).collect::<Vec<String>>().join(" ");
                        println!("info depth {} score cp {} pv {}", depth, _score, pv_string);
                    }
                    move_result
                } else if movetime > 0 {
                    // Fixed time search
                    find_best_move_timed(&mut board, &mut tt, movetime)
                } else {
                    // Time-controlled search
                    let time_for_move = if board.white_to_move {
                        if wtime > 0 {
                            wtime / movestogo.max(1)
                        } else {
                            1000 // Default 1 second
                        }
                    } else {
                        if btime > 0 {
                            btime / movestogo.max(1)
                        } else {
                            1000 // Default 1 second
                        }
                    };
                    find_best_move_timed(&mut board, &mut tt, time_for_move)
                };

                if let Some(m) = best_move {
                    println!("bestmove {}", move_to_uci(&m));
                } else {
                    println!("bestmove 0000");
                }
            }
            "perft" => {
                if parts.len() > 1 {
                    if let Ok(depth) = parts[1].parse::<u32>() {
                        use std::time::Instant;
                        let start = Instant::now();
                        let nodes = crate::perft::perft(&mut board, depth);
                        let elapsed = start.elapsed();
                        println!("Nodes: {}", nodes);
                        println!("Time: {:.3}s", elapsed.as_secs_f64());
                        println!("NPS: {:.0}", nodes as f64 / elapsed.as_secs_f64());
                    }
                }
            }
            "perftdivide" => {
                if parts.len() > 1 {
                    if let Ok(depth) = parts[1].parse::<u32>() {
                        crate::perft::perft_divide(&mut board, depth);
                    }
                }
            }
            "quit" => break,
            "d" => {
                board.print();
            }
            _ => {}
        }
    }
}
