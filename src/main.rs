use std::io;
use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chess_engine::board::SearchIterationInfo;
use chess_engine::board::DEFAULT_TT_MB;
use chess_engine::engine::time::{build_search_request, TimeControl};
use chess_engine::engine::{EngineController, SearchParams as EngineSearchParams};
use chess_engine::uci::command::{parse_go_params, parse_uci_command, GoParams, UciCommand};
use chess_engine::uci::options::{parse_setoption, UciOptionAction, UciOptions};
use chess_engine::uci::parse_position_command;
use chess_engine::uci::print::{print_perft_info, print_time_info};
use chess_engine::uci::report::{print_bestmove_with_ponder, print_ready};

/// Default depth limit when searching by nodes
const NODE_SEARCH_DEFAULT_DEPTH: u32 = 64;
/// Fallback time allocation when no time control is specified
const FALLBACK_TIME_SECS: u64 = 5;
const KNOWN_COMMANDS: &[&str] = &[
    "uci",
    "isready",
    "ucinewgame",
    "position",
    "go",
    "perft",
    "setoption",
    "debug",
    "stop",
    "ponderhit",
    "quit",
];

/// UCI session state (time controls, debug mode)
struct UciState {
    time_control: TimeControl,
    debug: bool,
}

impl Default for UciState {
    fn default() -> Self {
        UciState {
            time_control: TimeControl::move_time(Duration::from_secs(FALLBACK_TIME_SECS)),
            debug: false,
        }
    }
}

impl UciState {
    fn update_time_control(&mut self, params: &GoParams, is_white: bool) -> TimeControl {
        if params.infinite || params.ponder {
            self.time_control = TimeControl::Infinite;
            return self.time_control;
        }

        if let Some(mt) = params.movetime {
            self.time_control = TimeControl::move_time(Duration::from_millis(mt));
            return self.time_control;
        }

        let fallback = Duration::from_secs(FALLBACK_TIME_SECS);
        let time_left = if is_white {
            params.wtime.map(Duration::from_millis)
        } else {
            params.btime.map(Duration::from_millis)
        }
        .unwrap_or(fallback);

        let inc = if is_white {
            params.winc.map(Duration::from_millis)
        } else {
            params.binc.map(Duration::from_millis)
        }
        .unwrap_or(Duration::ZERO);

        self.time_control = TimeControl::incremental(time_left, inc, params.movestogo);
        self.time_control
    }
}

fn parts_as_strs(parts: &[String]) -> Vec<&str> {
    parts.iter().map(|p| p.as_str()).collect()
}

struct UciSession {
    engine: EngineController,
    options: UciOptions,
    state: UciState,
}

struct GoSearchPlan {
    search_params: EngineSearchParams,
    soft_time_ms: u64,
    hard_time_ms: u64,
    depth_hint: Option<u32>,
    go_ponder: bool,
    max_nodes: u64,
}

impl UciSession {
    fn new(default_tt_mb: usize) -> Self {
        let options = UciOptions::new(default_tt_mb);
        let mut engine = EngineController::new(options.hash_mb);
        engine.set_info_callback(Some(default_info_callback()));
        UciSession {
            engine,
            options,
            state: UciState::default(),
        }
    }

    fn build_go_plan(&mut self, params: &GoParams, is_white: bool) -> GoSearchPlan {
        let time_control = self.state.update_time_control(params, is_white);
        let mut depth = params.depth;
        let nodes = params.nodes;
        let mate = params.mate;
        let go_ponder = params.ponder;
        let go_infinite = params.infinite;

        if nodes.is_some() && depth.is_none() {
            depth = Some(NODE_SEARCH_DEFAULT_DEPTH);
        }

        if let Some(mate_moves) = mate {
            if mate_moves > 0 && depth.is_none() {
                depth = Some(mate_moves * 2);
            }
        }

        let (request, (soft_time_ms, hard_time_ms)) = build_search_request(
            time_control,
            depth,
            nodes,
            go_ponder,
            go_infinite,
            self.options.default_max_nodes,
            self.options.move_overhead_ms,
            self.options.soft_time_percent,
            self.options.hard_time_percent,
        );

        let search_params = EngineSearchParams {
            depth: request.depth,
            soft_time_ms: request.soft_time_ms,
            hard_time_ms: request.hard_time_ms,
            ponder: request.ponder,
            infinite: request.infinite,
        };

        GoSearchPlan {
            search_params,
            soft_time_ms,
            hard_time_ms,
            depth_hint: depth,
            go_ponder,
            max_nodes: request.max_nodes,
        }
    }

