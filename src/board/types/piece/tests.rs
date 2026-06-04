use super::*;

#[test]
fn test_piece_index() {
    assert_eq!(Piece::Pawn.index(), 0);
    assert_eq!(Piece::Knight.index(), 1);
    assert_eq!(Piece::Bishop.index(), 2);
    assert_eq!(Piece::Rook.index(), 3);
    assert_eq!(Piece::Queen.index(), 4);
    assert_eq!(Piece::King.index(), 5);
}

#[test]
fn test_piece_from_char() {
    assert_eq!(Piece::from_char('p'), Some(Piece::Pawn));
    assert_eq!(Piece::from_char('N'), Some(Piece::Knight));
    assert_eq!(Piece::from_char('b'), Some(Piece::Bishop));
    assert_eq!(Piece::from_char('R'), Some(Piece::Rook));
    assert_eq!(Piece::from_char('q'), Some(Piece::Queen));
    assert_eq!(Piece::from_char('K'), Some(Piece::King));
    assert_eq!(Piece::from_char('x'), None);
}

#[test]
fn test_piece_to_char() {
    assert_eq!(Piece::Pawn.to_char(), 'p');
    assert_eq!(Piece::Knight.to_char(), 'n');
    assert_eq!(Piece::Bishop.to_char(), 'b');
    assert_eq!(Piece::Rook.to_char(), 'r');
    assert_eq!(Piece::Queen.to_char(), 'q');
    assert_eq!(Piece::King.to_char(), 'k');
}

#[test]
fn test_piece_to_fen_char() {
    assert_eq!(Piece::Pawn.to_fen_char(Color::White), 'P');
    assert_eq!(Piece::Pawn.to_fen_char(Color::Black), 'p');
    assert_eq!(Piece::Knight.to_fen_char(Color::White), 'N');
    assert_eq!(Piece::Queen.to_fen_char(Color::Black), 'q');
}

#[test]
fn test_piece_value_ordering() {
    assert!(Piece::Pawn.value() < Piece::Knight.value());
    assert!(Piece::Knight.value() < Piece::Bishop.value());
    assert!(Piece::Bishop.value() < Piece::Rook.value());
    assert!(Piece::Rook.value() < Piece::Queen.value());
    assert!(Piece::Queen.value() < Piece::King.value());
}

#[test]
fn test_piece_attacks_diagonally() {
    assert!(!Piece::Pawn.attacks_diagonally());
    assert!(!Piece::Knight.attacks_diagonally());
    assert!(Piece::Bishop.attacks_diagonally());
    assert!(!Piece::Rook.attacks_diagonally());
    assert!(Piece::Queen.attacks_diagonally());
    assert!(!Piece::King.attacks_diagonally());
}

#[test]
fn test_piece_attacks_straight() {
    assert!(!Piece::Pawn.attacks_straight());
    assert!(!Piece::Knight.attacks_straight());
    assert!(!Piece::Bishop.attacks_straight());
    assert!(Piece::Rook.attacks_straight());
    assert!(Piece::Queen.attacks_straight());
    assert!(!Piece::King.attacks_straight());
}

#[test]
fn test_piece_is_slider() {
    assert!(!Piece::Pawn.is_slider());
    assert!(!Piece::Knight.is_slider());
    assert!(Piece::Bishop.is_slider());
    assert!(Piece::Rook.is_slider());
    assert!(Piece::Queen.is_slider());
    assert!(!Piece::King.is_slider());
}

#[test]
fn test_piece_all_array() {
    assert_eq!(Piece::ALL.len(), 6);
    for (i, piece) in Piece::ALL.iter().enumerate() {
        assert_eq!(piece.index(), i);
    }
}

#[test]
fn test_piece_minor_and_major() {
    assert_eq!(Piece::MINOR_AND_MAJOR.len(), 4);
    assert!(!Piece::MINOR_AND_MAJOR.contains(&Piece::Pawn));
    assert!(!Piece::MINOR_AND_MAJOR.contains(&Piece::King));
}

#[test]
fn test_color_index() {
    assert_eq!(Color::White.index(), 0);
    assert_eq!(Color::Black.index(), 1);
}

#[test]
fn test_color_opponent() {
    assert_eq!(Color::White.opponent(), Color::Black);
    assert_eq!(Color::Black.opponent(), Color::White);
}

#[test]
fn test_color_sign() {
    assert_eq!(Color::White.sign(), 1);
    assert_eq!(Color::Black.sign(), -1);
}

#[test]
fn test_color_back_rank() {
    assert_eq!(Color::White.back_rank(), 0);
    assert_eq!(Color::Black.back_rank(), 7);
}

#[test]
fn test_color_pawn_direction() {
    assert_eq!(Color::White.pawn_direction(), 1);
    assert_eq!(Color::Black.pawn_direction(), -1);
}

#[test]
fn test_color_pawn_start_rank() {
    assert_eq!(Color::White.pawn_start_rank(), 1);
    assert_eq!(Color::Black.pawn_start_rank(), 6);
}

#[test]
fn test_color_pawn_promotion_rank() {
    assert_eq!(Color::White.pawn_promotion_rank(), 7);
    assert_eq!(Color::Black.pawn_promotion_rank(), 0);
}

#[test]
fn test_color_display() {
    assert_eq!(format!("{}", Color::White), "White");
    assert_eq!(format!("{}", Color::Black), "Black");
}

#[test]
fn test_color_both_array() {
    assert_eq!(Color::BOTH.len(), 2);
    assert_eq!(Color::BOTH[0], Color::White);
    assert_eq!(Color::BOTH[1], Color::Black);
}
