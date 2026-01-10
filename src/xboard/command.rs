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
    /// Pause thinking
    Pause,
    /// Resume thinking
    Resume,
    /// Quit the program
    Quit,
    /// Unknown command
    Unknown(String),
}

fn parse_basic_command(cmd_str: &str) -> Option<XBoardCommand> {
    match cmd_str {
        "xboard" => Some(XBoardCommand::XBoard),
        "new" => Some(XBoardCommand::New),
        "go" => Some(XBoardCommand::Go),
        "force" => Some(XBoardCommand::Force),
        "playother" => Some(XBoardCommand::PlayOther),
        "white" => Some(XBoardCommand::White),
        "black" => Some(XBoardCommand::Black),
        "undo" => Some(XBoardCommand::Undo),
        "remove" => Some(XBoardCommand::Remove),
        "hint" => Some(XBoardCommand::Hint),
        "draw" => Some(XBoardCommand::Draw),
        "edit" => Some(XBoardCommand::Edit),
        "." => Some(XBoardCommand::EditDone),
        "#" => Some(XBoardCommand::ClearBoard),
        "computer" => Some(XBoardCommand::Computer),
        "random" => Some(XBoardCommand::Random),
        "post" => Some(XBoardCommand::Post),
        "nopost" => Some(XBoardCommand::NoPost),
        "hard" => Some(XBoardCommand::Hard),
        "easy" => Some(XBoardCommand::Easy),
        "analyze" => Some(XBoardCommand::Analyze),
        "exit" => Some(XBoardCommand::ExitAnalyze),
        "pause" => Some(XBoardCommand::Pause),
        "resume" => Some(XBoardCommand::Resume),
        "quit" => Some(XBoardCommand::Quit),
        "?" => Some(XBoardCommand::MoveNow),
        _ => None,
    }
}

fn parse_arg_command(parts: &[&str]) -> Option<XBoardCommand> {
    if parts.len() < 2 {
        return None; // Command requires an argument but none is present
    }
    match parts[0] {
        "protover" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::Protover),
        "accepted" => parts
            .get(1)
            .map(|s| XBoardCommand::Accepted((*s).to_string())),
        "rejected" => parts
            .get(1)
            .map(|s| XBoardCommand::Rejected((*s).to_string())),
        "time" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::Time),
        "otim" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::OTime),
        "st" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::St),
        "sd" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::Sd),
        "ping" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::Ping),
        "memory" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::Memory),
        "cores" => parts
            .get(1)
            .and_then(|v| v.parse().ok())
            .map(XBoardCommand::Cores),
        "c" => parts
            .get(1)
            .and_then(|v| v.chars().next())
            .map(XBoardCommand::EditColor),
        _ => None,
    }
}

fn parse_complex_command(trimmed: &str, parts: &[&str]) -> XBoardCommand {
    match parts[0] {
        "setboard" => {
            let fen = parts[1..].join(" ");
            XBoardCommand::SetBoard(fen)
        }
        "usermove" => {
            let mv = parts.get(1).map_or(String::new(), |s| (*s).to_string());
            XBoardCommand::UserMove(mv)
        }
        "level" => {
            let mps = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            let base = parts
                .get(2)
                .and_then(|v| parse_time_control(v))
                .unwrap_or(0);
            let inc = parts.get(3).and_then(|v| v.parse().ok()).unwrap_or(0);
            XBoardCommand::Level {
                moves_per_session: mps,
                base_minutes: base,
                increment_seconds: inc,
            }
        }
        "result" => {
            let result = parts[1..].join(" ");
            XBoardCommand::Result(result)
        }
        "name" => {
            let name = parts[1..].join(" ");
            XBoardCommand::Name(name)
        }
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
    }
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

    if let Some(cmd) = parse_basic_command(parts[0]) {
        return Some(cmd);
    }

    if let Some(cmd) = parse_arg_command(&parts) {
        return Some(cmd);
    }

    // Now call the complex command parser
    Some(parse_complex_command(trimmed, &parts))
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
        assert!(matches!(
            parse_xboard_command("xboard"),
            Some(XBoardCommand::XBoard)
        ));
        assert!(matches!(
            parse_xboard_command("new"),
            Some(XBoardCommand::New)
        ));
        assert!(matches!(
            parse_xboard_command("quit"),
            Some(XBoardCommand::Quit)
        ));
        assert!(matches!(
            parse_xboard_command("go"),
            Some(XBoardCommand::Go)
        ));
        assert!(matches!(
            parse_xboard_command("force"),
            Some(XBoardCommand::Force)
        ));
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
        match parse_xboard_command(
            "setboard rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ) {
            Some(XBoardCommand::SetBoard(fen)) => {
                assert!(fen.starts_with("rnbqkbnr"));
            }
            _ => panic!("Expected SetBoard"),
        }
    }
}
