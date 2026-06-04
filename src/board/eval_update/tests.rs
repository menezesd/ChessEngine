use super::*;

#[test]
fn test_pst_square_white() {
    // White's a1 (index 0) stays 0
    assert_eq!(pst_square(0, true), 0);
    // White's h8 (index 63) stays 63
    assert_eq!(pst_square(63, true), 63);
}

#[test]
fn test_pst_square_black() {
    // Black's a1 (index 0) becomes a8 (index 56)
    assert_eq!(pst_square(0, false), 56);
    // Black's h8 (index 63) becomes h1 (index 7)
    assert_eq!(pst_square(63, false), 7);
}

#[test]
fn test_eval_state_add_remove() {
    let mut state = EvalState::new();

    // Add a white pawn at e2 (index 12)
    state.add_piece(0, Piece::Pawn, 12, true);
    assert!(state.mg[0] > 0, "adding pawn should increase mg");
    assert!(state.eg[0] > 0, "adding pawn should increase eg");

    // Remove it
    state.remove_piece(0, Piece::Pawn, 12, true);
    assert_eq!(state.mg[0], 0, "removing should restore to 0");
    assert_eq!(state.eg[0], 0, "removing should restore to 0");
}

#[test]
fn test_eval_state_new() {
    let state = EvalState::new();
    assert_eq!(state.mg, [0, 0]);
    assert_eq!(state.eg, [0, 0]);
    assert_eq!(state.phase, [0, 0]);
}

#[test]
fn test_eval_state_default() {
    let state = EvalState::default();
    assert_eq!(state.mg, [0, 0]);
    assert_eq!(state.eg, [0, 0]);
}

#[test]
fn test_move_piece() {
    let mut state = EvalState::new();

    // Add a knight at b1 (index 1)
    state.add_piece(0, Piece::Knight, 1, true);
    let initial_mg = state.mg[0];
    let initial_phase = state.phase[0];

    // Move to c3 (index 18)
    state.move_piece(0, Piece::Knight, 1, 18, true);

    // Knight on c3 should have different PST bonus than b1
    // (c3 is a better square for a knight than b1)
    assert_ne!(
        state.mg[0], initial_mg,
        "move_piece should update PST bonus"
    );
    // Phase should be the same (same piece, just moved)
    assert_eq!(
        state.phase[0], initial_phase,
        "phase should not change on move"
    );
}

#[test]
fn test_queen_has_high_phase() {
    let mut state = EvalState::new();

    state.add_piece(0, Piece::Queen, 3, true); // d1
    assert!(state.phase[0] >= 4, "queen should have high phase weight");
}

#[test]
fn test_pawn_has_no_phase() {
    let mut state = EvalState::new();

    state.add_piece(0, Piece::Pawn, 12, true); // e2
    assert_eq!(state.phase[0], 0, "pawns should not contribute to phase");
}

#[test]
fn test_board_eval_state() {
    let board = Board::new();
    let state = board.eval_state();

    // Starting position should have equal material for both sides
    assert_eq!(state.mg[0], state.mg[1], "symmetric mg");
    assert_eq!(state.eg[0], state.eg[1], "symmetric eg");
    assert_eq!(state.phase[0], state.phase[1], "symmetric phase");
}