    /// Handle the "go" command - start a search
    fn handle_go(&mut self, parts: &[String]) {
        let parts_ref = parts_as_strs(parts);
        let params = parse_go_params(&parts_ref);

        let plan = self.build_go_plan(&params, self.engine.board().white_to_move());

        self.engine.set_max_nodes(plan.max_nodes);

        print_time_info(
            plan.soft_time_ms,
            plan.hard_time_ms,
            self.options.move_overhead_ms,
            plan.max_nodes,
            plan.go_ponder,
            plan.depth_hint.unwrap_or(0),
        );

        // Get board state for checkmate/stalemate reporting
        let is_checkmate = self.engine.board_mut().is_checkmate();
        let is_stalemate = self.engine.board_mut().is_stalemate();
        let is_draw = self.engine.board().is_draw();

        // Build search parameters
        self.engine.start_search(plan.search_params, move |result| {
            if result.best_move.is_none() {
                if is_checkmate {
                    println!("info score mate -1");
                } else if is_stalemate || is_draw {
                    println!("info score cp 0");
                }
            }
            print_bestmove_with_ponder(result);
        });
    }

    /// Handle the "setoption" command
    fn handle_setoption(&mut self, parts: &[String]) {
        self.engine.stop_search();
        let parts_ref = parts_as_strs(parts);
        if let Some((name, value)) = parse_setoption(&parts_ref) {
            let action = self.engine.with_search_state(|state| {
                self.options.apply_setoption(&name, value.as_deref(), state)
            });
            if let Some(Some(action)) = action {
                match action {
                    UciOptionAction::ReinitHash(new_mb) => {
                        self.engine.resize_hash(new_mb);
                    }
                    UciOptionAction::SetThreads(threads) => {
                        self.engine.set_threads(threads);
                    }
                }
            }
        }
    }

    /// Process a single UCI command. Returns false if the engine should quit.
    fn handle_command(&mut self, cmd: UciCommand) -> bool {
        match cmd {
            UciCommand::Uci => {
                self.engine
                    .with_search_state_ref(|state| self.options.print(state.params()));
            }
            UciCommand::IsReady => {
                print_ready();
            }
            UciCommand::UciNewGame => {
                self.engine.new_game();
            }
            UciCommand::Position(parts) => {
                self.engine.stop_search();
                let parts_ref = parts_as_strs(&parts);
                parse_position_command(self.engine.board_mut(), &parts_ref);
            }
            UciCommand::Perft(depth) => {
                self.engine.stop_search();
                let start = Instant::now();
                let nodes = self.engine.board_mut().perft(depth);
                let elapsed = start.elapsed();
                print_perft_info(depth, nodes, elapsed);
            }
            UciCommand::Go(parts) => {
                self.handle_go(&parts);
            }
            UciCommand::Stop => {
                self.engine.signal_stop();
            }
            UciCommand::PonderHit => {
                self.engine.ponderhit();
            }
            UciCommand::SetOption(parts) => {
                self.handle_setoption(&parts);
            }
            UciCommand::Debug(value) => {
                self.state.debug = matches!(value.as_deref(), Some("on"));
                self.engine.set_trace(self.state.debug);
            }
            UciCommand::Quit => {
                self.engine.stop_search();
                return false;
            }
            UciCommand::Unknown(line) => {
                if self.state.debug {
                    eprintln!("Unknown command: {}", line);
                    eprintln!("Known commands: {}", KNOWN_COMMANDS.join(", "));
                }
            }
        }
        true
    }
}

fn print_uci_info(info: &SearchIterationInfo) {
    if let Some(mate) = info.mate_in {
        println!(
            "info depth {} seldepth {} nodes {} nps {} time {} score mate {} pv {}",
            info.depth, info.seldepth, info.nodes, info.nps, info.time_ms, mate, info.pv
        );
    } else {
        println!(
            "info depth {} seldepth {} nodes {} nps {} time {} score cp {} pv {}",
            info.depth, info.seldepth, info.nodes, info.nps, info.time_ms, info.score, info.pv
        );
    }
}

fn default_info_callback() -> Arc<dyn Fn(&SearchIterationInfo) + Send + Sync> {
    Arc::new(print_uci_info)
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

fn run_uci_session<R: BufRead>(first_line: Option<String>, reader: R) {
    let mut stdout = io::stdout();
    let mut session = UciSession::new(DEFAULT_TT_MB);

    if let Some(line) = first_line {
        if let Some(cmd) = parse_uci_command(&line) {
            if !session.handle_command(cmd) {
                return;
            }
        }
        stdout.flush().unwrap();
    }

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(cmd) = parse_uci_command(&line) {
            let keep_running = session.handle_command(cmd);
            if !keep_running {
                break;
            }
        }

        stdout.flush().unwrap();
    }
}

fn run_uci() {
    let stdin = io::stdin();
    run_uci_session(None, stdin.lock());
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
                    if let Some(cmd) = chess_engine::xboard::command::parse_xboard_command(trimmed)
                    {
                        if let Some(response) = handler.handle_command(&cmd) {
                            for line in response.lines() {
                                println!("{line}");
                            }
                        }
                    }
                    handler.run();
                } else {
                    // UCI mode - process first command and continue
                    let remaining = stdin.lock();
                    run_uci_session(Some(trimmed.to_string()), remaining);
                }
            }
        }
    }
}
