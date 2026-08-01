use super::XBoardCommand;

struct CommandParts<'a> {
    parts: Vec<&'a str>,
}

impl<'a> CommandParts<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            parts: line.split_whitespace().collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    fn first(&self) -> &'a str {
        self.parts[0]
    }

    fn get(&self, index: usize) -> Option<&'a str> {
        self.parts.get(index).copied()
    }

    fn parsed<T: std::str::FromStr>(&self, index: usize) -> Option<T> {
        self.get(index).and_then(|value| value.parse().ok())
    }

    fn string(&self, index: usize) -> Option<String> {
        self.get(index).map(str::to_string)
    }

    fn trailing_text(&self, start: usize) -> String {
        self.parts[start..].join(" ")
    }
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

fn parse_arg_command(parts: &CommandParts<'_>) -> Option<XBoardCommand> {
    parts.get(1)?;
    match parts.first() {
        "protover" => parts.parsed(1).map(XBoardCommand::Protover),
        "accepted" => parts.string(1).map(XBoardCommand::Accepted),
        "rejected" => parts.string(1).map(XBoardCommand::Rejected),
        "time" => parts.parsed(1).map(XBoardCommand::Time),
        "otim" => parts.parsed(1).map(XBoardCommand::OTime),
        "st" => parts.parsed(1).map(XBoardCommand::St),
        "sd" => parts.parsed(1).map(XBoardCommand::Sd),
        "ping" => parts.parsed(1).map(XBoardCommand::Ping),
        "memory" => parts.parsed(1).map(XBoardCommand::Memory),
        "cores" => parts.parsed(1).map(XBoardCommand::Cores),
        "c" => parts
            .get(1)
            .and_then(|v| v.chars().next())
            .map(XBoardCommand::EditColor),
        _ => None,
    }
}

fn parse_complex_command(trimmed: &str, parts: &CommandParts<'_>) -> XBoardCommand {
    match parts.first() {
        "setboard" => {
            let fen = parts.trailing_text(1);
            if fen.is_empty() {
                XBoardCommand::Unknown(trimmed.to_string())
            } else {
                XBoardCommand::SetBoard(fen)
            }
        }
        "usermove" => {
            if let Some(mv) = parts.string(1) {
                XBoardCommand::UserMove(mv)
            } else {
                XBoardCommand::Unknown(trimmed.to_string())
            }
        }
        "level" => parse_level_command(parts)
            .unwrap_or_else(|| XBoardCommand::Unknown(trimmed.to_string())),
        "result" => {
            let result = parts.trailing_text(1);
            XBoardCommand::Result(result)
        }
        "name" => {
            let name = parts.trailing_text(1);
            XBoardCommand::Name(name)
        }
        _ => {
            let command = parts.first();
            if is_likely_move(command) {
                XBoardCommand::UserMove(command.to_string())
            } else if is_edit_piece(command) {
                XBoardCommand::EditPiece(command.to_string())
            } else {
                XBoardCommand::Unknown(trimmed.to_string())
            }
        }
    }
}

fn parse_level_command(parts: &CommandParts<'_>) -> Option<XBoardCommand> {
    let moves_per_session = parts.parsed(1)?;
    let base_seconds = parts.get(2).and_then(parse_time_control)?;
    let increment_seconds = parts.parsed(3)?;

    Some(XBoardCommand::Level {
        moves_per_session,
        base_seconds,
        increment_seconds,
    })
}

fn is_edit_piece(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 3
        && matches!(bytes[0], b'P' | b'N' | b'B' | b'R' | b'Q' | b'K' | b'x')
        && matches!(bytes[1], b'a'..=b'h')
        && matches!(bytes[2], b'1'..=b'8')
}

/// Parse an `XBoard` command from a line of input.
#[must_use]
pub fn parse_xboard_command(line: &str) -> Option<XBoardCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts = CommandParts::new(trimmed);
    if parts.is_empty() {
        return None;
    }

    if let Some(cmd) = parse_basic_command(parts.first()) {
        return Some(cmd);
    }

    if let Some(cmd) = parse_arg_command(&parts) {
        return Some(cmd);
    }

    Some(parse_complex_command(trimmed, &parts))
}

fn is_likely_move(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    if s == "O-O" || s == "O-O-O" || s == "0-0" || s == "0-0-0" {
        return true;
    }

    s.chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || "NBRQK".contains(first))
}

/// Parse time control string (supports "5" or "5:30" format).
/// Returns total seconds.
fn parse_time_control(s: &str) -> Option<u32> {
    if let Some((mins, secs)) = s.split_once(':') {
        let mins: u32 = mins.parse().ok()?;
        let secs: u32 = secs.parse().ok()?;
        mins.checked_mul(60)?.checked_add(secs)
    } else {
        let mins: u32 = s.parse().ok()?;
        mins.checked_mul(60)
    }
}

#[cfg(test)]
mod tests;
