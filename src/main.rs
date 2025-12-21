use std::io;
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

use chess_engine::board::{find_best_move_with_time, Board, SearchLimits, SearchState};
use chess_engine::uci::{format_uci_move, parse_position_command};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut search = SearchState::new(1024); // 1024MB TT

    let mut time_left = Duration::from_secs(5); // fallback
    let mut inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "uci" => {
                println!("id name MyRustEngine");
                println!("id author Dean Menezes");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "ucinewgame" => {
                board = Board::new();
            }
            "position" => {
                parse_position_command(&mut board, &parts);
            }
            "go" => {
                let mut i = 1;
                while i < parts.len() {
                    match parts[i] {
                        "wtime" if board.white_to_move() => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "btime" if !board.white_to_move() => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "winc" if board.white_to_move() => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "binc" if !board.white_to_move() => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "movetime" => {
                            movetime =
                                Some(Duration::from_millis(parts[i + 1].parse().unwrap_or(100)));
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }

                let max_time = movetime.unwrap_or_else(|| time_left / 30 + inc);
                let start = Instant::now();
                let limits = SearchLimits {
                    max_time,
                    start_time: start,
                };

                if let Some(best_move) = find_best_move_with_time(&mut board, &mut search, limits) {
                    let uci_move = format_uci_move(&best_move);
                    println!("bestmove {}", uci_move);
                } else {
                    println!("bestmove 0000");
                }
            }
            "quit" => break,
            _ => {
                // Ignore unknown commands or log them if needed
            }
        }

        stdout.flush().unwrap();
    }
}
