use std::io;
use std::io::{BufRead, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

/// Search thread stack size (32 MB)
const SEARCH_STACK_SIZE: usize = 32 * 1024 * 1024;
/// Default depth limit when searching by nodes
const NODE_SEARCH_DEFAULT_DEPTH: u32 = 64;
/// Fallback time allocation when no time control is specified
const FALLBACK_TIME_SECS: u64 = 5;

use chess_engine::board::{
    find_best_move_with_ponder, find_best_move_with_time_and_ponder, Board, SearchClock,
    SearchLimits, SearchResult, SearchState, DEFAULT_TT_MB,
};
use chess_engine::uci::command::{parse_go_params, parse_uci_command, GoParams, UciCommand};
use chess_engine::uci::options::{parse_setoption, UciOptionAction, UciOptions};
use chess_engine::uci::parse_position_command;
use chess_engine::uci::print::{print_perft_info, print_time_info};
use chess_engine::uci::report::{print_bestmove_with_ponder, print_ready};
use chess_engine::uci::time::compute_time_limits;

/// Active search job state
struct SearchJob {
    stop: Arc<AtomicBool>,
    clock: Arc<SearchClock>,
    pondering: Arc<AtomicBool>,
    planned_soft_time_ms: u64,
    planned_hard_time_ms: u64,
    handle: thread::JoinHandle<()>,
}

/// UCI session state (time controls, debug mode)
struct UciState {
    time_left: Duration,
    inc: Duration,
    movetime: Option<Duration>,
    debug: bool,
}

impl Default for UciState {
    fn default() -> Self {
        UciState {
            time_left: Duration::from_secs(FALLBACK_TIME_SECS),
            inc: Duration::ZERO,
            movetime: None,
            debug: false,
        }
    }
}

impl UciState {
    fn apply_go_params(&mut self, params: &GoParams, is_white: bool) {
        if is_white {
            if let Some(wtime) = params.wtime {
                self.time_left = Duration::from_millis(wtime);
            }
            if let Some(winc) = params.winc {
                self.inc = Duration::from_millis(winc);
            }
        } else {
            if let Some(btime) = params.btime {
                self.time_left = Duration::from_millis(btime);
            }
            if let Some(binc) = params.binc {
                self.inc = Duration::from_millis(binc);
            }
        }

        if let Some(mt) = params.movetime {
            self.movetime = Some(Duration::from_millis(mt));
        }
    }
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

/// Handle the "go" command - start a search
fn handle_go(
    parts: &[String],
    board: &Board,
    options: &UciOptions,
    search: &Arc<Mutex<SearchState>>,
    uci_state: &mut UciState,
    current_job: &mut Option<SearchJob>,
) {
    let parts_ref = parts_as_strs(parts);
    let params = parse_go_params(&parts_ref);

    uci_state.apply_go_params(&params, board.white_to_move());
    let movestogo = params.movestogo;
    let mut depth = params.depth;
    let nodes = params.nodes;
    let mate = params.mate;
    let go_ponder = params.ponder;
    let go_infinite = params.infinite;

    if nodes.is_some() && depth.is_none() {
        depth = Some(NODE_SEARCH_DEFAULT_DEPTH);
    }

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
        uci_state.time_left,
        uci_state.inc,
        uci_state.movetime,
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

    // Spawn timer thread for hard deadline
    if !go_infinite && !go_ponder && depth.is_none() {
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

    // Spawn search thread
    let mut search_board = board.clone();
    let search_clone = Arc::clone(search);
    let stop_clone = Arc::clone(&stop);
    let clock_clone = Arc::clone(&clock);
    let pondering_clone = Arc::clone(&pondering);

    let handle = thread::Builder::new()
        .name("search".to_string())
        .stack_size(SEARCH_STACK_SIZE)
        .spawn(move || {
            let mut guard = search_clone.lock().unwrap();
            let result: SearchResult = if let Some(d) = depth {
                find_best_move_with_ponder(&mut search_board, &mut guard, d, &stop_clone)
            } else {
                let limits = SearchLimits {
                    clock: clock_clone.clone(),
                    stop: stop_clone.clone(),
                };
                find_best_move_with_time_and_ponder(&mut search_board, &mut guard, &limits)
            };

            // Wait while pondering (unless stopped)
            // This handles the case where search completes before ponderhit
            while pondering_clone.load(Ordering::Relaxed) && !stop_clone.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
            }

            if result.best_move.is_none() {
                if search_board.is_checkmate() {
                    println!("info score mate -1");
                } else if search_board.is_stalemate() || search_board.is_draw() {
                    println!("info score cp 0");
                }
            }

            print_bestmove_with_ponder(result);
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

/// Handle the "setoption" command
fn handle_setoption(
    parts: &[String],
    options: &mut UciOptions,
    search: &Arc<Mutex<SearchState>>,
    current_job: &mut Option<SearchJob>,
) {
    stop_search(current_job);
    let parts_ref = parts_as_strs(parts);
    if let Some((name, value)) = parse_setoption(&parts_ref) {
        if let Ok(mut guard) = search.lock() {
            if let Some(action) = options.apply_setoption(&name, value.as_deref(), &mut guard) {
                match action {
                    UciOptionAction::ReinitHash(new_mb) => {
                        guard.reset_tables(new_mb);
                    }
                }
            }
        }
    }
}

/// Handle "stop" command
fn handle_stop(current_job: &mut Option<SearchJob>) {
    if let Some(job) = current_job {
        job.stop.store(true, Ordering::Relaxed);
        job.pondering.store(false, Ordering::Relaxed);
    }
}

/// Handle "ponderhit" command
fn handle_ponderhit(current_job: &mut Option<SearchJob>) {
    if let Some(job) = current_job {
        if job.pondering.load(Ordering::Relaxed) {
            let start = Instant::now();
            let hard_deadline = start + Duration::from_millis(job.planned_hard_time_ms);
            job.clock.reset(
                start,
                Some(start + Duration::from_millis(job.planned_soft_time_ms)),
                Some(hard_deadline),
            );

            // Spawn timer thread to enforce hard deadline
            let stop_timer = Arc::clone(&job.stop);
            thread::spawn(move || {
                let now = Instant::now();
                if hard_deadline > now {
                    thread::sleep(hard_deadline - now);
                }
                stop_timer.store(true, Ordering::Relaxed);
            });

            job.pondering.store(false, Ordering::Relaxed);
        }
    }
}

/// Process a single UCI command. Returns false if the engine should quit.
fn handle_command(
    cmd: UciCommand,
    board: &mut Board,
    options: &mut UciOptions,
    search: &Arc<Mutex<SearchState>>,
    current_job: &mut Option<SearchJob>,
    uci_state: &mut UciState,
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
            handle_go(&parts, board, options, search, uci_state, current_job);
        }
        UciCommand::Stop => {
            handle_stop(current_job);
        }
        UciCommand::PonderHit => {
            handle_ponderhit(current_job);
        }
        UciCommand::SetOption(parts) => {
            handle_setoption(&parts, options, search, current_job);
        }
        UciCommand::Debug(value) => {
            uci_state.debug = matches!(value.as_deref(), Some("on"));
            if let Ok(mut guard) = search.lock() {
                guard.set_trace(uci_state.debug);
            }
        }
        UciCommand::Quit => {
            stop_search(current_job);
            return false;
        }
        UciCommand::Unknown(line) => {
            if uci_state.debug {
                eprintln!("Unknown command: {}", line);
            }
        }
    }
    true
}

/// Protocol to use for communication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Uci,
    XBoard,
    Auto,
}

fn parse_args() -> Protocol {
    let args: Vec<String> = std::env::args().collect();
    for arg in &args[1..] {
        match arg.as_str() {
            "--uci" | "-u" => return Protocol::Uci,
            "--xboard" | "-x" => return Protocol::XBoard,
            _ => {}
        }
    }
    Protocol::Auto
}

fn run_uci() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut options = UciOptions::new(DEFAULT_TT_MB);
    let search = Arc::new(Mutex::new(SearchState::new(options.hash_mb)));
    let mut current_job: Option<SearchJob> = None;
    let mut uci_state = UciState::default();

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
                &mut uci_state,
            );
            if !keep_running {
                break;
            }
        }

        stdout.flush().unwrap();
    }
}

