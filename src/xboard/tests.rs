use super::handler::{normalized_memory_mb, normalized_search_depth, seconds_to_centiseconds};
use super::*;
use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::board::{Board, Color, Piece, Square, DEFAULT_TT_MB};
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
fn test_zero_remaining_time_is_a_clock_limit() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Time(0));

    assert_eq!(
        handler.time_control(),
        TimeControl::from_xboard_time(0, 0, Some(40))
    );
}

#[test]
fn test_session_clock_uses_moves_remaining_in_the_current_session() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Level {
        moves_per_session: 40,
        base_seconds: 300,
        increment_seconds: 2,
    });

    for (move_number, remaining) in [(1, 40), (39, 2), (40, 1), (41, 40)] {
        handler.handle_command(&XBoardCommand::SetBoard(format!(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 {move_number}"
        )));
        assert_eq!(
            handler.time_control(),
            TimeControl::from_xboard_time(30_000, 2, Some(remaining))
        );
    }
}

#[test]
fn test_midgame_level_starts_a_new_session_count_from_current_position() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::SetBoard(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 41".to_string(),
    ));
    handler.handle_command(&XBoardCommand::Level {
        moves_per_session: 15,
        base_seconds: 300,
        increment_seconds: 0,
    });

    assert_eq!(
        handler.time_control(),
        TimeControl::from_xboard_time(30_000, 0, Some(15))
    );

    for (side, move_number, remaining) in [('b', 41, 15), ('w', 42, 14), ('w', 56, 15)] {
        handler.handle_command(&XBoardCommand::SetBoard(format!(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR {side} KQkq - 0 {move_number}"
        )));
        assert_eq!(
            handler.time_control(),
            TimeControl::from_xboard_time(30_000, 0, Some(remaining))
        );
    }
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
fn test_setboard_rejects_invalid_king_counts() {
    let mut handler = XBoardHandler::new();
    let original_hash = handler.board.hash();
    let result = handler.handle_command(&XBoardCommand::SetBoard(
        "8/8/8/8/8/8/8/K7 w - - 0 1".to_string(),
    ));

    assert!(result.is_some());
    assert_eq!(handler.board.hash(), original_hash);
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
    assert_eq!(handler.handle_command(&place), None);
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
fn test_edit_mode_preserves_black_to_move_and_placement_color_is_independent() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::SetBoard(
        "8/8/8/8/8/8/8/K1k5 b - - 0 1".to_string(),
    ));
    assert!(!handler.board.white_to_move());

    handler.handle_command(&XBoardCommand::Edit);
    handler.handle_command(&XBoardCommand::ClearBoard);
    handler.handle_command(&XBoardCommand::UserMove("Ka1".to_string()));
    handler.handle_command(&XBoardCommand::EditColor);
    handler.handle_command(&XBoardCommand::UserMove("Kc1".to_string()));
    handler.handle_command(&XBoardCommand::EditColor);
    handler.handle_command(&XBoardCommand::EditDone);

    assert!(!handler.board.white_to_move());
    assert_eq!(
        handler.board.piece_at(Square::new(0, 0)),
        Some((Color::White, Piece::King))
    );
    assert_eq!(
        handler.board.piece_at(Square::new(0, 2)),
        Some((Color::Black, Piece::King))
    );
}

#[test]
fn test_edit_keeps_existing_pieces_until_clear_command() {
    let mut handler = XBoardHandler::new();
    let fen = handler.board.to_fen();

    handler.handle_command(&XBoardCommand::Edit);

    assert_eq!(handler.board.to_fen(), fen);
}

#[test]
fn test_color_commands_set_turn_and_play_the_opposite_side() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Black);
    assert_eq!(handler.board.side_to_move(), Color::Black);
    assert_eq!(handler.engine_color, Some(Color::White));

    handler.handle_command(&XBoardCommand::White);
    assert_eq!(handler.board.side_to_move(), Color::White);
    assert_eq!(handler.engine_color, Some(Color::Black));
    assert_eq!(
        handler.board.hash(),
        Board::from_fen(&handler.board.to_fen()).hash()
    );
}

#[test]
fn test_playother_leaves_force_mode_and_replies_after_opponent_move() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Force);
    handler.handle_command(&XBoardCommand::PlayOther);
    assert!(!handler.force_mode);
    assert!(!handler.should_think());
    handler.handle_command(&XBoardCommand::UserMove("e4".to_string()));
    assert!(handler.should_think());
}

#[test]
fn test_new_resets_depth_and_game_clocks() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Level {
        moves_per_session: 40,
        base_seconds: 300,
        increment_seconds: 2,
    });
    handler.handle_command(&XBoardCommand::Time(500));
    handler.handle_command(&XBoardCommand::OTime(700));
    handler.handle_command(&XBoardCommand::Sd(1));

    handler.handle_command(&XBoardCommand::New);

    assert_eq!(handler.max_depth, DEFAULT_MAX_DEPTH);
    assert_eq!(handler.engine_time_cs, Some(30_000));
    assert_eq!(handler.opponent_time_cs, 30_000);
}

#[test]
fn test_analysis_restarts_after_position_changes_and_invalid_moves() {
    let mut handler = XBoardHandler::new();
    handler.handle_command(&XBoardCommand::Memory(1));
    handler.handle_command(&XBoardCommand::Sd(1));
    handler.handle_command(&XBoardCommand::Analyze);

    for command in [
        XBoardCommand::SetBoard(Board::new().to_fen()),
        XBoardCommand::UserMove("e4".to_string()),
        XBoardCommand::Undo,
        XBoardCommand::UserMove("illegal".to_string()),
        XBoardCommand::Memory(1),
        XBoardCommand::New,
    ] {
        handler.handle_command(&command);
        assert!(handler.analyze_mode, "lost analysis mode after {command:?}");
        assert!(
            handler.analyze_handle.is_some(),
            "analysis stopped after {command:?}"
        );
        assert!(!handler.should_think());
    }
    handler.handle_command(&XBoardCommand::ExitAnalyze);
    assert!(handler.analyze_handle.is_none());
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
fn test_start_thinking_skips_when_not_engine_turn() {
    let mut handler = XBoardHandler::new();
    handler.start_thinking();
    assert!(handler.thinking.is_none());
    assert!(handler.move_history.is_empty());
}

#[test]
fn test_finished_search_makes_engine_move() {
    let mut handler = XBoardHandler::new();
    handler.max_depth = 1;
    handler.engine_color = Some(Color::White);

    handler.start_thinking();
    let output = handler.finish_thinking().expect("engine should move");

    assert!(output.starts_with("move "));
    assert_eq!(handler.move_history.len(), 1);
    assert_eq!(handler.board.side_to_move(), Color::Black);
}
