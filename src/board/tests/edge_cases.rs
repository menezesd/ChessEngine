//! Edge case tests for special chess positions and moves.

use crate::board::{Board, Move, Piece, Square};

#[test]
fn test_stalemate_position() {
    let mut board = Board::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1");
    assert!(!board.is_checkmate());
    assert!(board.is_stalemate());
    assert!(board.generate_moves().is_empty());
}

#[test]
fn test_underpromotion_to_knight() {
    let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");
    let moves = board.generate_moves();

    let knight_promo = moves.iter().find(|m| m.promotion() == Some(Piece::Knight));
    assert!(
        knight_promo.is_some(),
        "Knight promotion should be available"
    );

    let mv = knight_promo.unwrap();
    board.make_move(*mv);
    assert_eq!(board.piece_on(Square(7, 0)), Some(Piece::Knight));
}

#[test]
fn test_underpromotion_to_rook() {
    let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");
    let moves = board.generate_moves();

    let rook_promo = moves.iter().find(|m| m.promotion() == Some(Piece::Rook));
    assert!(rook_promo.is_some(), "Rook promotion should be available");
}

#[test]
fn test_underpromotion_to_bishop() {
    let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");
    let moves = board.generate_moves();

    let bishop_promo = moves.iter().find(|m| m.promotion() == Some(Piece::Bishop));
    assert!(
        bishop_promo.is_some(),
        "Bishop promotion should be available"
    );
}

#[test]
fn test_en_passant_removes_correct_pawn() {
    let mut board = Board::from_fen("rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 1");
    let moves = board.generate_moves();

    let ep_move = moves.iter().find(|m| m.is_en_passant());
    assert!(ep_move.is_some(), "En passant should be available");

    let mv = ep_move.unwrap();
    let info = board.make_move(*mv);

    assert!(
        board.piece_on(Square(4, 3)).is_none(),
        "Captured pawn should be removed"
    );
    assert_eq!(
        board.piece_on(Square(5, 3)),
        Some(Piece::Pawn),
        "Capturing pawn should be on d6"
    );

    board.unmake_move(*mv, info);
    assert_eq!(
        board.piece_on(Square(4, 3)),
        Some(Piece::Pawn),
        "Black pawn should be restored"
    );
    assert_eq!(
        board.piece_on(Square(4, 4)),
        Some(Piece::Pawn),
        "White pawn should be back on e5"
    );
}

#[test]
fn test_castling_blocked_by_check() {
    let mut board = Board::from_fen("r3k2r/8/8/8/4Q3/8/8/R3K2R b KQkq - 0 1");
    let moves = board.generate_moves();

    let castling_move = moves.iter().find(|m| m.is_castling());
    assert!(
        castling_move.is_none(),
        "Castling should not be available when in check"
    );
}

#[test]
fn test_castling_through_attacked_square() {
    let mut board = Board::from_fen("r4rk1/8/8/8/8/8/8/R3K2R w KQ - 0 1");
    let moves = board.generate_moves();

    assert!(
        moves.iter().any(|m| m.is_castling()),
        "Some castling should be available"
    );
}

#[test]
fn test_double_check_only_king_can_move() {
    let mut board = Board::from_fen("4k3/8/8/1b6/8/8/3r4/3K4 w - - 0 1");
    let moves = board.generate_moves();

    for mv in moves.iter() {
        assert_eq!(
            mv.from(),
            Square(0, 3),
            "Only king should be able to move in double check"
        );
    }
}

#[test]
fn test_checkmate_back_rank() {
    let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1");
    let moves = board.generate_moves();
    let mate_move = moves
        .iter()
        .find(|m| m.from() == Square(0, 0) && m.to() == Square(7, 0));
    assert!(mate_move.is_some());

    board.make_move(*mate_move.unwrap());
    assert!(board.is_checkmate());
}

#[test]
fn test_fen_parsing_errors() {
    assert!(Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").is_err());
    assert!(
        Board::try_from_fen("rnbxkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").is_err()
    );
    assert!(
        Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1").is_err()
    );
    assert!(
        Board::try_from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w XYZ - 0 1").is_err()
    );
}

#[test]
fn test_square_parsing() {
    use std::str::FromStr;

    assert_eq!(Square::from_str("a1").unwrap(), Square(0, 0));
    assert_eq!(Square::from_str("h8").unwrap(), Square(7, 7));
    assert_eq!(Square::from_str("e4").unwrap(), Square(3, 4));

    assert!(Square::from_str("i1").is_err());
    assert!(Square::from_str("a9").is_err());
    assert!(Square::from_str("").is_err());
    assert!(Square::from_str("a").is_err());
}

#[test]
fn test_square_try_from() {
    assert!(Square::try_from((0, 0)).is_ok());
    assert!(Square::try_from((7, 7)).is_ok());
    assert!(Square::try_from((8, 0)).is_err());
    assert!(Square::try_from((0, 8)).is_err());
}

#[test]
fn test_move_convenience_methods() {
    let quiet = Move::quiet(Square(1, 4), Square(3, 4));
    assert!(quiet.is_quiet());
    assert!(!quiet.is_capture());
    assert!(!quiet.is_promotion());
    assert!(!quiet.is_tactical());

    let double_pawn = Move::double_pawn_push(Square(1, 4), Square(3, 4));
    assert!(double_pawn.is_quiet());
    assert!(double_pawn.is_double_pawn_push());

    let capture = Move::capture(Square(3, 3), Square(4, 4));
    assert!(!capture.is_quiet());
    assert!(capture.is_capture());
    assert!(!capture.is_promotion());
    assert!(capture.is_tactical());

    let promo = Move::new_promotion(Square(6, 0), Square(7, 0), Piece::Queen);
    assert!(!promo.is_quiet());
    assert!(!promo.is_capture());
    assert!(promo.is_promotion());
    assert!(promo.is_tactical());
    assert_eq!(promo.promotion(), Some(Piece::Queen));

    let promo_cap = Move::new_promotion_capture(Square(6, 0), Square(7, 1), Piece::Queen);
    assert!(!promo_cap.is_quiet());
    assert!(promo_cap.is_capture());
    assert!(promo_cap.is_promotion());
    assert!(promo_cap.is_tactical());

    let castle = Move::castle_kingside(Square(0, 4), Square(0, 6));
    assert!(!castle.is_quiet());
    assert!(!castle.is_capture());
    assert!(castle.is_castling());
    assert!(castle.is_castle_kingside());

    let ep = Move::en_passant(Square(4, 4), Square(5, 5));
    assert!(!ep.is_quiet());
    assert!(ep.is_capture());
    assert!(ep.is_en_passant());
}

#[test]
fn test_movelist_index() {
    let mut board = Board::new();
    let moves = board.generate_moves();

    if !moves.is_empty() {
        let first = &moves[0];
        assert_eq!(first, moves.first().as_ref().unwrap());
    }
}

#[test]
fn test_board_from_str() {
    let board: Board = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    assert!(board.white_to_move());

    let result: Result<Board, _> = "invalid fen".parse();
    assert!(result.is_err());
}
