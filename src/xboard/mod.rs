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

impl XBoardHandler {
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

                if let Some(output) = self.play_engine_turn() {
                    writeln!(stdout, "{output}").ok();
                    stdout.flush().ok();
                }
            }
        }
    }

    fn play_engine_turn(&mut self) -> Option<String> {
        if !self.should_think() {
            return None;
        }

        let result = self.think();
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
}

/// Entry point for `XBoard` mode.
pub fn run_xboard() {
    let mut handler = XBoardHandler::new();
    handler.run();
}

#[cfg(test)]
mod tests;
