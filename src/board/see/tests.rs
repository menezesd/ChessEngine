use super::*;

fn make_board(fen: &str) -> Board {
    fen.parse().expect("valid fen")
}

#[test]
fn test_see_simple_capture() {
    let board = make_board("8/8/8/3p4/4P3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 100);
}

#[test]
fn test_see_bad_capture() {
    let board = make_board("8/8/2p5/3p4/4P3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 0);
}

#[test]
fn test_see_winning_exchange() {
    let board = make_board("8/8/2p5/3p4/4N3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert!(see < 0);
}

#[test]
fn test_see_queen_takes_defended_pawn() {
    let board = make_board("8/8/2p5/3p4/4Q3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert!(see < 0);
}

#[test]
fn test_see_with_xray() {
    let board = make_board("3r4/8/8/8/8/8/8/R2R4 w - - 0 1");
    let from = Square::new(0, 0);
    let to = Square::new(7, 3);
    let see = board.see(from, to);
    assert_eq!(see, 500);
}

#[test]
fn test_see_bishop_xray() {
    let board = make_board("8/8/5b2/4b3/3B4/2B5/8/8 w - - 0 1");
    let from = Square::new(2, 2);
    let to = Square::new(4, 4);
    let see = board.see(from, to);
    assert!(see > 0);
}

#[test]
fn test_see_rook_xray() {
    let board = make_board("8/8/8/3r4/8/8/8/R2R4 w - - 0 1");
    let from = Square::new(0, 3);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 500);
}

#[test]
fn test_see_queen_xray_diagonal() {
    let board = make_board("8/8/5b2/8/3B4/8/1Q6/8 w - - 0 1");
    let from = Square::new(3, 3);
    let to = Square::new(5, 5);
    let see = board.see(from, to);
    assert_eq!(see, 330);
}

#[test]
fn test_see_multiple_attackers() {
    let board = make_board("8/8/8/3p4/2N1N3/8/8/8 w - - 0 1");
    let from = Square::new(3, 2);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 100);
}

#[test]
fn test_see_king_cannot_recapture_defended() {
    let board = make_board("4k3/8/4p3/3P4/8/8/8/4R3 w - - 0 1");
    let from = Square::new(4, 3);
    let to = Square::new(5, 4);
    let see = board.see(from, to);
    assert_eq!(see, 100);
}

#[test]
fn test_see_winning_queen_takes_rook_defended_by_pawn() {
    let board = make_board("8/8/1p6/2r5/3Q4/8/8/8 w - - 0 1");
    let from = Square::new(3, 3);
    let to = Square::new(4, 2);
    let see = board.see(from, to);
    assert!(see < 0);
}

#[test]
fn test_see_knight_takes_defended_knight() {
    let board = make_board("8/8/8/3n4/2N5/8/8/8 w - - 0 1");
    let from = Square::new(3, 2);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 320);
}

#[test]
fn test_see_bishop_pair_exchange() {
    let board = make_board("8/8/8/3b4/4B3/5B2/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 330);
}

#[test]
fn test_see_en_passant_simple() {
    let board = make_board("8/8/8/3Pp3/8/8/8/8 w - e6 0 1");
    let from = Square::new(4, 3);
    let to = Square::new(5, 4);
    let see = board.see(from, to);
    assert_eq!(see, 100);
}

#[test]
fn test_see_en_passant_defended() {
    let board = make_board("8/5p2/8/3Pp3/8/8/8/8 w - e6 0 1");
    let from = Square::new(4, 3);
    let to = Square::new(5, 4);
    let see = board.see(from, to);
    assert_eq!(see, 0);
}

#[test]
fn test_see_no_capture() {
    let board = make_board("8/8/8/8/4N3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(5, 5);
    let see = board.see(from, to);
    assert_eq!(see, 0);
}

#[test]
fn test_see_undefended_piece() {
    let board = make_board("8/8/8/3r4/8/8/8/3R4 w - - 0 1");
    let from = Square::new(0, 3);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert_eq!(see, 500);
}

#[test]
fn test_see_pawn_takes_queen_defended_by_queen() {
    let board = make_board("8/8/3q4/2q5/3P4/8/8/8 w - - 0 1");
    let from = Square::new(3, 3);
    let to = Square::new(4, 2);
    let see = board.see(from, to);
    assert!(see > 700);
}

#[test]
fn test_see_rook_takes_rook_both_defended() {
    let board = make_board("3r4/8/8/3r4/8/8/8/R2R4 w - - 0 1");
    let from = Square::new(0, 3);
    let to = Square::new(4, 3);
    let see = board.see(from, to);
    assert!(see >= 0, "see={see}");
}

#[test]
fn test_see_ge_winning_capture() {
    let board = make_board("8/8/8/3p4/4N3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    assert!(board.see_ge(from, to, 0));
    assert!(board.see_ge(from, to, 100));
}

#[test]
fn test_see_ge_losing_capture() {
    let board = make_board("8/8/2p5/3p4/4Q3/8/8/8 w - - 0 1");
    let from = Square::new(3, 4);
    let to = Square::new(4, 3);
    assert!(!board.see_ge(from, to, 0));
}
