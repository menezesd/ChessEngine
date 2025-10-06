use chess_engine::{Position};
use chess_engine::chess_search::Search;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn main() {
    // Args: [path] [time_ms] [limit] [hash_mb]
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).map(String::as_str).unwrap_or("1001-brilliant-checkmates.pgn");
    let time_ms: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5000);
    let limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(50);
    let hash_mb: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(256);

    let file = File::open(path).expect("failed to open PGN file");
    let reader = BufReader::new(file);

    let mut puzzles = 0usize;
    let mut solved = 0usize;
    let mut total_elapsed_ms: u128 = 0;

    println!("Running tactics from {} with {} ms per puzzle (limit {}), Hash {} MB", path, time_ms, limit, hash_mb);

    for line in reader.lines() {
        if puzzles >= limit { break; }
        let line = match line { Ok(l) => l, Err(_) => continue };
        // Look for FEN tags: [FEN "..."]
        if let Some(start) = line.find("[FEN \"") {
            if let Some(endq) = line[start+6..].find('\"') {
                let fen = &line[start+6..start+6+endq];
                // Set up position
                if let Ok(mut pos) = Position::from_fen(fen) {
                    let mut search = Search::new();
                    // Larger TT helps for mates
                    search.hash_mb = hash_mb; search.tt.resize(hash_mb);
                    search.multipv = 1;

                    let t0 = Instant::now();
                    let best = search.think(&mut pos, time_ms, 64);
                    let elapsed = t0.elapsed().as_millis();
                    total_elapsed_ms += elapsed;
                    puzzles += 1;

                    let pv_str = if !search.last_pv_line.is_empty() {
                        search.last_pv_line.iter().map(|m| m.to_string()).take(8).collect::<Vec<_>>().join(" ")
                    } else { String::new() };

                    // Strict mate verification: play out PV, check for checkmate
                    let mut mate = false;
                    if best.is_some() && !search.last_pv_line.is_empty() {
                        let mut pv_pos = Position::from_fen(fen).unwrap();
                        let mut legal = pv_pos.legal_moves();
                        let mut mated = false;
                        for mv in &search.last_pv_line {
                            if !legal.contains(mv) { break; }
                            let undo = pv_pos.make_move(mv);
                            legal = pv_pos.legal_moves();
                            if legal.is_empty() {
                                // If side to move is in check, it's mate
                                if pv_pos.is_in_check() { mated = true; }
                                break;
                            }
                        }
                        mate = mated;
                    }

                    if mate { solved += 1; }
                    println!("[#{}] {}ms  solved:{}  best:{:?}  pv:{}", puzzles, elapsed, mate, best, pv_str);
                }
            }
        }
    }

    if puzzles == 0 { println!("No puzzles found."); return; }
    let avg = total_elapsed_ms as f64 / puzzles as f64;
    println!("Solved {}/{}  avg {:.1} ms", solved, puzzles, avg);
}
