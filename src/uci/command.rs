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

/// Parse the next parameter value as type T.
#[inline]
fn parse_next<T: std::str::FromStr>(parts: &[&str], i: usize) -> Option<T> {
    parts.get(i + 1).and_then(|v| v.parse::<T>().ok())
}

#[must_use]
pub fn parse_go_params(parts: &[&str]) -> GoParams {
    let mut params = GoParams::default();
    let mut i = 1;

    while i < parts.len() {
        let consumed = match parts[i] {
            // Time parameters (u64)
            "wtime" => {
                params.wtime = parse_next(parts, i);
                2
            }
            "btime" => {
                params.btime = parse_next(parts, i);
                2
            }
            "winc" => {
                params.winc = parse_next(parts, i);
                2
            }
            "binc" => {
                params.binc = parse_next(parts, i);
                2
            }
            "movetime" => {
                params.movetime = parse_next(parts, i);
                2
            }
            "movestogo" => {
                params.movestogo = parse_next(parts, i);
                2
            }
            "nodes" => {
                params.nodes = parse_next(parts, i);
                2
            }
            // Depth parameters (u32)
            "depth" => {
                params.depth = parse_next(parts, i);
                2
            }
            "mate" => {
                params.mate = parse_next(parts, i);
                2
            }
            // Flags
            "ponder" => {
                params.ponder = true;
                1
            }
            "infinite" => {
                params.infinite = true;
                1
            }
            // Unknown - skip
            _ => 1,
        };
        i += consumed;
    }
    params
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // parse_uci_command tests
    // ========================================================================

    #[test]
    fn parse_uci_command_uci() {
        let cmd = parse_uci_command("uci");
        assert!(matches!(cmd, Some(UciCommand::Uci)));
    }

    #[test]
    fn parse_uci_command_isready() {
        let cmd = parse_uci_command("isready");
        assert!(matches!(cmd, Some(UciCommand::IsReady)));
    }

    #[test]
    fn parse_uci_command_ucinewgame() {
        let cmd = parse_uci_command("ucinewgame");
        assert!(matches!(cmd, Some(UciCommand::UciNewGame)));
    }

    #[test]
    fn parse_uci_command_stop() {
        let cmd = parse_uci_command("stop");
        assert!(matches!(cmd, Some(UciCommand::Stop)));
    }

    #[test]
    fn parse_uci_command_quit() {
        let cmd = parse_uci_command("quit");
        assert!(matches!(cmd, Some(UciCommand::Quit)));
    }

    #[test]
    fn parse_uci_command_ponderhit() {
        let cmd = parse_uci_command("ponderhit");
        assert!(matches!(cmd, Some(UciCommand::PonderHit)));
    }

    #[test]
    fn parse_uci_command_position() {
        let cmd = parse_uci_command("position startpos moves e2e4 e7e5");
        match cmd {
            Some(UciCommand::Position(parts)) => {
                assert_eq!(parts.len(), 5);
                assert_eq!(parts[0], "position");
                assert_eq!(parts[1], "startpos");
                assert_eq!(parts[2], "moves");
                assert_eq!(parts[3], "e2e4");
                assert_eq!(parts[4], "e7e5");
            }
            _ => panic!("Expected Position command"),
        }
    }

    #[test]
    fn parse_uci_command_position_fen() {
        let cmd = parse_uci_command("position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        match cmd {
            Some(UciCommand::Position(parts)) => {
                assert_eq!(parts[0], "position");
                assert_eq!(parts[1], "fen");
                assert_eq!(parts[2], "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR");
            }
            _ => panic!("Expected Position command"),
        }
    }

    #[test]
    fn parse_uci_command_go() {
        let cmd = parse_uci_command("go depth 10");
        match cmd {
            Some(UciCommand::Go(parts)) => {
                assert_eq!(parts[0], "go");
                assert_eq!(parts[1], "depth");
                assert_eq!(parts[2], "10");
            }
            _ => panic!("Expected Go command"),
        }
    }

    #[test]
    fn parse_uci_command_go_complex() {
        let cmd = parse_uci_command("go wtime 300000 btime 300000 winc 3000 binc 3000");
        match cmd {
            Some(UciCommand::Go(parts)) => {
                assert_eq!(parts.len(), 9);
                assert_eq!(parts[0], "go");
            }
            _ => panic!("Expected Go command"),
        }
    }

    #[test]
    fn parse_uci_command_perft() {
        let cmd = parse_uci_command("perft 5");
        match cmd {
            Some(UciCommand::Perft(depth)) => {
                assert_eq!(depth, 5);
            }
            _ => panic!("Expected Perft command"),
        }
    }

    #[test]
    fn parse_uci_command_perft_default() {
        let cmd = parse_uci_command("perft");
        match cmd {
            Some(UciCommand::Perft(depth)) => {
                assert_eq!(depth, 1, "Default perft depth should be 1");
            }
            _ => panic!("Expected Perft command"),
        }
    }

    #[test]
    fn parse_uci_command_setoption() {
        let cmd = parse_uci_command("setoption name Hash value 256");
        match cmd {
            Some(UciCommand::SetOption(parts)) => {
                assert_eq!(parts[0], "setoption");
                assert_eq!(parts[1], "name");
                assert_eq!(parts[2], "Hash");
                assert_eq!(parts[3], "value");
                assert_eq!(parts[4], "256");
            }
            _ => panic!("Expected SetOption command"),
        }
    }

    #[test]
    fn parse_uci_command_debug_on() {
        let cmd = parse_uci_command("debug on");
        match cmd {
            Some(UciCommand::Debug(Some(val))) => {
                assert_eq!(val, "on");
            }
            _ => panic!("Expected Debug command"),
        }
    }

    #[test]
    fn parse_uci_command_debug_off() {
        let cmd = parse_uci_command("debug off");
        match cmd {
            Some(UciCommand::Debug(Some(val))) => {
                assert_eq!(val, "off");
            }
            _ => panic!("Expected Debug command"),
        }
    }

    #[test]
    fn parse_uci_command_debug_no_arg() {
        let cmd = parse_uci_command("debug");
        match cmd {
            Some(UciCommand::Debug(None)) => {}
            _ => panic!("Expected Debug command with no argument"),
        }
    }

    #[test]
    fn parse_uci_command_unknown() {
        let cmd = parse_uci_command("foobar");
        match cmd {
            Some(UciCommand::Unknown(s)) => {
                assert_eq!(s, "foobar");
            }
            _ => panic!("Expected Unknown command"),
        }
    }

    #[test]
    fn parse_uci_command_empty() {
        let cmd = parse_uci_command("");
        assert!(cmd.is_none());
    }

    #[test]
    fn parse_uci_command_whitespace_only() {
        let cmd = parse_uci_command("   \t  ");
        assert!(cmd.is_none());
    }

    #[test]
    fn parse_uci_command_with_leading_whitespace() {
        let cmd = parse_uci_command("  uci");
        assert!(matches!(cmd, Some(UciCommand::Uci)));
    }

    #[test]
    fn parse_uci_command_with_trailing_whitespace() {
        let cmd = parse_uci_command("uci  ");
        assert!(matches!(cmd, Some(UciCommand::Uci)));
    }

    #[test]
    fn parse_uci_command_case_sensitive() {
        // UCI commands are case-sensitive, "UCI" should be unknown
        let cmd = parse_uci_command("UCI");
        assert!(matches!(cmd, Some(UciCommand::Unknown(_))));
    }

    // ========================================================================
    // parse_go_params tests
    // ========================================================================

    #[test]
    fn parse_go_params_empty() {
        let parts: Vec<&str> = vec!["go"];
        let params = parse_go_params(&parts);

        assert!(params.wtime.is_none());
        assert!(params.btime.is_none());
        assert!(params.depth.is_none());
        assert!(!params.infinite);
        assert!(!params.ponder);
    }

    #[test]
    fn parse_go_params_depth() {
        let parts: Vec<&str> = vec!["go", "depth", "10"];
        let params = parse_go_params(&parts);

        assert_eq!(params.depth, Some(10));
    }

    #[test]
    fn parse_go_params_movetime() {
        let parts: Vec<&str> = vec!["go", "movetime", "5000"];
        let params = parse_go_params(&parts);

        assert_eq!(params.movetime, Some(5000));
    }

    #[test]
    fn parse_go_params_infinite() {
        let parts: Vec<&str> = vec!["go", "infinite"];
        let params = parse_go_params(&parts);

        assert!(params.infinite);
    }

    #[test]
    fn parse_go_params_ponder() {
        let parts: Vec<&str> = vec!["go", "ponder"];
        let params = parse_go_params(&parts);

        assert!(params.ponder);
    }

    #[test]
    fn parse_go_params_wtime_btime() {
        let parts: Vec<&str> = vec!["go", "wtime", "300000", "btime", "300000"];
        let params = parse_go_params(&parts);

        assert_eq!(params.wtime, Some(300000));
        assert_eq!(params.btime, Some(300000));
    }

    #[test]
    fn parse_go_params_with_increment() {
        let parts: Vec<&str> = vec!["go", "wtime", "300000", "btime", "300000", "winc", "3000", "binc", "3000"];
        let params = parse_go_params(&parts);

        assert_eq!(params.wtime, Some(300000));
        assert_eq!(params.btime, Some(300000));
        assert_eq!(params.winc, Some(3000));
        assert_eq!(params.binc, Some(3000));
    }

    #[test]
    fn parse_go_params_movestogo() {
        let parts: Vec<&str> = vec!["go", "wtime", "60000", "btime", "60000", "movestogo", "40"];
        let params = parse_go_params(&parts);

        assert_eq!(params.movestogo, Some(40));
    }

    #[test]
    fn parse_go_params_nodes() {
        let parts: Vec<&str> = vec!["go", "nodes", "1000000"];
        let params = parse_go_params(&parts);

        assert_eq!(params.nodes, Some(1000000));
    }

    #[test]
    fn parse_go_params_mate() {
        let parts: Vec<&str> = vec!["go", "mate", "3"];
        let params = parse_go_params(&parts);

        assert_eq!(params.mate, Some(3));
    }

    #[test]
    fn parse_go_params_complex() {
        let parts: Vec<&str> = vec![
            "go", "wtime", "300000", "btime", "300000", "winc", "3000", "binc", "3000", "depth", "20",
        ];
        let params = parse_go_params(&parts);

        assert_eq!(params.wtime, Some(300000));
        assert_eq!(params.btime, Some(300000));
        assert_eq!(params.winc, Some(3000));
        assert_eq!(params.binc, Some(3000));
        assert_eq!(params.depth, Some(20));
    }

    #[test]
    fn parse_go_params_invalid_value() {
        let parts: Vec<&str> = vec!["go", "depth", "invalid"];
        let params = parse_go_params(&parts);

        // Invalid value should result in None
        assert!(params.depth.is_none());
    }

    #[test]
    fn parse_go_params_missing_value() {
        let parts: Vec<&str> = vec!["go", "depth"];
        let params = parse_go_params(&parts);

        // Missing value should result in None
        assert!(params.depth.is_none());
    }

    #[test]
    fn parse_go_params_unknown_skipped() {
        let parts: Vec<&str> = vec!["go", "unknownparam", "depth", "10"];
        let params = parse_go_params(&parts);

        // Unknown param should be skipped, depth should still be parsed
        assert_eq!(params.depth, Some(10));
    }
}
