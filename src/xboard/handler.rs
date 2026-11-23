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

    pub(super) fn dispatch_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        // Edit commands take priority over the normal move handler because
        // editing a piece can look like a SAN move.
        let (consumed, response) = self.handle_edit_command(cmd);
        if consumed {
            return response;
        }

        match cmd {
            XBoardCommand::New => self.reset_game(),
            XBoardCommand::SetBoard(fen) => return self.set_position(fen),
            XBoardCommand::UserMove(mv_str) => return self.handle_user_move(mv_str),
            XBoardCommand::Go | XBoardCommand::PlayOther => {
                self.analyze_mode = false;
                self.force_mode = false;
                let side = self.board.side_to_move();
                self.engine_color = Some(if matches!(cmd, XBoardCommand::Go) {
                    side
                } else {
                    side.opponent()
                });
            }
            XBoardCommand::Force => {
                self.analyze_mode = false;
                self.force_mode = true;
                self.engine_color = None;
            }
            XBoardCommand::White => self.set_turn(Color::White),
            XBoardCommand::Black => self.set_turn(Color::Black),
            XBoardCommand::Undo => self.unmake_recent_moves(1),
            XBoardCommand::Remove => self.unmake_recent_moves(2),
            XBoardCommand::Result(_) => {
                self.analyze_mode = false;
                self.force_mode = true;
            }
            XBoardCommand::Hint => {
                return self
                    .get_hint()
                    .map(|mv| output::format_hint(&self.board, &mv));
            }
            XBoardCommand::Time(cs) => self.engine_time_cs = Some(*cs),
            XBoardCommand::OTime(cs) => self.opponent_time_cs = *cs,
            XBoardCommand::Level {
                moves_per_session,
                base_seconds,
                increment_seconds,
            } => self.set_level(
                *moves_per_session,
                *base_seconds,
                u64::from(*increment_seconds) * 1000,
            ),
            XBoardCommand::LevelWithFractionalIncrement {
                moves_per_session,
                base_seconds,
                increment_ms,
            } => self.set_level(*moves_per_session, *base_seconds, *increment_ms),
            XBoardCommand::St(secs) => self.time_per_move_sec = Some(*secs),
            XBoardCommand::Sd(depth) => self.max_depth = normalized_search_depth(*depth),
            XBoardCommand::Post => self.post_thinking.store(true, Ordering::Relaxed),
            XBoardCommand::NoPost => self.post_thinking.store(false, Ordering::Relaxed),
            XBoardCommand::Hard => self.pondering_enabled = true,
            XBoardCommand::Easy => {
                self.pondering_enabled = false;
                self.stop_ponder();
            }
            XBoardCommand::Memory(mb) => {
                self.state.lock().reset_tables(normalized_memory_mb(*mb));
            }
            XBoardCommand::Analyze => {
                self.analyze_mode = true;
                self.force_mode = true;
            }
            XBoardCommand::ExitAnalyze => {
                self.analyze_mode = false;
                self.stop_analyze();
            }
            XBoardCommand::Protover(version) => return (*version >= 2).then(format_features),
            XBoardCommand::Ping(n) => return Some(format_pong(*n)),
            XBoardCommand::Name(name) => self.opponent_name = Some(name.clone()),
            XBoardCommand::Pause => self.paused = true,
            XBoardCommand::Resume => {
                self.paused = false;
                if self.analyze_mode {
                    self.start_analyze();
                }
            }
            XBoardCommand::Unknown(s) => return Some(format_error(s, "unknown command")),
            XBoardCommand::XBoard
            | XBoardCommand::Accepted(_)
            | XBoardCommand::Rejected(_)
            | XBoardCommand::MoveNow
            | XBoardCommand::Draw
            | XBoardCommand::Edit
            | XBoardCommand::EditDone
            | XBoardCommand::ClearBoard
            | XBoardCommand::EditPiece(_)
            | XBoardCommand::EditColor
            | XBoardCommand::Computer
            | XBoardCommand::Random
            | XBoardCommand::Cores(_)
            | XBoardCommand::Quit => {}
        }
        None
    }

    fn reset_game(&mut self) {
        self.board = Board::new();
        self.force_mode = self.analyze_mode;
        self.engine_color = Some(Color::Black);
        self.max_depth = DEFAULT_MAX_DEPTH;
        self.level_start_fullmove = 0;
        self.opponent_time_cs = seconds_to_centiseconds(self.base_time_sec);
        self.engine_time_cs = Some(self.opponent_time_cs);
        self.move_history.clear();
        self.state.lock().new_game();
    }

    fn set_position(&mut self, fen: &str) -> Option<String> {
        let position = Board::try_from_fen(fen).and_then(|board| {
            board.validate_king_counts()?;
            Ok(board)
        });
        match position {
            Ok(board) => {
                self.board = board;
                self.move_history.clear();
                None
            }
            Err(error) => Some(format_error(fen, &error.to_string())),
        }
    }

    fn set_turn(&mut self, side: Color) {
        if self.board.side_to_move() != side {
            self.board.flip_side_to_move();
            self.move_history.clear();
        }
        self.engine_color = Some(side.opponent());
    }

    fn set_level(&mut self, moves_per_session: u32, base_seconds: u32, increment_ms: u64) {
        self.moves_per_session = moves_per_session;
        self.level_start_fullmove = self.board.fullmove_number().saturating_sub(1);
        self.base_time_sec = base_seconds;
        self.increment_ms = increment_ms;
        self.opponent_time_cs = seconds_to_centiseconds(base_seconds);
        self.engine_time_cs = Some(self.opponent_time_cs);
        self.time_per_move_sec = None;
    }

    /// Handle a user move (in SAN or coordinate notation).
    fn handle_user_move(&mut self, mv_str: &str) -> Option<String> {
        let mv = self
            .board
            .parse_san(mv_str)
            .or_else(|_| self.board.parse_move(mv_str));

        match mv {
            Ok(mv) => {
                let info = self.board.make_move(mv);
                self.move_history.push((mv, info));
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
        self.engine_color == Some(self.board.side_to_move())
    }
}
