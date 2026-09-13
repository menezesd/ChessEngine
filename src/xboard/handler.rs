use std::sync::atomic::Ordering;

use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::board::{Board, Color, DEFAULT_TT_MB};

use super::command::XBoardCommand;
use super::output::{self, format_error, format_features, format_illegal_move, format_pong};
use super::state::XBoardHandler;

const CENTISECONDS_PER_SECOND: u64 = 100;
const XBOARD_MEMORY_MIN_MB: u32 = 1;
const XBOARD_MEMORY_MAX_MB: u32 = DEFAULT_TT_MB as u32;

pub(super) fn normalized_memory_mb(mb: u32) -> usize {
    mb.clamp(XBOARD_MEMORY_MIN_MB, XBOARD_MEMORY_MAX_MB) as usize
}

pub(super) fn normalized_search_depth(depth: u32) -> u32 {
    depth.min(DEFAULT_MAX_DEPTH)
}

pub(super) fn seconds_to_centiseconds(seconds: u32) -> u64 {
    u64::from(seconds).saturating_mul(CENTISECONDS_PER_SECOND)
}

impl XBoardHandler {
    fn unmake_recent_moves(&mut self, count: usize) {
        for _ in 0..count {
            if let Some((mv, info)) = self.move_history.pop() {
                self.board.unmake_move(mv, info);
            }
        }
    }

    pub(super) fn handle_game_management_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::New => {
                self.stop_ponder();
                self.board = Board::new();
                self.force_mode = self.analyze_mode;
                self.engine_color = Some(Color::Black);
                self.max_depth = DEFAULT_MAX_DEPTH;
                self.level_start_fullmove = 0;
                self.opponent_time_cs = seconds_to_centiseconds(self.base_time_sec);
                self.engine_time_cs = Some(self.opponent_time_cs);
                self.move_history.clear();
                self.state.lock().new_game();
                None
            }
            XBoardCommand::SetBoard(fen) => {
                self.stop_ponder();
                match Board::try_from_fen(fen) {
                    Ok(board) => match board.validate_king_counts() {
                        Ok(()) => {
                            self.board = board;
                            self.move_history.clear();
                            None
                        }
                        Err(error) => Some(format_error(fen, &error.to_string())),
                    },
                    Err(error) => Some(format_error(fen, &error.to_string())),
                }
            }
            XBoardCommand::UserMove(mv_str) => self.handle_user_move(mv_str),
            XBoardCommand::Go => {
                self.analyze_mode = false;
                self.force_mode = false;
                self.engine_color = Some(self.board.side_to_move());
                None
            }
            XBoardCommand::Force => {
                self.analyze_mode = false;
                self.force_mode = true;
                self.engine_color = None;
                None
            }
            XBoardCommand::PlayOther => {
                self.analyze_mode = false;
                self.force_mode = false;
                self.engine_color = Some(self.board.side_to_move().opponent());
                None
            }
            XBoardCommand::White | XBoardCommand::Black => {
                let side = if matches!(cmd, XBoardCommand::White) {
                    Color::White
                } else {
                    Color::Black
                };
                if self.board.side_to_move() != side {
                    self.board.flip_side_to_move();
                    self.move_history.clear();
                }
                self.engine_color = Some(side.opponent());
                None
            }
            XBoardCommand::Undo => {
                self.unmake_recent_moves(1);
                None
            }
            XBoardCommand::Remove => {
                self.unmake_recent_moves(2);
                None
            }
            XBoardCommand::Result(_) => {
                self.analyze_mode = false;
                self.force_mode = true;
                None
            }
            XBoardCommand::Hint => self
                .get_hint()
                .map(|mv| output::format_hint(&self.board, &mv)),
            _ => None,
        }
    }

    pub(super) fn handle_time_setting_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Time(cs) => {
                self.engine_time_cs = Some(*cs);
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
                self.level_start_fullmove = self.board.fullmove_number().saturating_sub(1);
                self.base_time_sec = *base_seconds;
                self.increment_sec = *increment_seconds;
                self.opponent_time_cs = seconds_to_centiseconds(*base_seconds);
                self.engine_time_cs = Some(self.opponent_time_cs);
                self.time_per_move_sec = None;
                None
            }
            XBoardCommand::St(secs) => {
                self.time_per_move_sec = Some(*secs);
                None
            }
            _ => None,
        }
    }

    pub(super) fn handle_search_control_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Sd(depth) => {
                self.max_depth = normalized_search_depth(*depth);
                None
            }
            XBoardCommand::MoveNow => {
                self.stop_flag.store(true, Ordering::SeqCst);
                self.finish_thinking()
            }
            XBoardCommand::Post => {
                self.post_thinking.store(true, Ordering::Relaxed);
                None
            }
            XBoardCommand::NoPost => {
                self.post_thinking.store(false, Ordering::Relaxed);
                None
            }
            XBoardCommand::Hard => {
                self.pondering_enabled = true;
                None
            }
            XBoardCommand::Easy => {
                self.pondering_enabled = false;
                self.stop_ponder();
                None
            }
            XBoardCommand::Memory(mb) => {
                self.stop_ponder();
                self.state.lock().reset_tables(normalized_memory_mb(*mb));
                None
            }
            XBoardCommand::Cores(_n) => None,
            _ => None,
        }
    }

    pub(super) fn handle_protocol_misc_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
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
            XBoardCommand::Pause => {
                self.paused = true;
                self.stop_ponder();
                self.stop_analyze();
                None
            }
            XBoardCommand::Resume => {
                self.paused = false;
                if self.analyze_mode {
                    self.start_analyze();
                }
                None
            }
            XBoardCommand::Quit => {
                self.stop_ponder();
                self.stop_analyze();
                None
            }
            XBoardCommand::Unknown(s) => Some(format_error(s, "unknown command")),
            _ => None,
        }
    }

    pub(super) fn handle_analyze_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        match cmd {
            XBoardCommand::Analyze => {
                self.analyze_mode = true;
                self.force_mode = true;
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

    /// Handle a user move (in SAN or coordinate notation).
    pub(super) fn handle_user_move(&mut self, mv_str: &str) -> Option<String> {
        self.stop_ponder();
        self.stop_analyze();

        let mv = self
            .board
            .parse_san(mv_str)
            .or_else(|_| self.board.parse_move(mv_str));

        match mv {
            Ok(mv) => {
                let info = self.board.make_move(mv);
                self.move_history.push((mv, info));
                if self.analyze_mode && !self.paused {
                    self.start_analyze();
                }
                None
            }
            Err(_) => Some(format_illegal_move(mv_str)),
        }
    }

    /// Check if the engine should think now.
    pub(super) fn should_think(&self) -> bool {
        if self.force_mode || self.paused || self.analyze_mode || self.edit_mode {
            return false;
        }
        match self.engine_color {
            Some(color) => self.board.side_to_move() == color,
            None => false,
        }
    }
}
