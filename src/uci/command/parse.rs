use super::{go::parse_go_tokens, UciCommand};

const DEFAULT_PERFT_DEPTH: usize = 1;

fn parse_perft_depth(mut args: std::str::SplitWhitespace<'_>) -> usize {
    let depth = match args.next() {
        Some("depth") => args.next(),
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
    let mut words = trimmed.split_whitespace();

    let cmd = match words.next()? {
        "uci" => UciCommand::Uci,
        "isready" => UciCommand::IsReady,
        "ucinewgame" => UciCommand::UciNewGame,
        "position" => UciCommand::Position(trimmed.to_string()),
        "go" => UciCommand::Go(parse_go_tokens(words)),
        "eval" => UciCommand::Eval,
        "evalfeatures" => UciCommand::EvalFeatures,
        "perft" => UciCommand::Perft(parse_perft_depth(words)),
        "setoption" => {
            let parsed = crate::uci::options::parse_setoption_line(line);
            if let Some((name, value)) = parsed {
                UciCommand::SetOption { name, value }
            } else {
                UciCommand::Unknown(trimmed.to_string())
            }
        }
        "debug" => UciCommand::Debug(words.next().map(str::to_owned)),
        "stop" => UciCommand::Stop,
        "ponderhit" => UciCommand::PonderHit,
        "quit" => UciCommand::Quit,
        _ => UciCommand::Unknown(trimmed.to_string()),
    };

    Some(cmd)
}

#[cfg(test)]
mod tests;
