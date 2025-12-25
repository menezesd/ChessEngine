//! XBoard/WinBoard protocol command parsing.

/// `XBoard` protocol commands
#[derive(Debug, Clone)]
pub enum XBoardCommand {
    /// Enter `XBoard` mode
    XBoard,
    /// Protocol version negotiation
    Protover(u32),
    /// Acknowledge features accepted
    Accepted(String),
    /// Acknowledge features rejected
    Rejected(String),
    /// Start new game
    New,
    /// Set position from FEN
    SetBoard(String),
    /// Opponent's move (in SAN or coordinate notation)
    UserMove(String),
    /// Start thinking
    Go,
    /// Enter force mode (make moves without thinking)
    Force,
    /// Play the color to move
    PlayOther,
    /// Set computer's color
    White,
    Black,
    /// Time remaining for engine (centiseconds)
    Time(u64),
    /// Time remaining for opponent (centiseconds)
    OTime(u64),
    /// Set time control: level <mps> <base> <inc>
    Level {
        moves_per_session: u32,
        base_minutes: u32,
        increment_seconds: u32,
    },
    /// Set exact seconds per move
    St(u32),
    /// Set max depth
    Sd(u32),
    /// Move immediately
    MoveNow,
    /// Ping/pong for keepalive
    Ping(u32),
    /// Undo last move
    Undo,
    /// Remove last two half-moves (one for each side)
    Remove,
    /// Game result
    Result(String),
    /// Hint request
    Hint,
    /// Opponent offers a draw
    Draw,
    /// Set board (edit mode commands)
    Edit,
    /// Exit edit mode
    EditDone,
    /// Clear board in edit mode
    ClearBoard,
    /// Place piece in edit mode (e.g., "Pa2")
    EditPiece(String),
    /// Set side to move in edit mode
    EditColor(char),
    /// Computer plays itself
    Computer,
    /// Set random mode
    Random,
    /// Post thinking output
    Post,
    /// Don't post thinking output
    NoPost,
    /// Enable pondering
    Hard,
    /// Disable pondering
    Easy,
    /// Set opponent's name
    Name(String),
    /// Set memory/hash size in MB
    Memory(u32),
    /// Set number of cores
    Cores(u32),
    /// Analyze mode
    Analyze,
    /// Exit analyze mode
    ExitAnalyze,
    /// Quit the program
    Quit,
    /// Unknown command
    Unknown(String),
}

