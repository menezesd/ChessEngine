use super::*;

#[test]
fn test_single_pawn_attacks_white_center() {
    // e4 pawn attacks d5 and f5
    let attacks = single_pawn_attacks(28, Color::White).unwrap(); // e4 = index 28
    let d5 = 1u64 << 35; // d5
    let f5 = 1u64 << 37; // f5
    assert_eq!(attacks, d5 | f5);
}

#[test]
fn test_single_pawn_attacks_white_a_file() {
    // a4 pawn only attacks b5
    let attacks = single_pawn_attacks(24, Color::White).unwrap(); // a4
    let b5 = 1u64 << 33;
    assert_eq!(attacks, b5);
}

#[test]
fn test_single_pawn_attacks_black_center() {
    // e5 pawn attacks d4 and f4
    let attacks = single_pawn_attacks(36, Color::Black).unwrap(); // e5 = index 36
    let d4 = 1u64 << 27;
    let f4 = 1u64 << 29;
    assert_eq!(attacks, d4 | f4);
}

#[test]
fn test_single_pawn_attacks_edge_cases() {
    // White pawn on 8th rank can't attack (already promoted)
    assert!(single_pawn_attacks(56, Color::White).is_none()); // a8

    // Black pawn on 1st rank can't attack (illegal position)
    assert!(single_pawn_attacks(0, Color::Black).is_none()); // a1
}

#[test]
fn test_pawn_attacks_starting_position() {
    let board = Board::new();
    let white_attacks = board.pawn_attacks(Color::White);
    let black_attacks = board.pawn_attacks(Color::Black);

    // White pawns on rank 2 attack rank 3
    // Black pawns on rank 7 attack rank 6
    assert!(white_attacks.0 & (1u64 << 16) != 0); // a3 attacked by b2 pawn
    assert!(black_attacks.0 & (1u64 << 40) != 0); // a6 attacked by b7 pawn
}

#[test]
fn test_all_attacks_starting_position() {
    let board = Board::new();
    let white_attacks = board.all_attacks(Color::White);
    let black_attacks = board.all_attacks(Color::Black);

    // Both sides attack many squares in starting position
    assert!(white_attacks.popcount() > 16);
    assert!(black_attacks.popcount() > 16);
}

#[test]
fn test_attack_context_symmetry() {
    let board = Board::new();
    let ctx = board.compute_attack_context();

    // Starting position should be symmetric
    assert_eq!(ctx.white_attacks.popcount(), ctx.black_attacks.popcount());
    assert_eq!(
        ctx.white_pawn_attacks.popcount(),
        ctx.black_pawn_attacks.popcount()
    );
}

#[test]
fn test_knight_attacks_in_all_attacks() {
    // Board with just a knight
    let board: Board = "8/8/8/8/4N3/8/8/8 w - - 0 1".parse().unwrap();
    let attacks = board.all_attacks(Color::White);

    // Knight on e4 attacks d2, f2, c3, g3, c5, g5, d6, f6
    assert_eq!(attacks.popcount(), 8);
}
