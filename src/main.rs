use std::io;
use std::io::{BufRead, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use chess_engine::board::{
    find_best_move, find_best_move_with_time, Board, SearchLimits, SearchState,
};
use chess_engine::uci::options::{parse_setoption, UciOptionAction, UciOptions};
use chess_engine::uci::print::{print_perft_info, print_time_info};
use chess_engine::uci::{format_uci_move, parse_position_command};

const DEFAULT_MOVES_TO_GO: u64 = 30;

fn compute_time_limits(
    time_left: Duration,
    inc: Duration,
    movetime: Option<Duration>,
    movestogo: Option<u64>,
    move_overhead_ms: u64,
    soft_time_percent: u64,
    hard_time_percent: u64,
) -> (u64, u64) {
    let time_left_ms = time_left.as_millis() as u64;
    let inc_ms = inc.as_millis() as u64;
    let safe_ms = time_left_ms.saturating_sub(move_overhead_ms);

    if let Some(mt) = movetime {
        let mt_ms = mt.as_millis() as u64;
        let capped = if safe_ms > 0 { mt_ms.min(safe_ms) } else { mt_ms };
        let capped = capped.max(1);
        return (capped, capped);
    }

    if time_left_ms <= move_overhead_ms.saturating_add(20) {
        let fallback = time_left_ms / 2;
        let fallback = fallback.max(1);
        return (fallback, fallback);
    }

    let moves_to_go = movestogo.unwrap_or(DEFAULT_MOVES_TO_GO).max(1);
    let soft_ms = safe_ms / moves_to_go + inc_ms;
    let soft_cap = safe_ms * soft_time_percent / 100;
    let hard_cap = safe_ms * hard_time_percent / 100;
    let soft_ms = soft_ms.min(soft_cap).max(1);
    let hard_ms = hard_cap.max(soft_ms).max(1);
    (soft_ms, hard_ms)
}

struct SearchJob {
    stop: Arc<AtomicBool>,
    start_time: Arc<Mutex<Instant>>,
    soft_deadline: Arc<Mutex<Option<Instant>>>,
    hard_deadline: Arc<Mutex<Option<Instant>>>,
    pondering: Arc<AtomicBool>,
    planned_soft_time_ms: u64,
    planned_hard_time_ms: u64,
    handle: thread::JoinHandle<()>,
}

fn stop_search(job: &mut Option<SearchJob>) {
    if let Some(job) = job.take() {
        job.stop.store(true, Ordering::Relaxed);
        let _ = job.handle.join();
    }
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut options = UciOptions::new(1024);
    let search = Arc::new(Mutex::new(SearchState::new(options.hash_mb)));
    let mut current_job: Option<SearchJob> = None;

    let mut time_left = Duration::from_secs(5); // fallback
    let mut inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;
    let mut debug = false;
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "uci" => {
                if let Ok(guard) = search.lock() {
                    options.print(guard.params());
                }
            }
            "isready" => {
                println!("readyok");
            }
            "ucinewgame" => {
                stop_search(&mut current_job);
                board = Board::new();
            }
            "position" => {
                stop_search(&mut current_job);
                parse_position_command(&mut board, &parts);
            }
            "perft" => {
                stop_search(&mut current_job);
                let depth = parts.get(1).and_then(|d| d.parse::<usize>().ok()).unwrap_or(1);
                let start = Instant::now();
                let nodes = board.perft(depth);
                let elapsed = start.elapsed();
                print_perft_info(depth, nodes, elapsed);
            }
            "go" => {
                let mut i = 1;
                let mut movestogo: Option<u64> = None;
                let mut depth: Option<u32> = None;
                let mut nodes: Option<u64> = None;
                let mut mate: Option<u32> = None;
                let mut go_ponder = false;
                let mut go_infinite = false;
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
                        "movestogo" => {
                            movestogo = Some(parts[i + 1].parse().unwrap_or(DEFAULT_MOVES_TO_GO));
                            i += 2;
                        }
                        "depth" => {
                            depth = Some(parts[i + 1].parse().unwrap_or(1));
                            i += 2;
                        }
                        "nodes" => {
                            nodes = Some(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "mate" => {
                            mate = Some(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "ponder" => {
                            go_ponder = true;
                            i += 1;
                        }
                        "infinite" => {
                            go_infinite = true;
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }

                stop_search(&mut current_job);
                {
                    let mut guard = search.lock().unwrap();
                    guard.new_search();
                    let max_nodes = nodes.unwrap_or(options.default_max_nodes);
                    guard.set_max_nodes(max_nodes);
                }

                if let Some(mate_moves) = mate {
                    if mate_moves > 0 && depth.is_none() {
                        depth = Some(mate_moves * 2);
                    }
                }

                let (planned_soft_ms, planned_hard_ms) = compute_time_limits(
                    time_left,
                    inc,
                    movetime,
                    movestogo,
                    options.move_overhead_ms,
                    options.soft_time_percent,
                    options.hard_time_percent,
                );
                let (soft_time_ms, hard_time_ms) = if go_infinite || go_ponder {
                    (u64::MAX, u64::MAX)
                } else {
                    (planned_soft_ms, planned_hard_ms)
                };

                print_time_info(
                    soft_time_ms,
                    hard_time_ms,
                    options.move_overhead_ms,
                    nodes.unwrap_or(options.default_max_nodes),
                    go_ponder,
                    depth.unwrap_or(0),
                );

                let stop = Arc::new(AtomicBool::new(false));
                let start = Instant::now();
                let start_time = Arc::new(Mutex::new(start));
                let soft_deadline = Arc::new(Mutex::new(if go_infinite || go_ponder {
                    None
                } else {
                    Some(start + Duration::from_millis(soft_time_ms))
                }));
                let hard_deadline = Arc::new(Mutex::new(if go_infinite || go_ponder {
                    None
                } else {
                    Some(start + Duration::from_millis(hard_time_ms))
                }));
                let pondering = Arc::new(AtomicBool::new(go_ponder));

                if !go_infinite && !go_ponder {
                    let stop_timer = Arc::clone(&stop);
                    let hard_deadline_timer = Arc::clone(&hard_deadline);
                    thread::spawn(move || {
                        let deadline = *hard_deadline_timer.lock().unwrap();
                        if let Some(deadline) = deadline {
                            let now = Instant::now();
                            if deadline > now {
                                thread::sleep(deadline - now);
                            }
                            stop_timer.store(true, Ordering::Relaxed);
                        }
                    });
                }

                let mut search_board = board.clone();
                let search_clone = Arc::clone(&search);
                let stop_clone = Arc::clone(&stop);
                let start_time_clone = Arc::clone(&start_time);
                let soft_deadline_clone = Arc::clone(&soft_deadline);
                let hard_deadline_clone = Arc::clone(&hard_deadline);
                let pondering_clone = Arc::clone(&pondering);

                let handle = thread::Builder::new()
                    .name("search".to_string())
                    .stack_size(32 * 1024 * 1024)
                    .spawn(move || {
                    let mut guard = search_clone.lock().unwrap();
                    let best_move = if let Some(d) = depth {
                        find_best_move(&mut search_board, &mut *guard, d, &stop_clone)
                    } else {
                        let limits = SearchLimits {
                            start_time: start_time_clone,
                            soft_deadline: soft_deadline_clone,
                            hard_deadline: hard_deadline_clone,
                            stop: stop_clone,
                        };
                        find_best_move_with_time(&mut search_board, &mut *guard, limits)
                    };

                    if pondering_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    if let Some(best_move) = best_move {
                        let uci_move = format_uci_move(&best_move);
                        println!("bestmove {}", uci_move);
                    } else {
                        println!("bestmove 0000");
                    }
                })
                .expect("failed to spawn search thread");

                current_job = Some(SearchJob {
                    stop,
                    start_time,
                    soft_deadline,
                    hard_deadline,
                    pondering,
                    planned_soft_time_ms: planned_soft_ms,
                    planned_hard_time_ms: planned_hard_ms,
                    handle,
                });
            }
            "stop" => {
                if let Some(job) = &current_job {
                    job.stop.store(true, Ordering::Relaxed);
                    job.pondering.store(false, Ordering::Relaxed);
                }
            }
            "ponderhit" => {
                if let Some(job) = &current_job {
                    if job.pondering.load(Ordering::Relaxed) {
                        if let Ok(mut start) = job.start_time.lock() {
                            *start = Instant::now();
                            if let Ok(mut soft_deadline) = job.soft_deadline.lock() {
                                *soft_deadline = Some(
                                    *start + Duration::from_millis(job.planned_soft_time_ms),
                                );
                            }
                            if let Ok(mut hard_deadline) = job.hard_deadline.lock() {
                                *hard_deadline = Some(
                                    *start + Duration::from_millis(job.planned_hard_time_ms),
                                );
                            }
                        }
                        job.pondering.store(false, Ordering::Relaxed);
                    }
                }
            }
            "setoption" => {
                stop_search(&mut current_job);
                if let Some((name, value)) = parse_setoption(&parts) {
                    if let Ok(mut guard) = search.lock() {
                        if let Some(action) =
                            options.apply_setoption(&name, value.as_deref(), &mut *guard)
                        {
                            match action {
                                UciOptionAction::ReinitHash(new_mb) => {
                                    guard.reset_tables(new_mb);
                                }
                            }
                        }
                    }
                }
            }
            "debug" => {
                debug = parts.get(1).copied() == Some("on");
                if let Ok(mut guard) = search.lock() {
                    guard.set_trace(debug);
                }
            }
            "quit" => {
                stop_search(&mut current_job);
                break;
            }
            _ => {
                if debug {
                    eprintln!("Unknown command: {}", line);
                }
            }
        }

        stdout.flush().unwrap();
    }
}
