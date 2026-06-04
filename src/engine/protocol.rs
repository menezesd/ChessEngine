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
        match trimmed.split_whitespace().next() {
            Some("uci" | "isready") => ProtocolType::Uci,
            Some("xboard" | "protover") => ProtocolType::XBoard,
            _ => ProtocolType::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProtocolType;

    #[test]
    fn detects_uci_commands() {
        assert_eq!(ProtocolType::detect("uci"), ProtocolType::Uci);
        assert_eq!(ProtocolType::detect(" isready "), ProtocolType::Uci);
    }

    #[test]
    fn detects_xboard_commands() {
        assert_eq!(ProtocolType::detect("xboard"), ProtocolType::XBoard);
        assert_eq!(ProtocolType::detect("protover 2"), ProtocolType::XBoard);
    }

    #[test]
    fn rejects_partial_protocol_command_tokens() {
        assert_eq!(ProtocolType::detect("protover2"), ProtocolType::Unknown);
        assert_eq!(ProtocolType::detect("xboard2"), ProtocolType::Unknown);
    }

    #[test]
    fn unknown_for_unrecognized_first_line() {
        assert_eq!(
            ProtocolType::detect("position startpos"),
            ProtocolType::Unknown
        );
    }
}
