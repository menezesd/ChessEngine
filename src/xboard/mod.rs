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

use crate::board::Move;
use command::{parse_xboard_command, XBoardCommand};
use output::format_move;
pub use state::XBoardHandler;
use std::io::{self, BufRead, Write};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

struct CommandResponse {
    text: String,
    ponder_move: Option<Move>,
}

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
                        if let Some(response) = self.process_command(&cmd) {
                            self.publish_response(&mut stdout, &response);
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
                if let Some(response) = self.finish_thinking() {
                    self.publish_response(&mut stdout, &response);
                }
            }
        }
        self.stop_thinking();
        self.stop_ponder();
        self.stop_analyze();
    }

    fn publish_response(&mut self, stdout: &mut io::Stdout, response: &CommandResponse) {
        writeln!(stdout, "{}", response.text).ok();
        stdout.flush().ok();
        if let Some(mv) = response.ponder_move {
            self.start_ponder(mv);
        }
    }

    /// Join a completed (or explicitly stopped) move search and play its move.
    fn finish_thinking(&mut self) -> Option<CommandResponse> {
        let result = self.thinking.take()?.join().ok()?;
        let mv = result.best_move?;
        let text = format_move(&self.board, &mv);
        let info = self.board.make_move(mv);
        self.move_history.push((mv, info));

        Some(CommandResponse {
            text,
            ponder_move: result.ponder_move.filter(|_| self.pondering_enabled),
        })
    }

    /// Handle a single `XBoard` command.
    pub fn handle_command(&mut self, cmd: &XBoardCommand) -> Option<String> {
        let response = self.process_command(cmd)?;
        if let Some(mv) = response.ponder_move {
            self.start_ponder(mv);
        }
        Some(response.text)
    }

    fn process_command(&mut self, cmd: &XBoardCommand) -> Option<CommandResponse> {
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

        let response = if matches!(cmd, XBoardCommand::MoveNow) {
            self.stop_flag.store(true, Ordering::SeqCst);
            self.finish_thinking()
        } else {
            self.dispatch_command(cmd).map(|text| CommandResponse {
                text,
                ponder_move: None,
            })
        };
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
}

/// Entry point for `XBoard` mode.
pub fn run_xboard() {
    let mut handler = XBoardHandler::new();
    handler.run();
}

#[cfg(test)]
mod tests;
