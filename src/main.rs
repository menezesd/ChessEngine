use std::io;
use std::io::{BufRead, Write};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use chess_engine::board::{
    find_best_move, find_best_move_with_time, Board, SearchLimits, SearchState,
};
use chess_engine::uci::{format_uci_move, parse_position_command};

const DEFAULT_MOVES_TO_GO: u64 = 30;
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 50;
const DEFAULT_SOFT_TIME_PERCENT: u64 = 80;
const DEFAULT_HARD_TIME_PERCENT: u64 = 95;

fn parse_setoption(parts: &[&str]) -> Option<(String, Option<String>)> {
    let name_idx = parts.iter().position(|p| *p == "name")?;
    let value_idx = parts.iter().position(|p| *p == "value");
    let name = match value_idx {
        Some(v_idx) if v_idx > name_idx + 1 => parts[name_idx + 1..v_idx].join(" "),
        None if name_idx + 1 < parts.len() => parts[name_idx + 1..].join(" "),
        _ => return None,
    };
    let value = value_idx.and_then(|v_idx| {
        if v_idx + 1 < parts.len() {
            Some(parts[v_idx + 1..].join(" "))
        } else {
            None
        }
    });
    Some((name, value))
}

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

fn format_time_setting(value_ms: u64) -> String {
    if value_ms == u64::MAX {
        "inf".to_string()
    } else {
        value_ms.to_string()
    }
}

struct SearchJob {
    stop: Arc<AtomicBool>,
    soft_time_ms: Arc<AtomicU64>,
    hard_time_ms: Arc<AtomicU64>,
    start_time: Arc<Mutex<Instant>>,
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
    let mut tt_mb = 1024;
    let search = Arc::new(Mutex::new(SearchState::new(tt_mb)));
    let mut current_job: Option<SearchJob> = None;
    let mut move_overhead_ms = DEFAULT_MOVE_OVERHEAD_MS;
    let mut soft_time_percent = DEFAULT_SOFT_TIME_PERCENT;
    let mut hard_time_percent = DEFAULT_HARD_TIME_PERCENT;
    let mut default_max_nodes: u64 = 0;
    let mut ponder = false;
    let mut multipv: u32 = 1;

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
                println!("id name MyRustEngine");
                println!("id author Dean Menezes");
                println!("option name Hash type spin default {} min 1 max 32768", tt_mb);
                println!("option name Clear Hash type button");
                println!(
                    "option name Move Overhead type spin default {} min 0 max 500",
                    move_overhead_ms
                );
                println!(
                    "option name SoftTime type spin default {} min 10 max 100",
                    soft_time_percent
                );
                println!(
                    "option name HardTime type spin default {} min 10 max 100",
                    hard_time_percent
                );
                println!(
                    "option name Nodes type spin default {} min 0 max 1000000000",
                    default_max_nodes
                );
                println!(
                    "option name MultiPV type spin default {} min 1 max 4",
                    multipv
                );
                println!(
                    "option name Ponder type check default {}",
                    if ponder { "true" } else { "false" }
                );
                println!("uciok");
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
                println!("info string perft depth {} nodes {} time {:?}", depth, nodes, elapsed);
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
                    let max_nodes = nodes.unwrap_or(default_max_nodes);
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
                    move_overhead_ms,
                    soft_time_percent,
                    hard_time_percent,
                );
                let (soft_time_ms, hard_time_ms) = if go_infinite || go_ponder {
                    (u64::MAX, u64::MAX)
                } else {
                    (planned_soft_ms, planned_hard_ms)
                };

                println!(
                    "info string time soft={} hard={} overhead={} nodes={} ponder={} depth={}",
                    format_time_setting(soft_time_ms),
                    format_time_setting(hard_time_ms),
                    move_overhead_ms,
                    nodes.unwrap_or(default_max_nodes),
                    go_ponder,
                    depth.unwrap_or(0)
                );

                let stop = Arc::new(AtomicBool::new(false));
                let soft_time_ms = Arc::new(AtomicU64::new(soft_time_ms));
                let hard_time_ms = Arc::new(AtomicU64::new(hard_time_ms));
                let start_time = Arc::new(Mutex::new(Instant::now()));
                let pondering = Arc::new(AtomicBool::new(go_ponder));

                let mut search_board = board.clone();
                let search_clone = Arc::clone(&search);
                let stop_clone = Arc::clone(&stop);
                let soft_time_ms_clone = Arc::clone(&soft_time_ms);
                let hard_time_ms_clone = Arc::clone(&hard_time_ms);
                let start_time_clone = Arc::clone(&start_time);
                let pondering_clone = Arc::clone(&pondering);

                let handle = thread::spawn(move || {
                    let mut guard = search_clone.lock().unwrap();
                    let best_move = if let Some(d) = depth {
                        find_best_move(&mut search_board, &mut *guard, d, &stop_clone)
                    } else {
                        let limits = SearchLimits {
                            soft_time_ms: soft_time_ms_clone,
                            hard_time_ms: hard_time_ms_clone,
                            start_time: start_time_clone,
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
                });

                current_job = Some(SearchJob {
                    stop,
                    soft_time_ms,
                    hard_time_ms,
                    start_time,
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
                        job.soft_time_ms
                            .store(job.planned_soft_time_ms, Ordering::Relaxed);
                        job.hard_time_ms
                            .store(job.planned_hard_time_ms, Ordering::Relaxed);
                        if let Ok(mut start) = job.start_time.lock() {
                            *start = Instant::now();
                        }
                        job.pondering.store(false, Ordering::Relaxed);
                    }
                }
            }
            "setoption" => {
                stop_search(&mut current_job);
                if let Some((name, value)) = parse_setoption(&parts) {
                    match name.as_str() {
                        "Hash" => {
                            if let Some(v) = value.and_then(|v| v.parse::<usize>().ok()) {
                                if v > 0 {
                                    tt_mb = v;
                                    if let Ok(mut guard) = search.lock() {
                                        *guard = SearchState::new(tt_mb);
                                    }
                                }
                            }
                        }
                        "Clear Hash" => {
                            if let Ok(mut guard) = search.lock() {
                                *guard = SearchState::new(tt_mb);
                            }
                        }
                        "Move Overhead" => {
                            if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                                move_overhead_ms = v;
                            }
                        }
                        "SoftTime" => {
                            if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                                soft_time_percent = v.clamp(10, 100);
                            }
                        }
                        "HardTime" => {
                            if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                                hard_time_percent = v.clamp(10, 100);
                            }
                        }
                        "Nodes" => {
                            if let Some(v) = value.and_then(|v| v.parse::<u64>().ok()) {
                                default_max_nodes = v;
                            }
                        }
                        "Ponder" => {
                            if let Some(v) = value {
                                ponder = v == "true";
                            }
                        }
                        "MultiPV" => {
                            if let Some(v) = value.and_then(|v| v.parse::<u32>().ok()) {
                                multipv = v.max(1);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "debug" => {
                debug = parts.get(1).copied() == Some("on");
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
