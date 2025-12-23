use std::io;
use std::io::{BufRead, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use chess_engine::board::{
    find_best_move, find_best_move_with_time, Board, SearchClock, SearchLimits, SearchState,
    DEFAULT_TT_MB,
};
use chess_engine::uci::command::{parse_go_params, parse_uci_command, GoParams, UciCommand};
use chess_engine::uci::options::{parse_setoption, UciOptionAction, UciOptions};
use chess_engine::uci::print::{print_perft_info, print_time_info};
use chess_engine::uci::report::{print_bestmove, print_ready};
use chess_engine::uci::time::compute_time_limits;
use chess_engine::uci::parse_position_command;

struct SearchJob {
    stop: Arc<AtomicBool>,
    clock: Arc<SearchClock>,
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

fn parts_as_strs(parts: &[String]) -> Vec<&str> {
    parts.iter().map(|p| p.as_str()).collect()
}

fn apply_go_params(
    params: &GoParams,
    board: &Board,
    time_left: &mut Duration,
    inc: &mut Duration,
    movetime: &mut Option<Duration>,
) {
    if board.white_to_move() {
        if let Some(wtime) = params.wtime {
            *time_left = Duration::from_millis(wtime);
        }
        if let Some(winc) = params.winc {
            *inc = Duration::from_millis(winc);
        }
    } else {
        if let Some(btime) = params.btime {
            *time_left = Duration::from_millis(btime);
        }
        if let Some(binc) = params.binc {
            *inc = Duration::from_millis(binc);
        }
    }

    if let Some(mt) = params.movetime {
        *movetime = Some(Duration::from_millis(mt));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_command(
    cmd: UciCommand,
    board: &mut Board,
    options: &mut UciOptions,
    search: &Arc<Mutex<SearchState>>,
    current_job: &mut Option<SearchJob>,
    time_left: &mut Duration,
    inc: &mut Duration,
    movetime: &mut Option<Duration>,
    debug: &mut bool,
) -> bool {
    match cmd {
        UciCommand::Uci => {
            if let Ok(guard) = search.lock() {
                options.print(guard.params());
            }
        }
        UciCommand::IsReady => {
            print_ready();
        }
        UciCommand::UciNewGame => {
            stop_search(current_job);
            *board = Board::new();
        }
        UciCommand::Position(parts) => {
            stop_search(current_job);
            let parts_ref = parts_as_strs(&parts);
            parse_position_command(board, &parts_ref);
        }
        UciCommand::Perft(depth) => {
            stop_search(current_job);
            let start = Instant::now();
            let nodes = board.perft(depth);
            let elapsed = start.elapsed();
            print_perft_info(depth, nodes, elapsed);
        }
        UciCommand::Go(parts) => {
            let parts_ref = parts_as_strs(&parts);
            let params = parse_go_params(&parts_ref);

            apply_go_params(&params, board, time_left, inc, movetime);
            let movestogo = params.movestogo;
            let mut depth = params.depth;
            let nodes = params.nodes;
            let mate = params.mate;
            let go_ponder = params.ponder;
            let go_infinite = params.infinite;

            stop_search(current_job);
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
                *time_left,
                *inc,
                *movetime,
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
            let soft_deadline = if go_infinite || go_ponder {
                None
            } else {
                Some(start + Duration::from_millis(soft_time_ms))
            };
            let hard_deadline = if go_infinite || go_ponder {
                None
            } else {
                Some(start + Duration::from_millis(hard_time_ms))
            };
            let clock = Arc::new(SearchClock::new(start, soft_deadline, hard_deadline));
            let pondering = Arc::new(AtomicBool::new(go_ponder));

            if !go_infinite && !go_ponder {
                let stop_timer = Arc::clone(&stop);
                let clock_timer = Arc::clone(&clock);
                thread::spawn(move || {
                    let (_, _, hard_deadline) = clock_timer.snapshot();
                    if let Some(deadline) = hard_deadline {
                        let now = Instant::now();
                        if deadline > now {
                            thread::sleep(deadline - now);
                        }
                        stop_timer.store(true, Ordering::Relaxed);
                    }
                });
            }

            let mut search_board = board.clone();
            let search_clone = Arc::clone(search);
            let stop_clone = Arc::clone(&stop);
            let clock_clone = Arc::clone(&clock);
            let pondering_clone = Arc::clone(&pondering);

            let handle = thread::Builder::new()
                .name("search".to_string())
                .stack_size(32 * 1024 * 1024)
                .spawn(move || {
                    let mut guard = search_clone.lock().unwrap();
                    let best_move = if let Some(d) = depth {
                        find_best_move(&mut search_board, &mut guard, d, &stop_clone)
                    } else {
                        let limits = SearchLimits {
                            clock: clock_clone,
                            stop: stop_clone,
                        };
                        find_best_move_with_time(&mut search_board, &mut guard, limits)
                    };

                    if pondering_clone.load(Ordering::Relaxed) {
                        return;
                    }

                    print_bestmove(best_move);
                })
                .expect("failed to spawn search thread");

            *current_job = Some(SearchJob {
                stop,
                clock,
                pondering,
                planned_soft_time_ms: planned_soft_ms,
                planned_hard_time_ms: planned_hard_ms,
                handle,
            });
        }
        UciCommand::Stop => {
            if let Some(job) = current_job {
                job.stop.store(true, Ordering::Relaxed);
                job.pondering.store(false, Ordering::Relaxed);
            }
        }
        UciCommand::PonderHit => {
            if let Some(job) = current_job {
                if job.pondering.load(Ordering::Relaxed) {
                    let start = Instant::now();
                    job.clock.reset(
                        start,
                        Some(start + Duration::from_millis(job.planned_soft_time_ms)),
                        Some(start + Duration::from_millis(job.planned_hard_time_ms)),
                    );
                    job.pondering.store(false, Ordering::Relaxed);
                }
            }
        }
        UciCommand::SetOption(parts) => {
            stop_search(current_job);
            let parts_ref = parts_as_strs(&parts);
            if let Some((name, value)) = parse_setoption(&parts_ref) {
                if let Ok(mut guard) = search.lock() {
                    if let Some(action) =
                        options.apply_setoption(&name, value.as_deref(), &mut guard)
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
        UciCommand::Debug(value) => {
            *debug = matches!(value.as_deref(), Some("on"));
            if let Ok(mut guard) = search.lock() {
                guard.set_trace(*debug);
            }
        }
        UciCommand::Quit => {
            stop_search(current_job);
            return false;
        }
        UciCommand::Unknown(line) => {
            if *debug {
                eprintln!("Unknown command: {}", line);
            }
        }
    }
    true
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut options = UciOptions::new(DEFAULT_TT_MB);
    let search = Arc::new(Mutex::new(SearchState::new(options.hash_mb)));
    let mut current_job: Option<SearchJob> = None;

    let mut time_left = Duration::from_secs(5); // fallback
    let mut inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;
    let mut debug = false;
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(cmd) = parse_uci_command(&line) {
            let keep_running = handle_command(
                cmd,
                &mut board,
                &mut options,
                &search,
                &mut current_job,
                &mut time_left,
                &mut inc,
                &mut movetime,
                &mut debug,
            );
            if !keep_running {
                break;
            }
        }

        stdout.flush().unwrap();
    }
}
