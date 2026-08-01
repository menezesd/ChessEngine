use super::handler::{normalized_memory_mb, normalized_search_depth, seconds_to_centiseconds};
use super::*;
use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::board::{Color, Piece, Square, DEFAULT_TT_MB};
use crate::engine::time::TimeControl;
use crate::xboard::command::parse_xboard_command;

#[test]
fn test_default_search_depth_matches_search_default() {
    let handler = XBoardHandler::new();
    assert_eq!(handler.max_depth, DEFAULT_MAX_DEPTH);
}

#[test]
fn test_st_command_uses_seconds_per_move() {
    let mut handler = XBoardHandler::new();

    handler.handle_command(&XBoardCommand::St(5));

    assert_eq!(handler.time_control(), TimeControl::move_time_ms(5_000));
}

#[test]
fn test_level_command_initializes_clock_from_base_time() {
    let mut handler = XBoardHandler::new();

    handler.handle_command(&XBoardCommand::Level {
        moves_per_session: 40,
        base_seconds: 300,
        increment_seconds: 2,
    });

    assert_eq!(
        handler.time_control(),
        TimeControl::from_xboard_time(30_000, 2, Some(40))
    );
}

#[test]
fn test_seconds_to_centiseconds_converts_large_values() {
    assert_eq!(seconds_to_centiseconds(300), 30_000);
    assert_eq!(seconds_to_centiseconds(u32::MAX), u64::from(u32::MAX) * 100);
}

#[test]
fn test_memory_command_normalization_clamps_to_safe_range() {
    assert_eq!(normalized_memory_mb(0), 1);
    assert_eq!(normalized_memory_mb(64), 64);
    assert_eq!(normalized_memory_mb(u32::MAX), DEFAULT_TT_MB);
}

#[test]
fn test_sd_command_normalization_clamps_to_search_default() {
    assert_eq!(normalized_search_depth(1), 1);
    assert_eq!(
        normalized_search_depth(DEFAULT_MAX_DEPTH),
        DEFAULT_MAX_DEPTH
    );
    assert_eq!(normalized_search_depth(u32::MAX), DEFAULT_MAX_DEPTH);
}

#[test]
fn test_sd_command_clamps_to_search_default() {
    let mut handler = XBoardHandler::new();

    handler.handle_command(&XBoardCommand::Sd(u32::MAX));

    assert_eq!(handler.max_depth, DEFAULT_MAX_DEPTH);
}

#[test]
fn test_new_command() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::New);
    assert!(!handler.force_mode);
    assert_eq!(handler.engine_color, Some(Color::Black));
}

#[test]
fn test_force_command() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Force);
    assert!(handler.force_mode);
}

#[test]
fn test_usermove() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Force);
    let result = handler.handle_command(&XBoardCommand::UserMove("e4".to_string()));
    assert!(result.is_none());
}

#[test]
fn test_protover() {
    let mut handler = XBoardHandler::new();
    let result = handler.handle_command(&XBoardCommand::Protover(2));
    assert!(result.is_some());
    let features = result.unwrap();
    assert!(features.contains("setboard=1"));
}

#[test]
fn test_ping_pong() {
    let mut handler = XBoardHandler::new();
    let result = handler.handle_command(&XBoardCommand::Ping(42));
    assert_eq!(result, Some("pong 42".to_string()));
}

#[test]
fn test_setboard() {
    let mut handler = XBoardHandler::new();
    let result = handler.handle_command(&XBoardCommand::SetBoard(
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
    ));
    assert!(result.is_none());
    assert!(!handler.board.white_to_move());
}

#[test]
fn test_pause_resume() {
    let mut handler = XBoardHandler::new();
    assert!(!handler.paused);
    handler.handle_command(&XBoardCommand::Pause);
    assert!(handler.paused);
    handler.handle_command(&XBoardCommand::Resume);
    assert!(!handler.paused);
}

#[test]
fn test_edit_mode() {
    let mut handler = XBoardHandler::new();
    assert!(!handler.edit_mode);
    handler.handle_command(&XBoardCommand::Edit);
    assert!(handler.edit_mode);
    handler.handle_command(&XBoardCommand::EditDone);
    assert!(!handler.edit_mode);
}

#[test]
fn test_edit_mode_accepts_san_like_piece_placement_and_removal() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Edit);

    // `Ke1` is parsed as a normal-looking move, but edit mode must treat it
    // as a placement command.
    let place = parse_xboard_command("Ke1").expect("edit placement command should parse");
    handler.handle_command(&place);
    assert_eq!(handler.board.piece_on(Square::new(0, 4)), Some(Piece::King));

    let remove = parse_xboard_command("xe1").expect("edit removal command should parse");
    handler.handle_command(&remove);
    assert_eq!(handler.board.piece_on(Square::new(0, 4)), None);
}

#[test]
fn test_edit_mode_discards_prior_undo_history() {
    let mut handler = XBoardHandler::new();
    let mv = handler.board.parse_move("e2e4").unwrap();
    let info = handler.board.make_move(mv);
    handler.move_history.push((mv, info));

    handler.handle_command(&XBoardCommand::Edit);

    assert!(handler.move_history.is_empty());
}

#[test]
fn test_edit_mode_defaults_to_white_to_move_after_black_position() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::SetBoard(
        "8/8/8/8/8/8/8/K1k5 b - - 0 1".to_string(),
    ));
    assert!(!handler.board.white_to_move());

    handler.handle_command(&XBoardCommand::Edit);
    handler.handle_command(&XBoardCommand::EditDone);

    assert!(handler.board.white_to_move());
}

#[test]
fn test_analyze_mode() {
    let mut handler = XBoardHandler::new();
    assert!(!handler.analyze_mode);
    handler.handle_command(&XBoardCommand::Analyze);
    assert!(handler.analyze_mode);
    assert!(handler.force_mode); // Analyze mode sets force mode
    handler.handle_command(&XBoardCommand::ExitAnalyze);
    assert!(!handler.analyze_mode);
}

#[test]
fn test_random_noop() {
    let mut handler = XBoardHandler::new();
    let result = handler.handle_command(&XBoardCommand::Random);
    assert!(result.is_none()); // Should be silent no-op
}

#[test]
fn test_result() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::New);
    assert!(!handler.force_mode);
    handler.handle_command(&XBoardCommand::Result("1-0 {White wins}".to_string()));
    assert!(handler.force_mode); // Result sets force mode
}

#[test]
fn test_play_engine_turn_skips_when_not_engine_turn() {
    let mut handler = XBoardHandler::new();
    assert_eq!(handler.play_engine_turn(), None);
    assert!(handler.move_history.is_empty());
}

#[test]
fn test_play_engine_turn_makes_engine_move() {
    let mut handler = XBoardHandler::new();
    handler.max_depth = 1;
    handler.engine_color = Some(Color::White);

    let output = handler.play_engine_turn().expect("engine should move");

    assert!(output.starts_with("move "));
    assert_eq!(handler.move_history.len(), 1);
    assert_eq!(handler.board.side_to_move(), Color::Black);
}
