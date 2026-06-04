use super::*;

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
        Some(UciCommand::Position(line)) => {
            assert_eq!(line, "position startpos moves e2e4 e7e5");
        }
        _ => panic!("Expected Position command"),
    }
}

#[test]
fn parse_uci_command_position_fen() {
    let cmd =
        parse_uci_command("position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    match cmd {
        Some(UciCommand::Position(line)) => {
            assert_eq!(
                line,
                "position fen rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
            );
        }
        _ => panic!("Expected Position command"),
    }
}

#[test]
fn parse_uci_command_go() {
    let cmd = parse_uci_command("go depth 10");
    match cmd {
        Some(UciCommand::Go(params)) => {
            assert_eq!(params.depth, Some(10));
        }
        _ => panic!("Expected Go command"),
    }
}

#[test]
fn parse_uci_command_go_complex() {
    let cmd = parse_uci_command("go wtime 300000 btime 300000 winc 3000 binc 3000");
    match cmd {
        Some(UciCommand::Go(params)) => {
            assert_eq!(params.wtime, Some(300000));
            assert_eq!(params.btime, Some(300000));
            assert_eq!(params.winc, Some(3000));
            assert_eq!(params.binc, Some(3000));
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
fn parse_uci_command_perft_invalid_uses_default() {
    let cmd = parse_uci_command("perft nope");
    match cmd {
        Some(UciCommand::Perft(depth)) => {
            assert_eq!(depth, 1, "Invalid perft depth should default to 1");
        }
        _ => panic!("Expected Perft command"),
    }
}

#[test]
fn parse_uci_command_setoption() {
    let cmd = parse_uci_command("setoption name Hash value 256");
    match cmd {
        Some(UciCommand::SetOption { name, value }) => {
            assert_eq!(name, "Hash");
            assert_eq!(value.as_deref(), Some("256"));
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
    let cmd = parse_uci_command("UCI");
    assert!(matches!(cmd, Some(UciCommand::Unknown(_))));
}