fn main() {
    let protocol = parse_args();

    match protocol {
        Protocol::Uci => run_uci(),
        Protocol::XBoard => chess_engine::xboard::run_xboard(),
        Protocol::Auto => {
            // Auto-detect based on first command
            let stdin = io::stdin();
            let mut first_line = String::new();
            if stdin.read_line(&mut first_line).is_ok() {
                let trimmed = first_line.trim();
                if trimmed == "xboard" || trimmed.starts_with("protover") {
                    // XBoard mode - process first command and continue
                    let mut handler = chess_engine::xboard::XBoardHandler::new();
                    if let Some(cmd) = chess_engine::xboard::command::parse_xboard_command(trimmed) {
                        if let Some(response) = handler.handle_command(&cmd) {
                            for line in response.lines() {
                                println!("{line}");
                            }
                        }
                    }
                    handler.run();
                } else {
                    // UCI mode - process first command and continue
                    let mut stdout = io::stdout();
                    let mut board = Board::new();
                    let mut options = UciOptions::new(DEFAULT_TT_MB);
                    let search = Arc::new(Mutex::new(SearchState::new(options.hash_mb)));
                    let mut current_job: Option<SearchJob> = None;
                    let mut uci_state = UciState::default();

                    // Handle first command
                    if let Some(cmd) = parse_uci_command(trimmed) {
                        handle_command(
                            cmd,
                            &mut board,
                            &mut options,
                            &search,
                            &mut current_job,
                            &mut uci_state,
                        );
                    }
                    stdout.flush().unwrap();

                    // Continue with remaining input
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
                                &mut uci_state,
                            );
                            if !keep_running {
                                break;
                            }
                        }
                        stdout.flush().unwrap();
                    }
                }
            }
        }
    }
}
