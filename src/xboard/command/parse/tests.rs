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
fn test_usermove_missing_move_is_unknown() {
    match parse_xboard_command("usermove") {
        Some(XBoardCommand::Unknown(command)) => assert_eq!(command, "usermove"),
        _ => panic!("Expected malformed usermove to remain unknown"),
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
fn test_malformed_edit_piece_is_unknown() {
    match parse_xboard_command("Pa2extra") {
        Some(XBoardCommand::Unknown(command)) => assert_eq!(command, "Pa2extra"),
        _ => panic!("Expected malformed edit-piece command to remain unknown"),
    }
}

#[test]
fn test_edit_color_toggle_is_bare_command() {
    // CECP edit mode: "c" alone toggles the color; it takes no argument.
    assert!(matches!(
        parse_xboard_command("c"),
        Some(XBoardCommand::EditColor)
    ));
}

#[test]
fn test_setboard() {
    match parse_xboard_command("setboard rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    {
        Some(XBoardCommand::SetBoard(fen)) => {
            assert!(fen.starts_with("rnbqkbnr"));
        }
        _ => panic!("Expected SetBoard"),
    }
}

#[test]
fn test_setboard_missing_fen_is_unknown() {
    match parse_xboard_command("setboard") {
        Some(XBoardCommand::Unknown(command)) => assert_eq!(command, "setboard"),
        _ => panic!("Expected malformed setboard to remain unknown"),
    }
}

#[test]
fn test_result() {
    match parse_xboard_command("result 1-0 {White mates}") {
        Some(XBoardCommand::Result(result)) => assert_eq!(result, "1-0 {White mates}"),
        _ => panic!("Expected Result"),
    }
}

#[test]
fn test_name() {
    match parse_xboard_command("name Example Opponent") {
        Some(XBoardCommand::Name(name)) => assert_eq!(name, "Example Opponent"),
        _ => panic!("Expected Name"),
    }
}

#[test]
fn test_level() {
    match parse_xboard_command("level 40 5:30 2") {
        Some(XBoardCommand::Level {
            moves_per_session,
            base_seconds,
            increment_seconds,
        }) => {
            assert_eq!(moves_per_session, 40);
            assert_eq!(base_seconds, 330);
            assert_eq!(increment_seconds, 2);
        }
        _ => panic!("Expected Level"),
    }
}

#[test]
fn test_level_malformed_values_are_unknown() {
    match parse_xboard_command("level nope bad also_bad") {
        Some(XBoardCommand::Unknown(command)) => assert_eq!(command, "level nope bad also_bad"),
        _ => panic!("Expected malformed level to remain unknown"),
    }
}

#[test]
fn test_level_missing_values_are_unknown() {
    match parse_xboard_command("level 40 5") {
        Some(XBoardCommand::Unknown(command)) => assert_eq!(command, "level 40 5"),
        _ => panic!("Expected incomplete level to remain unknown"),
    }
}

#[test]
fn test_parse_time_control() {
    assert_eq!(parse_time_control("5"), Some(300));
    assert_eq!(parse_time_control("10"), Some(600));

    assert_eq!(parse_time_control("5:30"), Some(330));
    assert_eq!(parse_time_control("0:30"), Some(30));
    assert_eq!(parse_time_control("1:00"), Some(60));

    assert_eq!(parse_time_control("abc"), None);
    assert_eq!(parse_time_control("5:abc"), None);
    assert_eq!(parse_time_control("999999999999"), None);
}
