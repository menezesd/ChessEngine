#[derive(Debug, Clone)]
pub enum UciCommand {
    Uci,
    IsReady,
    UciNewGame,
    Position(Vec<String>),
    Go(Vec<String>),
    Perft(usize),
    SetOption(Vec<String>),
    Debug(Option<String>),
    Stop,
    PonderHit,
    Quit,
    Unknown(String),
}

pub fn parse_uci_command(line: &str) -> Option<UciCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let owned_parts = || parts.iter().map(|p| (*p).to_string()).collect::<Vec<String>>();

    let cmd = match parts[0] {
        "uci" => UciCommand::Uci,
        "isready" => UciCommand::IsReady,
        "ucinewgame" => UciCommand::UciNewGame,
        "position" => UciCommand::Position(owned_parts()),
        "go" => UciCommand::Go(owned_parts()),
        "perft" => {
            let depth = parts.get(1).and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
            UciCommand::Perft(depth)
        }
        "setoption" => UciCommand::SetOption(owned_parts()),
        "debug" => UciCommand::Debug(parts.get(1).map(|v| (*v).to_string())),
        "stop" => UciCommand::Stop,
        "ponderhit" => UciCommand::PonderHit,
        "quit" => UciCommand::Quit,
        _ => UciCommand::Unknown(trimmed.to_string()),
    };

    Some(cmd)
}
