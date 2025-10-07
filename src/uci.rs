use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, Instant};

use crate::board::{
    parse_uci_move,
    Board,
};
use crate::engine::{SimpleEngine, SearchOptions, SearchEngine};
use crate::search_control;
use crate::transposition_table::TranspositionTable;
use crate::types::{format_square, Move, Piece};

/// Parse a UCI `position` command into `board`.
///
/// Handles `startpos`, `fen` and trailing `moves ...` sequences and mutates
/// the given `Board` to reach the requested position.
pub fn parse_position_command(board: &mut Board, parts: &[&str]) {
    let mut i = 1;
    if i < parts.len() && parts[i] == "startpos" {
        *board = Board::new();
        i += 1;
    } else if i < parts.len() && parts[i] == "fen" {
        let fen = parts[i + 1..i + 7].join(" ");
        *board = Board::from_fen(&fen);
        i += 7;
    }

    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        while i < parts.len() {
            if let Some(mv) = parse_uci_move(board, parts[i]) {
                board.make_move(&mv);
            } else {
                eprintln!("Invalid move: {}", parts[i]);
            }
            i += 1;
        }
    }
}

/// Format a `Move` as a UCI move string (e.g. "e2e4", with optional promotion).
pub fn format_uci_move(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

/// Run the UCI main loop reading stdin and responding to UCI commands.
///
/// This function blocks and is intended to be the program entrypoint when
/// running the engine via UCI.
pub fn run_uci_loop() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut tt = TranspositionTable::new(1024);

    let mut _time_left = Duration::from_secs(5);
    let mut _inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;
    // Background search state
    let mut search_thread: Option<JoinHandle<()>> = None;
    let mut search_best: Option<Arc<Mutex<Option<Move>>>> = None;
    let mut searching = false;
    let mut pondering = false;
    // UCI info channel (sender -> worker, receiver -> printer thread)
    let (info_tx, info_rx) = crate::uci_info::channel();
    // Spawn printer thread that serializes all info output to stdout
    let _printer_handle = spawn(move || {
        let stdout = std::io::stdout();
        while let Ok(info) = info_rx.recv() {
            let line = info.to_uci_line();
            let mut lock = stdout.lock();
            if let Err(e) = writeln!(lock, "{}", line) {
                eprintln!("UCI printer write error: {}", e);
            }
            if let Err(e) = lock.flush() {
                eprintln!("UCI printer flush error: {}", e);
            }
            // `lock` is dropped here so other threads can lock stdout too
        }
    });

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin for UCI: {}", e);
                continue;
            }
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
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
            "display" | "d" => {
                // Debug helper: print the current board to stdout for the GUI/user.
                board.print();
            }
            "go" => {
                let mut i = 1;
                // If a previous search is running, stop it before starting a new one
                if searching {
                    search_control::set_stop(true);
                    if let Some(handle) = search_thread.take() {
                        let _ = handle.join();
                    }
                    searching = false;
                    pondering = false;
                }
                // Parsed time control fields
                let mut wtime_ms: Option<u64> = None;
                let mut btime_ms: Option<u64> = None;
                let mut winc_ms: u64 = 0;
                let mut binc_ms: u64 = 0;
                let movestogo_opt: Option<u32> = Some(30);
                while i < parts.len() {
                    match parts[i] {
                        "depth" => {
                            // fixed-depth search request (synchronous via engine facade)
                            if let Some(d) = parts.get(i + 1).and_then(|s| s.parse::<u32>().ok()) {
                                let engine = SimpleEngine::new();
                                let opts = SearchOptions {
                                    max_depth: Some(d),
                                    max_time: None,
                                    max_nodes: None,
                                    is_ponder: false,
                                    sink: None,
                                    // Provide the UCI info sender so the search can publish
                                    // intermediate and final `info` messages (pv/nodes/time).
                                    info_sender: Some(info_tx.clone()),
                                    move_ordering: None,
                                };
                                match engine.search(&mut board, &mut tt, opts) {
                                    Ok(res) => {
                                        if let Some(bm) = res.best_move {
                                            println!("bestmove {}", format_uci_move(&bm));
                                        } else {
                                            println!("bestmove 0000");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("search error: {}", e);
                                        println!("bestmove 0000");
                                    }
                                }
                            }
                            i += 2;
                        }
                        "perft" => {
                            // perft request: run perft to the given depth and print nodes/time
                            if let Some(depth) =
                                parts.get(i + 1).and_then(|s| s.parse::<usize>().ok())
                            {
                                let mut b = board.clone();
                                let start = Instant::now();
                                let nodes = b.perft(depth);
                                let dur = start.elapsed();
                                println!("perft {} nodes in {:?}", nodes, dur);
                            }
                            i += 2;
                        }
                        "wtime" => {
                            wtime_ms = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok());
                            i += 2;
                        }
                        "btime" => {
                            btime_ms = parts.get(i + 1).and_then(|s| s.parse::<u64>().ok());
                            i += 2;
                        }
                        "winc" => {
                            winc_ms = parts
                                .get(i + 1)
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(0);
                            i += 2;
                        }
                        "binc" => {
                            binc_ms = parts
                                .get(i + 1)
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(0);
                            i += 2;
                        }
                        "movetime" => {
                            movetime =
                                Some(Duration::from_millis(parts[i + 1].parse().unwrap_or(100)));
                            i += 2;
                        }
                        "nodes" => {
                            // handled below by setting node_limit before spawning worker
                            i += 2;
                        }
                        "infinite" => {
                            // start an infinite (until stop) search
                            i += 1;
                        }
                        "ponder" => {
                            pondering = true;
                            i += 1;
                        }
                        "mate" => {
                            // searched as depth limit (mate in N)
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }

                // Spawn a background worker depending on requested options
                // Clone board and alloc fresh TT for background search
                let board_clone = board.clone();
                let tt_clone = TranspositionTable::new(1024);
                let bm = Arc::new(Mutex::new(None::<Move>));
                let bm_thread = bm.clone();
                search_best = Some(bm);
                search_control::reset();

                // Check if nodes= was provided
                if let Some(pos) = parts.iter().position(|&s| s == "nodes") {
                    if let Some(nstr) = parts.get(pos + 1) {
                        if let Ok(n) = nstr.parse::<u64>() {
                            search_control::set_node_limit(n);
                        }
                    }
                }

                // Determine mode: depth, movetime, nodes, infinite, mate, ponder
                // Build a thread and spawn
                // If movetime wasn't specified but wtime/btime were, compute an allocation
                let mut computed_movetime = movetime;
                if computed_movetime.is_none() {
                    // pick side's remaining time and increment
                    let (time_ms, inc) = if board.white_to_move {
                        (wtime_ms, winc_ms)
                    } else {
                        (btime_ms, binc_ms)
                    };
                    if let Some(tms) = time_ms {
                        let moves_to_go = match movestogo_opt {
                            Some(v) => v as u64,
                            None => 30u64,
                        };
                                                                              // simple allocation: divide remaining time by moves_to_go, minus safety
                        let mut alloc = tms / moves_to_go;
                        if alloc > 50 {
                            alloc = alloc.saturating_sub(50);
                        }
                        // add a fraction of increment
                        alloc = alloc.saturating_add(inc / 4);
                        if alloc == 0 {
                            alloc = 1;
                        }
                        computed_movetime = Some(Duration::from_millis(alloc));
                    }
                }
                let use_movetime = computed_movetime;
                let mut use_depth: Option<u32> = None;
                if let Some(pos) = parts.iter().position(|&s| s == "depth") {
                    if let Some(dstr) = parts.get(pos + 1) {
                        if let Ok(d) = dstr.parse::<u32>() {
                            use_depth = Some(d);
                        }
                    }
                }
                let tx = info_tx.clone();
                let is_ponder = pondering;
                let handle = std::thread::spawn(move || {
                    // Worker thread: perform search according to mode and publish intermediate best moves
                    let engine = SimpleEngine::new();
                    let mut local_board = board_clone;
                    let mut local_tt = tt_clone;
                    let opts = if let Some(d) = use_depth {
                        SearchOptions {
                            max_depth: Some(d),
                            max_time: None,
                            max_nodes: None,
                            is_ponder,
                            sink: Some(bm_thread.clone()),
                            info_sender: Some(tx.clone()),
                            move_ordering: None,
                        }
                    } else if let Some(t) = use_movetime {
                        SearchOptions {
                            max_depth: None,
                            max_time: Some(t),
                            max_nodes: None,
                            is_ponder,
                            sink: Some(bm_thread.clone()),
                            info_sender: Some(tx.clone()),
                            move_ordering: None,
                        }
                    } else {
                        // nodes / infinite / ponder: iterative deepening until stop flag
                        SearchOptions {
                            max_depth: Some(64),
                            max_time: None,
                            max_nodes: None,
                            is_ponder,
                            sink: Some(bm_thread.clone()),
                            info_sender: Some(tx.clone()),
                            move_ordering: None,
                        }
                    };

                    match engine.search(&mut local_board, &mut local_tt, opts) {
                        Ok(res) => {
                            if let Some(bm) = res.best_move {
                                println!("bestmove {}", format_uci_move(&bm));
                            } else {
                                println!("bestmove 0000");
                            }
                        }
                        Err(e) => {
                            eprintln!("search error: {}", e);
                            println!("bestmove 0000");
                        }
                    }
                });

                search_thread = Some(handle);
            }
            "stop" => {
                // Signal stop and join worker, then print bestmove
                search_control::set_stop(true);
                if let Some(handle) = search_thread.take() {
                    let _ = handle.join();
                }
                searching = false;
                // print best move if available
                if let Some(bm_arc) = &search_best {
                    let guard = match bm_arc.lock() {
                        Ok(g) => g,
                        Err(poisoned) => {
                            eprintln!("warning: bestmove mutex poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    if let Some(best) = *guard {
                        println!("bestmove {}", format_uci_move(&best));
                    } else {
                        println!("bestmove 0000");
                    }
                } else {
                    println!("bestmove 0000");
                }
            }
            "ponderhit" => {
                // If we are currently pondering and a worker exists, convert the ponder search
                // into the main search: clear the stop flag and mark pondering false so the
                // background worker continues as a normal search (the search routines are
                // cooperative and will keep running until node/time/depth limits or stop).
                if pondering {
                    search_control::set_stop(false);
                    pondering = false;
                    // Keep searching; do not join the worker here. The GUI will later send
                    // 'stop' or a new 'go' when appropriate.
                } else {
                    // Fallback: just clear stop so any paused search can resume
                    search_control::set_stop(false);
                }
            }
            "quit" => break,
            _ => {}
        }

        stdout.flush().unwrap();
    }
}
