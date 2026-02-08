//! XBoard/WinBoard protocol implementation.
//!
//! Handles communication with chess GUIs using the `XBoard` protocol.
//! This is an alternative to UCI, commonly used by older interfaces.
//!
//! # Protocol Overview
//!
//! `XBoard` uses SAN (Standard Algebraic Notation) for moves, unlike UCI which
//! uses long algebraic notation. Key differences:
//!
//! - Moves: "Nf3", "O-O", "exd5" (SAN) vs "g1f3", "e1g1", "e4d5" (UCI)
//! - Time: centiseconds vs milliseconds
//! - Thinking output: `<ply> <score> <time> <nodes> <pv>`

pub mod command;
pub mod output;

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::board::{
    find_best_move, find_best_move_with_ponder, find_best_move_with_time_and_ponder, Board, Color,
    Move, SearchClock, SearchLimits, SearchResult, SearchState, DEFAULT_TT_MB,
};
use crate::engine::time::{TimeConfig, TimeControl};

use command::{parse_xboard_command, XBoardCommand};
use output::{format_error, format_features, format_illegal_move, format_move, format_pong};

/// Ponder state for background thinking
struct PonderState {
    /// The expected opponent move we're pondering on

    /// Stop flag for the ponder search
    stop: Arc<AtomicBool>,
    /// Handle to the ponder thread
    handle: JoinHandle<Option<SearchResult>>,
}

/// `XBoard` protocol handler state
#[allow(clippy::struct_excessive_bools)]
pub struct XBoardHandler {
    board: Board,
    state: Arc<Mutex<SearchState>>,
    force_mode: bool,
    engine_color: Option<Color>,
    post_thinking: bool,
    pondering_enabled: bool,
    max_depth: u32,
    time_per_move_cs: Option<u32>,
    engine_time_cs: u64,
    opponent_time_cs: u64,
    moves_per_session: u32,
    base_time_sec: u32,
    increment_sec: u32,
    stop_flag: Arc<AtomicBool>,
    move_history: Vec<(Move, crate::board::UnmakeInfo)>,
    opponent_name: Option<String>,
    /// Active ponder search state
    ponder: Option<PonderState>,
    /// Whether we're in edit mode
    edit_mode: bool,
    /// Side to move in edit mode (true = white)
    edit_white_to_move: bool,
    /// Whether we're in analyze mode
    analyze_mode: bool,
    /// Active analyze search state
    analyze_handle: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
    /// Whether the engine is paused
    paused: bool,
}

