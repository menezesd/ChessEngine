//! Protocol trait for chess engine communication.
//!
//! Defines a common interface for different chess protocols (UCI, `XBoard`).

use crate::board::SearchResult;

/// Result of processing a protocol command
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Command processed successfully, with optional output
    Ok(Option<String>),
    /// Engine should quit
    Quit,
    /// Command not recognized
    Unknown(String),
}

/// Trait for chess engine protocols (UCI, `XBoard`, etc.)
pub trait Protocol {
    /// Process a single command line and return the result
    fn process_command(&mut self, line: &str) -> CommandResult;

    /// Called when a search completes with results
    fn on_search_complete(&mut self, result: SearchResult);

    /// Get the protocol name (for logging/debugging)
    fn name(&self) -> &'static str;

    /// Run the protocol's main loop (blocking)
    fn run(&mut self);
}

/// Protocol identification based on first command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    Uci,
    XBoard,
    Unknown,
}

impl ProtocolType {
    /// Detect protocol from the first command
    #[must_use]
    pub fn detect(first_line: &str) -> Self {
        let trimmed = first_line.trim();
        if trimmed == "uci" || trimmed == "isready" {
            ProtocolType::Uci
        } else if trimmed == "xboard" || trimmed.starts_with("protover") {
            ProtocolType::XBoard
        } else {
            ProtocolType::Unknown
        }
    }
}