/// Parse an `XBoard` command from a line of input
#[must_use]
pub fn parse_xboard_command(line: &str) -> Option<XBoardCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let cmd = match parts[0] {
        "xboard" => XBoardCommand::XBoard,
        "protover" => {
            let version = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(1);
            XBoardCommand::Protover(version)
        }
        "accepted" => {
            let feature = parts.get(1).map_or(String::new(), |s| (*s).to_string());
            XBoardCommand::Accepted(feature)
        }
        "rejected" => {
            let feature = parts.get(1).map_or(String::new(), |s| (*s).to_string());
            XBoardCommand::Rejected(feature)
        }
        "new" => XBoardCommand::New,
        "setboard" => {
            // Collect the rest as FEN
            let fen = parts[1..].join(" ");
            XBoardCommand::SetBoard(fen)
        }
        "usermove" => {
            let mv = parts.get(1).map_or(String::new(), |s| (*s).to_string());
            XBoardCommand::UserMove(mv)
        }
        "go" => XBoardCommand::Go,
        "force" => XBoardCommand::Force,
        "playother" => XBoardCommand::PlayOther,
        "white" => XBoardCommand::White,
        "black" => XBoardCommand::Black,
        "time" => {
            let cs = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            XBoardCommand::Time(cs)
        }
        "otim" => {
            let cs = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            XBoardCommand::OTime(cs)
        }
        "level" => {
            let mps = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            let base = parts.get(2).and_then(|v| parse_time_control(v)).unwrap_or(0);
            let inc = parts.get(3).and_then(|v| v.parse().ok()).unwrap_or(0);
            XBoardCommand::Level {
                moves_per_session: mps,
                base_minutes: base,
                increment_seconds: inc,
            }
        }
        "st" => {
            let secs = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            XBoardCommand::St(secs)
        }
        "sd" => {
            let depth = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(64);
            XBoardCommand::Sd(depth)
        }
        "?" => XBoardCommand::MoveNow,
        "ping" => {
            let n = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            XBoardCommand::Ping(n)
        }
        "undo" => XBoardCommand::Undo,
        "remove" => XBoardCommand::Remove,
        "result" => {
            let result = parts[1..].join(" ");
            XBoardCommand::Result(result)
        }
        "hint" => XBoardCommand::Hint,
        "draw" => XBoardCommand::Draw,
        "edit" => XBoardCommand::Edit,
        "." => XBoardCommand::EditDone,
        "#" => XBoardCommand::ClearBoard,
        "c" => XBoardCommand::EditColor('b'),
        "computer" => XBoardCommand::Computer,
        "random" => XBoardCommand::Random,
        "post" => XBoardCommand::Post,
        "nopost" => XBoardCommand::NoPost,
        "hard" => XBoardCommand::Hard,
        "easy" => XBoardCommand::Easy,
        "name" => {
            let name = parts[1..].join(" ");
            XBoardCommand::Name(name)
        }
        "memory" => {
            let mb = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(64);
            XBoardCommand::Memory(mb)
        }
        "cores" => {
            let n = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(1);
            XBoardCommand::Cores(n)
        }
        "analyze" => XBoardCommand::Analyze,
        "exit" => XBoardCommand::ExitAnalyze,
        "quit" => XBoardCommand::Quit,
        _ => {
            // Check if it's a move (starts with lowercase letter or is "O-O")
            if is_likely_move(parts[0]) {
                XBoardCommand::UserMove(parts[0].to_string())
            } else if parts[0].len() >= 2 && parts[0].chars().next().unwrap().is_ascii_uppercase() {
                // Edit mode piece placement (e.g., "Pa2")
                XBoardCommand::EditPiece(parts[0].to_string())
            } else {
                XBoardCommand::Unknown(trimmed.to_string())
            }
        }
    };

    Some(cmd)
}

/// Check if a string looks like a chess move
fn is_likely_move(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Castling
    if s == "O-O" || s == "O-O-O" || s == "0-0" || s == "0-0-0" {
        return true;
    }

    let first = s.chars().next().unwrap();

    // SAN move starting with piece or file
    if first.is_ascii_lowercase() || "NBRQK".contains(first) {
        return true;
    }

    false
}

/// Parse time control string (supports "5" or "5:30" format)
fn parse_time_control(s: &str) -> Option<u32> {
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        let mins: u32 = parts.first()?.parse().ok()?;
        let secs: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        Some(mins + secs / 60)
    } else {
        s.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_commands() {
        assert!(matches!(parse_xboard_command("xboard"), Some(XBoardCommand::XBoard)));
        assert!(matches!(parse_xboard_command("new"), Some(XBoardCommand::New)));
        assert!(matches!(parse_xboard_command("quit"), Some(XBoardCommand::Quit)));
        assert!(matches!(parse_xboard_command("go"), Some(XBoardCommand::Go)));
        assert!(matches!(parse_xboard_command("force"), Some(XBoardCommand::Force)));
    }

    #[test]
    fn test_protover() {
        match parse_xboard_command("protover 2") {
            Some(XBoardCommand::Protover(2)) => {}
            _ => panic!("Expected Protover(2)"),
        }
    }

    #[test]
    fn test_time() {
        match parse_xboard_command("time 6000") {
            Some(XBoardCommand::Time(6000)) => {}
            _ => panic!("Expected Time(6000)"),
        }
    }

    #[test]
    fn test_usermove() {
        match parse_xboard_command("usermove e2e4") {
            Some(XBoardCommand::UserMove(m)) => assert_eq!(m, "e2e4"),
            _ => panic!("Expected UserMove"),
        }
    }

    #[test]
    fn test_san_move() {
        match parse_xboard_command("Nf3") {
            Some(XBoardCommand::UserMove(m)) => assert_eq!(m, "Nf3"),
            _ => panic!("Expected UserMove from SAN"),
        }
    }

    #[test]
    fn test_setboard() {
        match parse_xboard_command("setboard rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1") {
            Some(XBoardCommand::SetBoard(fen)) => {
                assert!(fen.starts_with("rnbqkbnr"));
            }
            _ => panic!("Expected SetBoard"),
        }
    }
}
