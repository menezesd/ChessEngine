use super::*;

#[test]
fn test_piece_values_ordering() {
    // Pieces should be ordered by value
    assert!(piece_value(Piece::Pawn) < piece_value(Piece::Knight));
    assert!(piece_value(Piece::Knight) < piece_value(Piece::Bishop));
    assert!(piece_value(Piece::Bishop) < piece_value(Piece::Rook));
    assert!(piece_value(Piece::Rook) < piece_value(Piece::Queen));
    assert!(piece_value(Piece::Queen) < piece_value(Piece::King));
}

#[test]
fn test_pawn_value() {
    assert_eq!(piece_value(Piece::Pawn), 100);
}

#[test]
fn test_minor_pieces() {
    // Knight and bishop should be close in value
    let knight = piece_value(Piece::Knight);
    let bishop = piece_value(Piece::Bishop);
    assert!((knight - bishop).abs() < 50);
}

#[test]
fn test_rook_vs_minor() {
    // Rook should be worth roughly a minor + 1.5-2 pawns
    let rook = piece_value(Piece::Rook);
    let bishop = piece_value(Piece::Bishop);
    let exchange = rook - bishop;
    assert!(exchange > 100 && exchange < 250);
}

#[test]
fn test_queen_vs_rooks() {
    // Queen should be worth less than two rooks
    let queen = piece_value(Piece::Queen);
    let two_rooks = 2 * piece_value(Piece::Rook);
    assert!(queen < two_rooks);
}

#[test]
fn test_king_highest_value() {
    // King should have highest value (for move ordering purposes)
    assert!(piece_value(Piece::King) > piece_value(Piece::Queen));
    assert!(piece_value(Piece::King) >= 20000);
}
