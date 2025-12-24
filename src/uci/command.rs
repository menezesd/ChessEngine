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

#[derive(Default, Debug, Clone)]
pub struct GoParams {
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub movetime: Option<u64>,
    pub movestogo: Option<u64>,
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub mate: Option<u32>,
    pub ponder: bool,
    pub infinite: bool,
}

pub fn parse_go_params(parts: &[&str]) -> GoParams {
    let mut params = GoParams::default();
    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "wtime" => {
                params.wtime = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "btime" => {
                params.btime = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "winc" => {
                params.winc = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "binc" => {
                params.binc = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "movetime" => {
                params.movetime = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "movestogo" => {
                params.movestogo = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "depth" => {
                params.depth = parts.get(i + 1).and_then(|v| v.parse::<u32>().ok());
                i += 2;
            }
            "nodes" => {
                params.nodes = parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
                i += 2;
            }
            "mate" => {
                params.mate = parts.get(i + 1).and_then(|v| v.parse::<u32>().ok());
                i += 2;
            }
            "ponder" => {
                params.ponder = true;
                i += 1;
            }
            "infinite" => {
                params.infinite = true;
                i += 1;
            }
            _ => i += 1,
        }
    }
    params
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

    let owned_parts = || {
        parts
            .iter()
            .map(|p| (*p).to_string())
            .collect::<Vec<String>>()
    };

    let cmd = match parts[0] {
        "uci" => UciCommand::Uci,
        "isready" => UciCommand::IsReady,
        "ucinewgame" => UciCommand::UciNewGame,
        "position" => UciCommand::Position(owned_parts()),
        "go" => UciCommand::Go(owned_parts()),
        "perft" => {
            let depth = parts
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1);
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
