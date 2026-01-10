//! Tests for search tables: killer moves, history, counter moves, and MVV-LVA.

use crate::board::search::{CounterMoveTable, HistoryTable, KillerTable, SearchState};
use crate::board::state::Board;
use crate::board::{Move, Square, EMPTY_MOVE};

fn make_board(fen: &str) -> Board {
    fen.parse().expect("valid fen")
}

fn make_move(from: (usize, usize), to: (usize, usize)) -> Move {
    Move::quiet(Square::new(from.0, from.1), Square::new(to.0, to.1))
}

// ============================================================================
// Killer Move Tests
// ============================================================================

#[test]
fn test_killer_primary_empty_initially() {
    let table = KillerTable::new();
    for ply in 0..10 {
        assert_eq!(table.primary(ply), EMPTY_MOVE);
        assert_eq!(table.secondary(ply), EMPTY_MOVE);
    }
}

#[test]
fn test_killer_update_sets_primary() {
    let mut table = KillerTable::new();
    let mv = make_move((1, 4), (3, 4)); // e2-e4

    table.update(0, mv);
    assert_eq!(table.primary(0), mv);
    assert_eq!(table.secondary(0), EMPTY_MOVE);
}

#[test]
fn test_killer_update_shifts_to_secondary() {
    let mut table = KillerTable::new();
    let mv1 = make_move((1, 4), (3, 4)); // e2-e4
    let mv2 = make_move((1, 3), (3, 3)); // d2-d4

    table.update(0, mv1);
    table.update(0, mv2);

    assert_eq!(table.primary(0), mv2);
    assert_eq!(table.secondary(0), mv1);
}

#[test]
fn test_killer_same_move_no_duplicate() {
    let mut table = KillerTable::new();
    let mv = make_move((1, 4), (3, 4)); // e2-e4

    table.update(0, mv);
    table.update(0, mv); // Same move again

    assert_eq!(table.primary(0), mv);
    assert_eq!(table.secondary(0), EMPTY_MOVE); // Should not duplicate
}

#[test]
fn test_killer_different_plies_independent() {
    let mut table = KillerTable::new();
    let mv0 = make_move((1, 4), (3, 4));
    let mv1 = make_move((6, 4), (4, 4));

    table.update(0, mv0);
    table.update(1, mv1);

    assert_eq!(table.primary(0), mv0);
    assert_eq!(table.primary(1), mv1);
}

#[test]
fn test_killer_reset_clears_all() {
    let mut table = KillerTable::new();
    let mv = make_move((1, 4), (3, 4));

    table.update(0, mv);
    table.update(5, mv);
    table.reset();

    assert_eq!(table.primary(0), EMPTY_MOVE);
    assert_eq!(table.primary(5), EMPTY_MOVE);
}

#[test]
fn test_killer_out_of_bounds_safe() {
    let mut table = KillerTable::new();
    let mv = make_move((1, 4), (3, 4));

    // Should not panic
    table.update(1000, mv);
    assert_eq!(table.primary(1000), EMPTY_MOVE);
}

// ============================================================================
// History Table Tests
// ============================================================================

#[test]
fn test_history_initial_zero() {
    let table = HistoryTable::new();
    let mv = make_move((1, 4), (3, 4));
    assert_eq!(table.score(&mv), 0);
}

#[test]
fn test_history_update_increases_score() {
    let mut table = HistoryTable::new();
    let mv = make_move((1, 4), (3, 4));

    table.update(&mv, 3, 0);
    assert!(table.score(&mv) > 0);
}

#[test]
fn test_history_higher_depth_higher_bonus() {
    let mut table = HistoryTable::new();
    let mv1 = make_move((1, 4), (3, 4));
    let mv2 = make_move((1, 3), (3, 3));

    table.update(&mv1, 2, 0); // depth 2: bonus = 8
    table.update(&mv2, 4, 0); // depth 4: bonus = 64

    assert!(table.score(&mv2) > table.score(&mv1));
}

#[test]
fn test_history_accumulates() {
    let mut table = HistoryTable::new();
    let mv = make_move((1, 4), (3, 4));

    table.update(&mv, 2, 0);
    let score1 = table.score(&mv);
    table.update(&mv, 2, 0);
    let score2 = table.score(&mv);

    assert!(score2 > score1);
}

#[test]
fn test_history_decay_reduces_scores() {
    let mut table = HistoryTable::new();
    let mv = make_move((1, 4), (3, 4));

    table.update(&mv, 5, 0);
    let before = table.score(&mv);
    table.decay();
    let after = table.score(&mv);

    assert!(after < before);
}

#[test]
fn test_history_reset_clears() {
    let mut table = HistoryTable::new();
    let mv = make_move((1, 4), (3, 4));

    table.update(&mv, 5, 0);
    table.reset();

    assert_eq!(table.score(&mv), 0);
}

// ============================================================================
// Counter Move Table Tests
// ============================================================================

#[test]
fn test_counter_initial_empty() {
    let table = CounterMoveTable::new();
    assert_eq!(table.get(12, 28), EMPTY_MOVE); // e2 -> e4
}

