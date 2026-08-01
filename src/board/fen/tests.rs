use super::*;
use crate::board::error::MoveParseError;
use crate::board::FenError;

#[test]
fn test_fen_round_trip() {
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let board = Board::try_from_fen(fen).unwrap();
    let output = board.to_fen();
    // Compare the relevant parts (ignoring move number)
    assert!(output.starts_with("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -"));
}

#[test]
fn test_fen_black_to_move() {
    let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let board = Board::try_from_fen(fen).unwrap();
    assert!(!board.white_to_move());
    assert!(board.en_passant_target.is_some());
}

#[test]
fn test_fen_error_too_few_parts() {
    let result = Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w");
    assert!(matches!(result, Err(FenError::TooFewParts { .. })));
}

#[test]
fn test_fen_error_too_many_parts() {
    let result = Board::try_from_fen("8/8/8/8/8/8/8/K1k5 w - - 0 1 trailing");
    assert!(matches!(result, Err(FenError::TooManyParts { found: 7 })));
}

#[test]
fn test_fen_error_invalid_piece() {
    let result = Board::try_from_fen("rnbqkbnr/ppppxppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    assert!(matches!(result, Err(FenError::InvalidPiece { .. })));
}

#[test]
fn test_fen_error_invalid_empty_square_count() {
    let zero = Board::try_from_fen("08/8/8/8/8/8/8/K1k5 w - - 0 1");
    assert!(matches!(
        zero,
        Err(FenError::InvalidEmptySquareCount { char: '0' })
    ));

    let nine = Board::try_from_fen("9/8/8/8/8/8/8/K1k5 w - - 0 1");
    assert!(matches!(
        nine,
        Err(FenError::InvalidEmptySquareCount { char: '9' })
    ));
}

#[test]
fn test_fen_error_too_few_ranks() {
    let result = Board::try_from_fen("8/8/8/8/8/8/8 w - - 0 1");
    assert!(matches!(result, Err(FenError::TooFewRanks { found: 7 })));
}

#[test]
fn test_fen_error_too_few_files() {
    let result = Board::try_from_fen("8/8/8/8/8/8/8/K1k4 w - - 0 1");
    assert!(matches!(
        result,
        Err(FenError::TooFewFiles { rank: 7, files: 7 })
    ));
}

#[test]
fn test_fen_error_invalid_side_to_move() {
    let result = Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1");
    assert!(matches!(result, Err(FenError::InvalidSideToMove { .. })));
}

#[test]
fn test_fen_error_invalid_castling() {
    let result = Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w XQkq - 0 1");
    assert!(matches!(result, Err(FenError::InvalidCastling { .. })));
}

#[test]
fn test_fen_error_malformed_castling() {
    for castling in ["KK", "K-", "-K"] {
        let fen = format!("8/8/8/8/8/8/8/K1k5 w {castling} - 0 1");
        assert!(matches!(
            Board::try_from_fen(&fen),
            Err(FenError::InvalidCastling { .. })
        ));
    }
}

#[test]
fn test_fen_error_invalid_en_passant() {
    let result = Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq z9 0 1");
    assert!(matches!(result, Err(FenError::InvalidEnPassant { .. })));
}

#[test]
fn test_fen_error_en_passant_wrong_rank_or_side_to_move() {
    for fen in [
        "8/8/8/8/8/8/8/K1k5 w - e3 0 1",
        "8/8/8/8/8/8/8/K1k5 b - e6 0 1",
        "8/8/8/8/8/8/8/K1k5 w - e4 0 1",
    ] {
        assert!(matches!(
            Board::try_from_fen(fen),
            Err(FenError::InvalidEnPassant { .. })
        ));
    }
}

#[test]
fn test_fen_error_invalid_halfmove_clock() {
    let result = Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - nope 1");
    assert!(matches!(result, Err(FenError::InvalidHalfmoveClock { .. })));
}

#[test]
fn test_fen_no_castling() {
    let board =
        Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
    assert_eq!(board.castling_rights, 0);
}

#[test]
fn test_fen_partial_castling() {
    let board =
        Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Kq - 0 1").unwrap();
    assert!((board.castling_rights & CASTLE_WHITE_K) != 0);
    assert!((board.castling_rights & CASTLE_WHITE_Q) == 0);
    assert!((board.castling_rights & CASTLE_BLACK_K) == 0);
    assert!((board.castling_rights & CASTLE_BLACK_Q) != 0);
}

#[test]
fn test_parse_move_e2e4() {
    let mut board = Board::new();
    let mv = board.parse_move("e2e4").unwrap();
    assert_eq!(mv.from(), Square::new(1, 4));
    assert_eq!(mv.to(), Square::new(3, 4));
}

#[test]
fn test_parse_move_promotion() {
    let mut board = Board::try_from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1").unwrap();
    let mv = board.parse_move("a7a8q").unwrap();
    assert_eq!(mv.promotion(), Some(Piece::Queen));
}

#[test]
fn test_parse_move_error_invalid_length() {
    let mut board = Board::new();
    let result = board.parse_move("e2");
    assert!(matches!(result, Err(MoveParseError::InvalidLength { .. })));
}

#[test]
fn test_parse_move_error_invalid_square() {
    let mut board = Board::new();
    let result = board.parse_move("z9z9");
    assert!(matches!(result, Err(MoveParseError::InvalidSquare { .. })));
}

#[test]
fn test_parse_move_error_illegal() {
    let mut board = Board::new();
    let result = board.parse_move("e2e5"); // Pawn can't move 3 squares
    assert!(matches!(result, Err(MoveParseError::IllegalMove { .. })));
}

#[test]
fn test_parse_move_error_invalid_promotion() {
    let mut board = Board::try_from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1").unwrap();
    // Promote to pawn is invalid
    let result = board.parse_move("a7a8p");
    assert!(matches!(
        result,
        Err(MoveParseError::InvalidPromotion { .. })
    ));
}

#[test]
fn test_from_str_trait() {
    let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    assert!(board.white_to_move());
}

#[test]
fn test_make_move_uci() {
    let mut board = Board::new();
    board.make_move_uci("e2e4").unwrap();
    assert!(!board.white_to_move()); // Black to move after e4
}

#[test]
fn test_halfmove_clock_parsing() {
    let board = Board::try_from_fen("8/8/8/8/8/8/8/K1k5 w - - 42 1").unwrap();
    assert_eq!(board.halfmove_clock, 42);
}

#[test]
fn test_fullmove_number_round_trip() {
    let fen = "8/8/8/8/8/8/8/K1k5 b - - 42 57";
    let board = Board::try_from_fen(fen).unwrap();

    assert_eq!(board.fullmove_number(), 57);
    assert_eq!(board.to_fen(), fen);
}

#[test]
fn test_fen_error_invalid_fullmove_number() {
    for fullmove in ["0", "nope"] {
        let fen = format!("8/8/8/8/8/8/8/K1k5 w - - 0 {fullmove}");
        assert!(matches!(
            Board::try_from_fen(&fen),
            Err(FenError::InvalidFullmoveNumber { .. })
        ));
    }
}
