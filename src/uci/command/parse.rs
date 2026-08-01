use super::{parse_go_params, UciCommand};

const DEFAULT_PERFT_DEPTH: usize = 1;

fn parse_perft_depth(parts: &[&str]) -> usize {
    let depth = match parts.get(1) {
        Some(&"depth") => parts.get(2),
        value => value,
    };

    depth
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PERFT_DEPTH)
}

/// Parse a UCI command from a line of input.
#[must_use]
pub fn parse_uci_command(line: &str) -> Option<UciCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let cmd = match parts[0] {
        "uci" => UciCommand::Uci,
        "isready" => UciCommand::IsReady,
        "ucinewgame" => UciCommand::UciNewGame,
        "position" => UciCommand::Position(trimmed.to_string()),
        "go" => UciCommand::Go(parse_go_params(&parts)),
        "eval" => UciCommand::Eval,
        "evalfeatures" => UciCommand::EvalFeatures,
        "perft" => UciCommand::Perft(parse_perft_depth(&parts)),
        "setoption" => {
            let parsed = crate::uci::options::parse_setoption(&parts);
            if let Some((name, value)) = parsed {
                UciCommand::SetOption { name, value }
            } else {
                UciCommand::Unknown(trimmed.to_string())
            }
        }
        "debug" => UciCommand::Debug(parts.get(1).map(|v| (*v).to_string())),
        "stop" => UciCommand::Stop,
        "ponderhit" => UciCommand::PonderHit,
        "quit" => UciCommand::Quit,
        _ => UciCommand::Unknown(trimmed.to_string()),
    };

    Some(cmd)
}

#[cfg(test)]
mod tests;
