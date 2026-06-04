use super::*;

#[test]
fn test_color_index_round_trip() {
    assert_eq!(ColorIndex::WHITE.to_color(), Color::White);
    assert_eq!(ColorIndex::BLACK.to_color(), Color::Black);
    assert_eq!(ColorIndex::from_color(Color::White), ColorIndex::WHITE);
    assert_eq!(ColorIndex::from_color(Color::Black), ColorIndex::BLACK);
}

#[test]
fn test_color_index_opponent() {
    assert_eq!(ColorIndex::WHITE.opponent(), ColorIndex::BLACK);
    assert_eq!(ColorIndex::BLACK.opponent(), ColorIndex::WHITE);
}

#[test]
fn test_piece_index_round_trip() {
    for piece in Piece::ALL {
        let idx = PieceIndex::from_piece(piece);
        assert_eq!(idx.to_piece(), piece);
    }
}
