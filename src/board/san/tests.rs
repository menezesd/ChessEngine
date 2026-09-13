use crate::board::{Board, Piece, SanError, Square};

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
fn test_capture_marker_may_be_omitted_but_cannot_be_spurious() {
    let mut capture_position =
        Board::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2");
    let expected = capture_position.parse_san("exd5").unwrap();
    for san in ["ed5", "xd5", "e4d5", "e4xd5"] {
        assert_eq!(capture_position.parse_san(san), Ok(expected), "{san}");
    }

    let mut quiet_position = Board::new();
    assert!(matches!(
        quiet_position.parse_san("Nxf3"),
        Err(SanError::NoMatchingMove { .. })
    ));
}

#[test]
fn test_crafty_style_extended_algebraic() {
    let mut board = Board::new();
    let expected = board.parse_move("e2e4").unwrap();
    for san in ["Pe4", "e2e4", "e2-e4"] {
        assert_eq!(board.parse_san(san), Ok(expected), "{san}");
    }

    let expected = board.parse_move("g1f3").unwrap();
    for san in ["Ng1f3", "Ng1-f3"] {
        assert_eq!(board.parse_san(san), Ok(expected), "{san}");
    }
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
fn full_origin_disambiguation_round_trips_for_quiet_moves_and_captures() {
    for (fen, coordinate, expected) in [
        ("7k/8/8/8/8/1Q6/8/1Q1Q3K w - - 0 1", "b1c2", "Qb1c2"),
        ("7k/8/8/8/8/1Q6/2p5/1Q1Q3K w - - 0 1", "b1c2", "Qb1xc2"),
        ("7k/8/8/8/8/1N6/8/1N2KN2 w - - 0 1", "b1d2", "Nb1d2"),
        ("7k/8/8/8/8/1N6/3p4/1N2KN2 w - - 0 1", "b1d2", "Nb1xd2"),
    ] {
        let mut board = Board::from_fen(fen);
        let mv = board.parse_move(coordinate).unwrap();
        let san = board.move_to_san(&mv);
        assert_eq!(san, expected);
        assert_eq!(board.parse_san(&san), Ok(mv), "{fen}: {san}");
    }
}

#[test]
fn malformed_san_does_not_silently_become_a_legal_move() {
    let mut board = Board::new();
    for san in ["e4=", "Nf3=", "NNf3", "N!f3", "Nλf3"] {
        assert!(board.parse_san(san).is_err(), "accepted {san}");
    }
    assert!(board.parse_san("Nf3!?").is_ok());
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

#[test]
fn test_malformed_disambiguation_is_rejected_without_panicking() {
    let mut board = Board::new();

    for san in ["N0a3", "N9a3", "Nixd4"] {
        assert!(matches!(
            board.parse_san(san),
            Err(SanError::InvalidSquare { .. })
        ));
    }
}