#[test]
fn test_counter_set_and_get() {
    let mut table = CounterMoveTable::new();
    let counter = make_move((6, 4), (4, 4)); // e7-e5

    table.set(12, 28, counter); // After e2-e4
    assert_eq!(table.get(12, 28), counter);
}

#[test]
fn test_counter_overwrites() {
    let mut table = CounterMoveTable::new();
    let counter1 = make_move((6, 4), (4, 4));
    let counter2 = make_move((6, 3), (4, 3));

    table.set(12, 28, counter1);
    table.set(12, 28, counter2);

    assert_eq!(table.get(12, 28), counter2);
}

#[test]
fn test_counter_reset() {
    let mut table = CounterMoveTable::new();
    let counter = make_move((6, 4), (4, 4));

    table.set(12, 28, counter);
    table.reset();

    assert_eq!(table.get(12, 28), EMPTY_MOVE);
}

#[test]
fn test_counter_out_of_bounds_safe() {
    let mut table = CounterMoveTable::new();
    let mv = make_move((1, 4), (3, 4));

    // Should not panic and return EMPTY_MOVE
    table.set(100, 50, mv);
    assert_eq!(table.get(100, 50), EMPTY_MOVE);
}

// ============================================================================
// MVV-LVA Tests
// ============================================================================

#[test]
fn test_mvv_lva_pawn_takes_queen() {
    let board = make_board("8/8/3q4/4P3/8/8/8/8 w - - 0 1");
    let state = SearchState::new(1);
    let mv = Move::capture(Square::new(4, 4), Square::new(5, 3)); // exd6

    let score = state.tables.mvv_lva_score(&board, &mv);
    // Queen (900) * 10 - Pawn (100) = 8900
    assert!(score > 0);
    assert!(score > 8000); // High score for capturing queen with pawn
}

#[test]
fn test_mvv_lva_queen_takes_pawn() {
    let board = make_board("8/8/3p4/4Q3/8/8/8/8 w - - 0 1");
    let state = SearchState::new(1);
    let mv = Move::capture(Square::new(4, 4), Square::new(5, 3)); // Qxd6

    let score = state.tables.mvv_lva_score(&board, &mv);
    // Pawn (100) * 10 - Queen (900) = 100
    assert!(score > 0); // Still positive but low
    assert!(score < 200);
}

#[test]
fn test_mvv_lva_equal_trade() {
    let board = make_board("8/8/3n4/4N3/8/8/8/8 w - - 0 1");
    let state = SearchState::new(1);
    let mv = Move::capture(Square::new(4, 4), Square::new(5, 3)); // Nxd6

    let score = state.tables.mvv_lva_score(&board, &mv);
    // Knight (320) * 10 - Knight (320) = 2880
    assert!(score > 0);
}

#[test]
fn test_mvv_lva_non_capture_zero() {
    let board = make_board("8/8/8/4N3/8/8/8/8 w - - 0 1");
    let state = SearchState::new(1);
    let mv = Move::quiet(Square::new(4, 4), Square::new(5, 2)); // Ne5-c6 (quiet)

    let score = state.tables.mvv_lva_score(&board, &mv);
    assert_eq!(score, 0);
}

#[test]
fn test_mvv_lva_ordering_correct() {
    // Position where multiple captures possible
    let board = make_board("8/8/2pq4/3PN3/8/8/8/8 w - - 0 1");
    let state = SearchState::new(1);

    // Pawn takes queen (d5xd6)
    let pxq = Move::capture(Square::new(4, 3), Square::new(5, 3));
    // Knight takes queen
    let nxq = Move::capture(Square::new(4, 4), Square::new(5, 3));
    // Knight takes pawn
    let nxp = Move::capture(Square::new(4, 4), Square::new(5, 2));

    let score_pxq = state.tables.mvv_lva_score(&board, &pxq);
    let score_nxq = state.tables.mvv_lva_score(&board, &nxq);
    let score_nxp = state.tables.mvv_lva_score(&board, &nxp);

    // PxQ should be best (high value victim, low value attacker)
    assert!(score_pxq > score_nxq);
    // NxQ should be better than NxP
    assert!(score_nxq > score_nxp);
}

#[test]
fn test_mvv_lva_en_passant() {
    let board = make_board("8/8/8/3Pp3/8/8/8/8 w - e6 0 1");
    let state = SearchState::new(1);
    let mv = Move::en_passant(Square::new(4, 3), Square::new(5, 4)); // dxe6 e.p.

    let score = state.tables.mvv_lva_score(&board, &mv);
    // Should use pawn value for victim
    assert!(score > 0);
}

// ============================================================================
// Integration: Move Ordering Score Priority
// ============================================================================

#[test]
fn test_move_ordering_captures_positive() {
    // MVV-LVA scores should be positive for good captures
    let board = make_board("8/8/3q4/4P3/8/8/8/8 w - - 0 1");
    let state = SearchState::new(1);

    // Pawn captures queen
    let mv = Move::capture(Square::new(4, 4), Square::new(5, 3));
    let score = state.tables.mvv_lva_score(&board, &mv);
    assert!(score > 0, "Capturing queen should have positive MVV-LVA");
}
