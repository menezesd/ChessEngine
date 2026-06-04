use super::*;

#[test]
fn test_starting_position() {
    let built = BoardBuilder::starting_position().build();
    let standard = Board::new();

    assert_eq!(built.to_fen(), standard.to_fen());
}

#[test]
fn test_empty_board() {
    let board = BoardBuilder::new()
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .build();

    assert!(board.piece_at(Square::new(0, 4)).is_some());
    assert!(board.piece_at(Square::new(7, 4)).is_some());
    assert!(board.piece_at(Square::new(0, 0)).is_none());
}

#[test]
fn test_castling_rights() {
    let board = BoardBuilder::starting_position()
        .no_castling_rights()
        .castle_kingside(Color::White)
        .build();

    let rights = CastlingRights::from_u8(board.castling_rights);
    assert!(rights.has(Color::White, true));
    assert!(!rights.has(Color::White, false));
    assert!(!rights.has(Color::Black, true));
    assert!(!rights.has(Color::Black, false));
}

#[test]
fn test_side_to_move() {
    let board = BoardBuilder::new()
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .side_to_move(Color::Black)
        .build();

    assert!(!board.white_to_move());
}

#[test]
fn test_clear_square() {
    let board = BoardBuilder::starting_position()
        .clear(Square::new(0, 0))
        .build();

    assert!(board.piece_at(Square::new(0, 0)).is_none());
    assert!(board.piece_at(Square::new(0, 1)).is_some());
}

#[test]
fn test_en_passant() {
    let board = BoardBuilder::new()
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .piece(Square::new(4, 3), Color::White, Piece::Pawn)
        .piece(Square::new(4, 4), Color::Black, Piece::Pawn)
        .en_passant(Square::new(5, 4))
        .build();

    assert_eq!(board.en_passant_target, Some(Square::new(5, 4)));
}

#[test]
fn test_clear_en_passant() {
    let board = BoardBuilder::new()
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .en_passant(Square::new(5, 4))
        .clear_en_passant()
        .build();

    assert!(board.en_passant_target.is_none());
}

#[test]
fn test_halfmove_clock() {
    let board = BoardBuilder::new()
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .halfmove_clock(50)
        .build();

    assert_eq!(board.halfmove_clock, 50);
}

#[test]
fn test_castle_queenside() {
    let board = BoardBuilder::starting_position()
        .no_castling_rights()
        .castle_queenside(Color::Black)
        .build();

    let rights = CastlingRights::from_u8(board.castling_rights);
    assert!(!rights.has(Color::White, true));
    assert!(!rights.has(Color::White, false));
    assert!(!rights.has(Color::Black, true));
    assert!(rights.has(Color::Black, false));
}

#[test]
fn test_all_castling_rights() {
    let board = BoardBuilder::starting_position()
        .no_castling_rights()
        .all_castling_rights()
        .build();

    let rights = CastlingRights::from_u8(board.castling_rights);
    assert!(rights.has(Color::White, true));
    assert!(rights.has(Color::White, false));
    assert!(rights.has(Color::Black, true));
    assert!(rights.has(Color::Black, false));
}

#[test]
fn test_piece_replacement() {
    let board = BoardBuilder::new()
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .piece(Square::new(3, 3), Color::White, Piece::Pawn)
        .piece(Square::new(3, 3), Color::White, Piece::Queen)
        .build();

    let (color, piece) = board.piece_at(Square::new(3, 3)).unwrap();
    assert_eq!(color, Color::White);
    assert_eq!(piece, Piece::Queen);
}

#[test]
fn test_default_builder() {
    let builder = BoardBuilder::default();
    let board = builder
        .piece(Square::new(0, 4), Color::White, Piece::King)
        .piece(Square::new(7, 4), Color::Black, Piece::King)
        .build();

    assert!(board.white_to_move());
}