impl Default for XBoardHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl XBoardHandler {
    fn handle_game_management_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::New => {
                self.stop_ponder();
                self.board = Board::new();
                self.force_mode = false;
                self.engine_color = Some(Color::Black);
                self.move_history.clear();
                self.state.lock().new_search();
                None
            }
            XBoardCommand::SetBoard(fen) => {
                self.stop_ponder();
                match Board::try_from_fen(fen) {
                    Ok(board) => {
                        self.board = board;
                        self.move_history.clear();
                        None
                    }
                    Err(e) => Some(format_error(fen, &e.to_string())),
                }
            }
            XBoardCommand::UserMove(mv_str) => self.handle_user_move(mv_str),
            XBoardCommand::Go => {
                self.force_mode = false;
                self.engine_color = Some(self.board.side_to_move());
                None
            }
            XBoardCommand::Force => {
                self.force_mode = true;
                self.engine_color = None;
                None
            }
            XBoardCommand::PlayOther => {
                self.engine_color = Some(self.board.side_to_move().opponent());
                None
            }
            XBoardCommand::White => {
                self.engine_color = Some(Color::White);
                None
            }
            XBoardCommand::Black => {
                self.engine_color = Some(Color::Black);
                None
            }
            XBoardCommand::Undo => {
                if let Some((mv, info)) = self.move_history.pop() {
                    self.board.unmake_move(mv, info);
                }
                None
            }
            XBoardCommand::Remove => {
                // Remove two half-moves
                for _ in 0..2 {
                    if let Some((mv, info)) = self.move_history.pop() {
                        self.board.unmake_move(mv, info);
                    }
                }
                None
            }
            XBoardCommand::Result(_) => {
                self.force_mode = true;
                None
            }
            XBoardCommand::Hint => {
                if let Some(mv) = self.get_hint() {
                    Some(output::format_hint(&self.board, &mv))
                } else {
                    None
                }
            }
            XBoardCommand::Draw => {
                // For now, always decline draws
                // Could check if position is drawn and accept
                None
            }
            _ => None, // Commands not handled by this helper
        }
    }

    fn handle_time_setting_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Time(cs) => {
                self.engine_time_cs = *cs;
                None
            }
            XBoardCommand::OTime(cs) => {
                self.opponent_time_cs = *cs;
                None
            }
            XBoardCommand::Level {
                moves_per_session,
                base_seconds,
                increment_seconds,
            } => {
                self.moves_per_session = *moves_per_session;
                self.base_time_sec = *base_seconds;
                self.increment_sec = *increment_seconds;
                self.time_per_move_cs = None;
                None
            }
            XBoardCommand::St(secs) => {
                self.time_per_move_cs = Some(*secs * 100);
                None
            }
            _ => None, // Commands not handled by this helper
        }
    }

    fn handle_search_control_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Sd(depth) => {
                self.max_depth = *depth;
                None
            }
            XBoardCommand::MoveNow => {
                self.stop_flag.store(true, Ordering::SeqCst);
                None
            }
            XBoardCommand::Post => {
                self.post_thinking = true;
                None
            }
            XBoardCommand::NoPost => {
                self.post_thinking = false;
                None
            }
            XBoardCommand::Hard => {
                self.pondering_enabled = true;
                None
            }
            XBoardCommand::Easy => {
                self.pondering_enabled = false;
                None
            }
            XBoardCommand::Memory(mb) => {
                self.stop_ponder();
                self.state.lock().reset_tables(*mb as usize);
                None
            }
            XBoardCommand::Cores(_n) => {
                // Single-threaded engine, ignore
                None
            }
            _ => None, // Commands not handled by this helper
        }
    }

    fn handle_protocol_misc_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::XBoard => {
                // Acknowledge XBoard mode
                None
            }
            XBoardCommand::Protover(version) => {
                if *version >= 2 {
                    Some(format_features())
                } else {
                    None
                }
            }
            XBoardCommand::Ping(n) => Some(format_pong(*n)),
            XBoardCommand::Name(name) => {
                self.opponent_name = Some(name.clone());
                None
            }
            XBoardCommand::Random | XBoardCommand::Computer => {
                // Random and Computer modes: acknowledge silently (no-op)
                None
            }
            XBoardCommand::Pause => {
                self.paused = true;
                self.stop_ponder();
                self.stop_analyze();
                None
            }
            XBoardCommand::Resume => {
                self.paused = false;
                // If in analyze mode, restart analysis
                if self.analyze_mode {
                    self.start_analyze();
                }
                None
            }
            XBoardCommand::Quit => {
                self.stop_ponder();
                self.stop_analyze();
                std::process::exit(0);
            }
            XBoardCommand::Unknown(s) => Some(format_error(s, "unknown command")),
            _ => None,
        }
    }

    fn handle_edit_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Edit => {
                self.edit_mode = true;
                self.edit_white_to_move = true;
                None
            }
            XBoardCommand::EditDone => {
                self.edit_mode = false;
                // Set side to move based on edit_white_to_move
                if self.edit_white_to_move != self.board.white_to_move() {
                    self.board.flip_side_to_move();
                }
                None
            }
            XBoardCommand::ClearBoard => {
                if self.edit_mode {
                    self.board.clear();
                }
                None
            }
            XBoardCommand::EditColor(c) => {
                if self.edit_mode {
                    self.edit_white_to_move = *c == 'w' || *c == 'W';
                }
                None
            }
            XBoardCommand::EditPiece(piece_str) => {
                if self.edit_mode {
                    self.place_piece(piece_str);
                }
                None
            }
            _ => None,
        }
    }

    fn handle_analyze_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Analyze => {
                self.analyze_mode = true;
                self.force_mode = true; // In analyze mode, don't auto-play
                self.start_analyze();
                None
            }
            XBoardCommand::ExitAnalyze => {
                self.analyze_mode = false;
                self.stop_analyze();
                None
            }
            _ => None,
        }
    }

    /// Create a new `XBoard` handler.
    #[must_use]
    pub fn new() -> Self {
        XBoardHandler {
            board: Board::new(),
            state: Arc::new(Mutex::new(SearchState::new(DEFAULT_TT_MB))),
            force_mode: false,
            engine_color: None,
            post_thinking: false,
            pondering_enabled: false,
            max_depth: 64,
            time_per_move_cs: None,
            engine_time_cs: 0,
            opponent_time_cs: 0,
            moves_per_session: 40,
            base_time_sec: 300, // 5 minutes in seconds
            increment_sec: 0,
            stop_flag: Arc::new(AtomicBool::new(false)),
            move_history: Vec::new(),
            opponent_name: None,
            ponder: None,
            edit_mode: false,
            edit_white_to_move: true,
            analyze_mode: false,
            analyze_handle: None,
            paused: false,
        }
    }

    /// Stop any active ponder search
    fn stop_ponder(&mut self) {
        if let Some(ponder) = self.ponder.take() {
            ponder.stop.store(true, Ordering::Relaxed);
            let _ = ponder.handle.join();
        }
    }

    /// Stop any active analyze search
    fn stop_analyze(&mut self) {
        if let Some((stop, handle)) = self.analyze_handle.take() {
            stop.store(true, Ordering::Relaxed);
            let _ = handle.join();
        }
    }

    /// Start analyze mode (continuous search with output)
    fn start_analyze(&mut self) {
        self.stop_analyze();

        if self.paused {
            return;
        }

        let board = self.board.clone();
        let state = Arc::clone(&self.state);
        let max_depth = self.max_depth;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let post_thinking = self.post_thinking;

        let handle = thread::spawn(move || {
            let mut board = board;
            let mut guard = state.lock();
            guard.new_search();

            // Iterative deepening with output
            for depth in 1..=max_depth {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                let start_time = Instant::now();
                let result = find_best_move(&mut board, &mut guard, depth, &stop_clone);

                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                if let Some(mv) = result {
                    let elapsed_cs = start_time.elapsed().as_millis() as u64 / 10;
                    let nodes = guard.stats.nodes;
                    let score = 0; // We don't have easy access to score here

                    if post_thinking {
                        // XBoard analyze output format: depth score time nodes pv
                        let san = board.move_to_san(&mv);
                        println!("{depth} {score} {elapsed_cs} {nodes} {san}");
                    }
                }
            }
        });

        self.analyze_handle = Some((stop, handle));
    }

    /// Place a piece on the board in edit mode (e.g., "Pa2", "Ke1", "x" to remove)
    fn place_piece(&mut self, piece_str: &str) {
        if piece_str.is_empty() {
            return;
        }

        let chars: Vec<char> = piece_str.chars().collect();

        // Handle "x" prefix for removing pieces (e.g., "xa2")
        if chars[0] == 'x' && chars.len() >= 3 {
            let file = chars[1];
            let rank = chars[2];
            if let Some(sq) = parse_square(file, rank) {
                self.board.remove_piece_at(sq);
            }
            return;
        }

        // Normal piece placement: "Pa2", "Ke1", etc.
        if chars.len() < 3 {
            return;
        }

        let piece_char = chars[0];
        let file = chars[1];
        let rank = chars[2];

        let color = if self.edit_white_to_move {
            Color::White
        } else {
            Color::Black
        };

        let piece = match piece_char {
            'P' => Some(crate::board::Piece::Pawn),
            'N' => Some(crate::board::Piece::Knight),
            'B' => Some(crate::board::Piece::Bishop),
            'R' => Some(crate::board::Piece::Rook),
            'Q' => Some(crate::board::Piece::Queen),
            'K' => Some(crate::board::Piece::King),
            _ => None,
        };

        if let (Some(piece), Some(sq)) = (piece, parse_square(file, rank)) {
            self.board.place_piece(sq, color, piece);
        }
    }

    /// Start pondering on the expected opponent move
    fn start_ponder(&mut self, ponder_move: Move) {
        // Stop any existing ponder
        self.stop_ponder();

        // Create ponder position: make our move, then opponent's expected reply
        let mut ponder_board = self.board.clone();
        ponder_board.make_move(ponder_move);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let state_clone = Arc::clone(&self.state);
        let max_depth = self.max_depth;

        let handle = thread::spawn(move || {
            let mut guard = state_clone.lock();
            // Search with low depth for pondering (background thinking)
            let result =
                find_best_move_with_ponder(&mut ponder_board, &mut guard, max_depth, &stop_clone);
            Some(result)
        });

        self.ponder = Some(PonderState { stop, handle });
    }

    /// Run the `XBoard` protocol main loop.
    pub fn run(&mut self) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };

            if let Some(cmd) = parse_xboard_command(&line) {
                let response = self.handle_command(&cmd);
                if let Some(resp) = response {
                    for line in resp.lines() {
                        writeln!(stdout, "{line}").ok();
                    }
                    stdout.flush().ok();
                }

                // Check if we should think
                if self.should_think() {
                    if let Some(result) = self.think() {
                        if let Some(mv) = result.best_move {
                            let output = format_move(&self.board, &mv);
                            writeln!(stdout, "{output}").ok();
                            stdout.flush().ok();
                            let info = self.board.make_move(mv);
                            self.move_history.push((mv, info));

                            // Start pondering if enabled and we have a ponder move
                            if self.pondering_enabled {
                                if let Some(ponder_mv) = result.ponder_move {
                                    self.start_ponder(ponder_mv);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle a single `XBoard` command.
    pub fn handle_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        if let Some(response) = self.handle_game_management_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_time_setting_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_search_control_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_edit_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_analyze_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_protocol_misc_command(cmd) {
            return Some(response);
        }

        None
    }

    /// Handle a user move (in SAN or coordinate notation).
    fn handle_user_move(&mut self, mv_str: &str) -> Option<String> {
        // Stop any ongoing ponder or analyze
        self.stop_ponder();
        self.stop_analyze();

        // Try SAN first, then coordinate notation
        let mv = self
            .board
            .parse_san(mv_str)
            .or_else(|_| self.board.parse_move(mv_str));

        match mv {
            Ok(mv) => {
                let info = self.board.make_move(mv);
                self.move_history.push((mv, info));
                // Restart analysis if in analyze mode
                if self.analyze_mode && !self.paused {
                    self.start_analyze();
                }
                None
            }
            Err(_) => Some(format_illegal_move(mv_str)),
        }
    }

    /// Check if the engine should think now.
    fn should_think(&self) -> bool {
        if self.force_mode || self.paused || self.analyze_mode {
            return false;
        }
        match self.engine_color {
            Some(color) => self.board.side_to_move() == color,
            None => false,
        }
    }

    /// Think and return the search result with best move and ponder move.
    #[allow(clippy::unnecessary_wraps)]
    fn think(&mut self) -> Option<SearchResult> {
        // Stop any ongoing ponder
        self.stop_ponder();

        self.stop_flag.store(false, Ordering::SeqCst);

        let mut state = self.state.lock();

        // Determine time control using unified TimeControl enum
        let time_control = if let Some(time_cs) = self.time_per_move_cs {
            // Fixed time per move (XBoard "st" command, in centiseconds)
            TimeControl::from_xboard_st(time_cs)
        } else if self.engine_time_cs > 0 {
            // Incremental time control (XBoard "time" command)
            TimeControl::from_xboard_time(
                self.engine_time_cs,
                self.increment_sec,
                if self.moves_per_session > 0 {
                    Some(self.moves_per_session)
                } else {
                    None
                },
            )
        } else {
            // Fixed depth - no time limit
            TimeControl::Depth
        };

        // Compute time limits
        if time_control.is_unlimited() {
            // Fixed depth search
            Some(find_best_move_with_ponder(
                &mut self.board,
                &mut state,
                self.max_depth,
                &self.stop_flag,
            ))
        } else {
            // Timed search
            let config = TimeConfig {
                move_overhead_ms: 0,
                soft_time_percent: 5,
                hard_time_percent: 15,
                default_max_nodes: 0,
            };
            let (soft_ms, hard_ms) = time_control.compute_limits(&config);

            let start = Instant::now();
            let soft_deadline = start + Duration::from_millis(soft_ms);
            let hard_deadline = start + Duration::from_millis(hard_ms);
            let clock = Arc::new(SearchClock::new(
                start,
                Some(soft_deadline),
                Some(hard_deadline),
            ));
            let limits = SearchLimits {
                clock,
                stop: self.stop_flag.clone(),
            };
            Some(find_best_move_with_time_and_ponder(
                &mut self.board,
                &mut state,
                &limits,
            ))
        }
    }

    /// Get a hint (quick search).
    fn get_hint(&mut self) -> Option<Move> {
        let mut state = self.state.lock();
        find_best_move(&mut self.board, &mut state, 4, &self.stop_flag)
    }
}

/// Entry point for `XBoard` mode.
pub fn run_xboard() {
    let mut handler = XBoardHandler::new();
    handler.run();
}

/// Parse a square from file and rank characters (e.g., 'e', '4' -> e4)
fn parse_square(file: char, rank: char) -> Option<crate::board::Square> {
    let file_idx = match file {
        'a'..='h' => file as u8 - b'a',
        _ => return None,
    };
    let rank_idx = match rank {
        '1'..='8' => rank as u8 - b'1',
        _ => return None,
    };
    Some(crate::board::Square::from_index(
        (rank_idx * 8 + file_idx) as usize,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_command() {
        let mut handler = XBoardHandler::new();
        handler.handle_command(&XBoardCommand::New);
        assert!(!handler.force_mode);
        assert_eq!(handler.engine_color, Some(Color::Black));
    }

    #[test]
    fn test_force_command() {
        let mut handler = XBoardHandler::new();
        handler.handle_command(&XBoardCommand::Force);
        assert!(handler.force_mode);
    }

    #[test]
    fn test_usermove() {
        let mut handler = XBoardHandler::new();
        handler.handle_command(&XBoardCommand::Force);
        let result = handler.handle_command(&XBoardCommand::UserMove("e4".to_string()));
        assert!(result.is_none());
    }

    #[test]
    fn test_protover() {
        let mut handler = XBoardHandler::new();
        let result = handler.handle_command(&XBoardCommand::Protover(2));
        assert!(result.is_some());
        let features = result.unwrap();
        assert!(features.contains("setboard=1"));
    }

    #[test]
    fn test_ping_pong() {
        let mut handler = XBoardHandler::new();
        let result = handler.handle_command(&XBoardCommand::Ping(42));
        assert_eq!(result, Some("pong 42".to_string()));
    }

    #[test]
    fn test_setboard() {
        let mut handler = XBoardHandler::new();
        let result = handler.handle_command(&XBoardCommand::SetBoard(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
        ));
        assert!(result.is_none());
        assert!(!handler.board.white_to_move());
    }

    #[test]
    fn test_pause_resume() {
        let mut handler = XBoardHandler::new();
        assert!(!handler.paused);
        handler.handle_command(&XBoardCommand::Pause);
        assert!(handler.paused);
        handler.handle_command(&XBoardCommand::Resume);
        assert!(!handler.paused);
    }

    #[test]
    fn test_edit_mode() {
        let mut handler = XBoardHandler::new();
        assert!(!handler.edit_mode);
        handler.handle_command(&XBoardCommand::Edit);
        assert!(handler.edit_mode);
        handler.handle_command(&XBoardCommand::EditDone);
        assert!(!handler.edit_mode);
    }

    #[test]
    fn test_analyze_mode() {
        let mut handler = XBoardHandler::new();
        assert!(!handler.analyze_mode);
        handler.handle_command(&XBoardCommand::Analyze);
        assert!(handler.analyze_mode);
        assert!(handler.force_mode); // Analyze mode sets force mode
        handler.handle_command(&XBoardCommand::ExitAnalyze);
        assert!(!handler.analyze_mode);
    }

    #[test]
    fn test_random_noop() {
        let mut handler = XBoardHandler::new();
        let result = handler.handle_command(&XBoardCommand::Random);
        assert!(result.is_none()); // Should be silent no-op
    }

    #[test]
    fn test_result() {
        let mut handler = XBoardHandler::new();
        handler.handle_command(&XBoardCommand::New);
        assert!(!handler.force_mode);
        handler.handle_command(&XBoardCommand::Result("1-0 {White wins}".to_string()));
        assert!(handler.force_mode); // Result sets force mode
    }
}
