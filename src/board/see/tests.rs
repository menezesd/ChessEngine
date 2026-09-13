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
    // Rxe4 wins a pawn. The adjacent king cannot recapture because Bb1
    // protects e4; the rook capture itself gives no check.
    let board = make_board("8/8/8/3k4/4p3/8/8/1B2R2K w - - 0 1");
    let from = Square::new(0, 4);
    let to = Square::new(3, 4);
    let see = board.see(from, to);
    assert_eq!(see, 100);
}

#[test]
fn see_ignores_recapture_that_exposes_own_king() {
    // f5xe6 opens the f-file, so Black is in check from Rf1. Although the
    // queen on e8 attacks e6 geometrically, ...Qxe6 is not a legal reply
    // because it does not answer that check.
    let board = make_board("4qk2/N7/p3p3/5Pn1/1r6/8/3Kb3/5R2 w - - 8 64");
    assert_eq!(board.see(Square::new(4, 5), Square::new(5, 4)), 100);
}

#[test]
fn see_includes_later_recaptures_when_an_exchange_is_already_losing() {
    // ...Bxc3 bxc3 bxc3 Nxc3 loses bishop + pawn for two pawns.
    let board =
        make_board("rnbqk2r/p2pp1b1/7p/2p2p1P/1p3Bn1/P1PP1P2/QP2P1P1/RN2KBNR b KQkq - 3 12");
    assert_eq!(board.see(Square::new(6, 6), Square::new(2, 2)), -230);
}

#[test]
fn see_accounts_for_a_pawn_promoting_during_a_recapture() {
    // ...Rxc8 bxc8=Q loses a rook and gives White a promotion, gaining only a bishop.
    let board = make_board("r1B5/1P6/8/8/8/8/3R2pk/3K4 b - - 3 107");
    assert_eq!(board.see(Square::new(7, 0), Square::new(7, 2)), -970);
}

#[test]
fn see_counts_the_initial_queen_promotion_gain() {
    let board = make_board("r1B5/1P6/8/8/8/8/3R2pk/3K4 w - - 3 107");
    assert_eq!(board.see(Square::new(6, 1), Square::new(7, 0)), 1300);
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
fn test_see_en_passant_opens_rook_recapture() {
    // The pawn on e5 blocks the rook before d5xe6 e.p.; removing it opens
    // the e-file, so the rook can immediately recapture on e6.
    let board = make_board("k7/8/8/3Pp3/8/8/8/K3r3 w - e6 0 1");
    let from = Square::new(4, 3);
    let to = Square::new(5, 4);
    assert_eq!(board.see(from, to), 0);
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
fn test_see_rejects_friendly_capture_and_wrong_side_attacker() {
    let friendly_target = make_board("8/8/8/3P4/4P3/8/8/8 w - - 0 1");
    assert_eq!(friendly_target.see(Square::new(3, 4), Square::new(4, 3)), 0);

    let wrong_side = make_board("8/8/8/3p4/4PP2/8/8/8 w - - 0 1");
    assert_eq!(wrong_side.see(Square::new(4, 3), Square::new(3, 5)), 0);
}

#[test]
fn test_see_rejects_capturing_the_king() {
    let board = make_board("4k3/8/8/8/8/8/8/4Q2K w - - 0 1");

    assert_eq!(board.see(Square::new(0, 4), Square::new(7, 4)), 0);
}

#[test]
fn test_see_rejects_rank_edge_en_passant_target_without_panicking() {
    let board = crate::board::BoardBuilder::new()
        .piece(Square::new(0, 1), Color::White, Piece::Pawn)
        .en_passant(Square::new(0, 0))
        .build();

    assert_eq!(board.see(Square::new(0, 1), Square::new(0, 0)), 0);
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
