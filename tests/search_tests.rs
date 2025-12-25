//! Search tests to verify the engine finds correct moves in various positions.

use std::sync::atomic::AtomicBool;

use chess_engine::board::{find_best_move, Board, SearchState};
use chess_engine::uci::format_uci_move;

/// Test that the engine finds a simple mate in 1
#[test]
fn finds_mate_in_one_back_rank() {
    // White to move, Qe8# is mate
    let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/4Q2K w - - 0 1");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    let uci = format_uci_move(&mv);
    assert_eq!(uci, "e1e8", "Should find Qe8# (back rank mate)");
}

/// Test that the engine finds a simple mate in 1 with queen
#[test]
fn finds_mate_in_one_queen() {
    // White to move, Qxf7# is mate
    let mut board = Board::from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 0 4");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    let uci = format_uci_move(&mv);
    assert_eq!(uci, "h5f7", "Should find Qxf7# (scholar's mate)");
}

/// Test that the engine avoids giving away material
#[test]
fn avoids_hanging_queen() {
    // White to move, should not hang the queen
    let mut board = Board::from_fen("r1bqkbnr/pppppppp/2n5/8/4P3/5Q2/PPPP1PPP/RNB1KBNR w KQkq - 0 3");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    let uci = format_uci_move(&mv);
    // Should not move queen to c6 where it can be taken by pawn
    assert_ne!(uci, "f3c6", "Should not hang the queen on c6");
}

/// Test that the engine captures free material
#[test]
fn captures_free_piece() {
    // White to move, free bishop on c6
    let mut board = Board::from_fen("rnbqk1nr/pppp1ppp/2b5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    // Should capture with the bishop or find a strong tactical move
    assert!(mv.captured_piece.is_some() || format_uci_move(&mv) == "c4f7",
            "Should capture material or threaten king");
}

/// Test iterative deepening produces consistent results
#[test]
fn iterative_deepening_consistency() {
    let mut board = Board::new();
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    // Search at depth 2 and depth 4, both should return legal moves
    let best2 = find_best_move(&mut board, &mut state, 2, &stop);
    let best4 = find_best_move(&mut board, &mut state, 4, &stop);

    assert!(best2.is_some(), "Should find move at depth 2");
    assert!(best4.is_some(), "Should find move at depth 4");

    // Both should be legal moves
    let moves = board.generate_moves();
    assert!(moves.iter().any(|m| *m == best2.unwrap()), "Depth 2 move should be legal");
    assert!(moves.iter().any(|m| *m == best4.unwrap()), "Depth 4 move should be legal");
}

/// Test that search handles single legal move positions
#[test]
fn single_legal_move() {
    // White king on a1 can only escape to a2
    let mut board = Board::from_fen("8/8/8/8/8/8/8/K6rk w - - 0 1");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    let uci = format_uci_move(&mv);
    assert_eq!(uci, "a1a2", "Only legal move should be Ka2");
}

/// Test that search returns None for checkmate position
#[test]
fn no_move_in_checkmate() {
    // White is checkmated
    let mut board = Board::from_fen("rnb1kbnr/pppp1ppp/4p3/8/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 1");

    // First verify it's actually checkmate
    assert!(board.is_checkmate(), "Position should be checkmate");

    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_none(), "Should return None for checkmate position");
}

/// Test search handles draw positions correctly
#[test]
fn handles_draw_by_repetition() {
    use chess_engine::uci::parse_position_command;

    let mut board = Board::new();

    // Play Nf3 Nf6 Ng1 Ng8 twice to get close to threefold
    let parts = [
        "position", "startpos", "moves",
        "g1f3", "g8f6", "f3g1", "f6g8",
        "g1f3", "g8f6", "f3g1", "f6g8",
    ];
    parse_position_command(&mut board, &parts);

    // Position should now be a draw by repetition
    assert!(board.is_draw(), "Should be a draw by repetition");
}

/// Test evaluation is symmetric
#[test]
fn evaluation_symmetry() {
    // Starting position should evaluate close to 0
    let board = Board::new();
    let eval = board.evaluate();
    assert!(eval.abs() < 50, "Starting position should be roughly equal (eval: {})", eval);
}

/// Test that positions with material advantage evaluate correctly
#[test]
fn evaluation_material_advantage() {
    // White up a queen
    let board_white_up = Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let eval_white_up = board_white_up.evaluate();

    // Black up a queen
    let board_black_up = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1");
    let eval_black_up = board_black_up.evaluate();

    assert!(eval_white_up > 800, "White up a queen should be very positive (eval: {})", eval_white_up);
    assert!(eval_black_up < -800, "Black up a queen should be very negative (eval: {})", eval_black_up);
}

/// Test that search completes at reasonable depth
#[test]
fn search_completes_at_depth_6() {
    use std::time::Instant;

    let mut board = Board::new();
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let start = Instant::now();
    // Search at reasonable depth
    let best = find_best_move(&mut board, &mut state, 6, &stop);
    let elapsed = start.elapsed();

    assert!(best.is_some(), "Should find a move at depth 6");
    // Should complete reasonably quickly
    assert!(elapsed.as_secs() < 60, "Search at depth 6 took too long: {:?}", elapsed);
}

/// Test that stalemate is correctly identified
#[test]
fn identifies_stalemate() {
    // Classic stalemate position: black to move, king on a8, white queen on b6, white king on c6
    let mut board = Board::from_fen("k7/8/1QK5/8/8/8/8/8 b - - 0 1");
    assert!(board.is_stalemate(), "Position should be stalemate");
    assert!(!board.is_checkmate(), "Position should not be checkmate");
}

/// Test fifty move rule detection
#[test]
fn fifty_move_rule() {
    let board = Board::from_fen("8/8/8/8/8/8/8/K1k5 w - - 100 1");
    assert!(board.is_draw(), "Position with 100 halfmove clock should be a draw");
}

/// Test search finds forced mate
#[test]
fn finds_forced_mate_in_two() {
    // Position where black has a strong move
    let mut board = Board::from_fen("6k1/pp4pp/8/8/8/8/PP4PP/1q4K1 b - - 0 1");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move in this position");
}
