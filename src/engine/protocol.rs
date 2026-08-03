//! Protocol auto-detection for chess engine communication.

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
