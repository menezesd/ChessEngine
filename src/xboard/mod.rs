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
mod edit;
mod handler;
pub mod output;
mod search;
mod state;

use command::{parse_xboard_command, XBoardCommand};
use output::format_move;
pub use state::XBoardHandler;
use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

impl XBoardHandler {
    /// Run the `XBoard` protocol main loop.
    pub fn run(&mut self) {
        let mut stdout = io::stdout();
        let (sender, commands) = mpsc::channel();
        thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });

        loop {
            match commands.recv_timeout(Duration::from_millis(5)) {
                Ok(line) => {
                    if let Some(cmd) = parse_xboard_command(&line) {
                        if let Some(response) = self.handle_command(&cmd) {
                            writeln!(stdout, "{response}").ok();
                            stdout.flush().ok();
                        }
                        if matches!(cmd, XBoardCommand::Quit) {
                            break;
                        }
                        self.start_thinking();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            if self
                .thinking
                .as_ref()
                .is_some_and(thread::JoinHandle::is_finished)
            {
                if let Some(output) = self.finish_thinking() {
                    writeln!(stdout, "{output}").ok();
                    stdout.flush().ok();
                }
            }
        }
        self.stop_thinking();
        self.stop_ponder();
        self.stop_analyze();
    }

    /// Join a completed (or explicitly stopped) move search and play its move.
    fn finish_thinking(&mut self) -> Option<String> {
        let result = self.thinking.take()?.join().ok()?;
        let mv = result.best_move?;
        let output = format_move(&self.board, &mv);
        let info = self.board.make_move(mv);
        self.move_history.push((mv, info));

        if let (true, Some(ponder_mv)) = (self.pondering_enabled, result.ponder_move) {
            self.start_ponder(ponder_mv);
        }

        Some(output)
    }

    /// Handle a single `XBoard` command.
    pub fn handle_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        // Position and mode changes invalidate a running result. Join before
        // touching the board or locking search state so no stale move can
        // be published and no command waits for an uninterruptible search.
        if matches!(
            cmd,
            XBoardCommand::New
                | XBoardCommand::SetBoard(_)
                | XBoardCommand::UserMove(_)
                | XBoardCommand::Go
                | XBoardCommand::Force
                | XBoardCommand::PlayOther
                | XBoardCommand::White
                | XBoardCommand::Black
                | XBoardCommand::Undo
                | XBoardCommand::Remove
                | XBoardCommand::Result(_)
                | XBoardCommand::Edit
                | XBoardCommand::Analyze
                | XBoardCommand::Pause
                | XBoardCommand::Quit
                | XBoardCommand::Memory(_)
        ) {
            self.stop_thinking();
            self.stop_ponder();
            self.stop_analyze();
        }

        let response = self.dispatch_command(cmd);
        if self.analyze_mode
            && !self.paused
            && !self.edit_mode
            && self.analyze_handle.is_none()
            && !matches!(cmd, XBoardCommand::Quit)
        {
            self.start_analyze();
        }
        response
    }

    fn dispatch_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        // Edit commands must win over the normal move handler. Several valid
        // edit tokens (for example `Ke1`) are also valid-looking SAN moves,
        // but inside edit mode they place pieces rather than play a move.
        let (edit_consumed, edit_response) = self.handle_edit_command(cmd);
        if edit_consumed {
            return edit_response;
        }

        if let Some(response) = self.handle_game_management_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_time_setting_command(cmd) {
            return Some(response);
        }

        if let Some(response) = self.handle_search_control_command(cmd) {
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
}

/// Entry point for `XBoard` mode.
pub fn run_xboard() {
    let mut handler = XBoardHandler::new();
    handler.run();
}

#[cfg(test)]
mod tests;
