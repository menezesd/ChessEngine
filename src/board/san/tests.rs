use crate::board::{Board, Piece, Square};

#[test]
fn test_pawn_moves() {
    let mut board = Board::new();

    // e4
    let mv = board.parse_san("e4").unwrap();
    assert_eq!(mv.from(), Square::new(1, 4));
    assert_eq!(mv.to(), Square::new(3, 4));
    assert_eq!(board.move_to_san(&mv), "e4");
}

#[test]
fn test_knight_moves() {
    let mut board = Board::new();

    // Nf3
    let mv = board.parse_san("Nf3").unwrap();
    assert_eq!(mv.from(), Square::new(0, 6));
    assert_eq!(mv.to(), Square::new(2, 5));
    assert_eq!(board.move_to_san(&mv), "Nf3");
}

#[test]
fn test_castling() {
    let mut board = Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1");

    // O-O
    let mv = board.parse_san("O-O").unwrap();
    assert!(mv.is_castle_kingside());
    assert_eq!(board.move_to_san(&mv), "O-O");

    // O-O-O
    let mv = board.parse_san("O-O-O").unwrap();
    assert!(mv.is_castle_queenside());
    assert_eq!(board.move_to_san(&mv), "O-O-O");
}

#[test]
fn test_captures() {
    let mut board =
        Board::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2");

    // exd5
    let mv = board.parse_san("exd5").unwrap();
    assert!(mv.is_capture());
    assert_eq!(board.move_to_san(&mv), "exd5");
}

#[test]
fn test_promotion() {
    let mut board = Board::from_fen("8/P7/8/8/8/8/8/K1k5 w - - 0 1");

    // a8=Q
    let mv = board.parse_san("a8=Q").unwrap();
    assert_eq!(mv.promotion(), Some(Piece::Queen));
    assert_eq!(board.move_to_san(&mv), "a8=Q");
}

#[test]
fn test_disambiguation() {
    // Position with two rooks that can go to d4
    let mut board = Board::from_fen("3k4/8/8/8/R6R/8/8/4K3 w - - 0 1");

    // Rad4 vs Rhd4
    let mv = board.parse_san("Rad4").unwrap();
    assert_eq!(mv.from().file(), 0); // a-file

    let mv = board.parse_san("Rhd4").unwrap();
    assert_eq!(mv.from().file(), 7); // h-file
}

#[test]
fn test_check() {
    let mut board = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1");

    // Rh8+ gives check
    let mv = board.parse_san("Rh8").unwrap();
    let san = board.move_to_san(&mv);
    assert_eq!(san, "Rh8+");
}

#[test]
fn test_checkmate() {
    // Fool's mate position
    let mut board =
        Board::from_fen("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2");

    // Qh4#
    let mv = board.parse_san("Qh4").unwrap();
    let san = board.move_to_san(&mv);
    assert_eq!(san, "Qh4#");
}

#[test]
fn test_round_trip() {
    let mut board = Board::new();
    let moves = board.generate_moves();

    for mv in &moves {
        let san = board.move_to_san(mv);
        let parsed = board.parse_san(&san).unwrap();
        assert_eq!(mv.from(), parsed.from());
        assert_eq!(mv.to(), parsed.to());
    }
}
