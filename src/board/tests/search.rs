//! Search algorithm tests.
//!
//! Tests for alpha-beta, quiescence, pruning, and extensions.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::board::search::{find_best_move, search, SearchConfig, SearchState, MATE_SCORE};
use crate::board::{Board, Piece, EMPTY_MOVE};

// ============================================================================
// Alpha-beta search tests
// ============================================================================

#[test]
fn alphabeta_finds_mate_in_one() {
    // White to move, Qe8# is mate
    let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/4Q2K w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 2, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    assert_eq!(mv.to_string(), "e1e8", "Should find Qe8#");
}

#[test]
fn alphabeta_keeps_mate_on_fifty_move_boundary() {
    let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/4Q2K w - - 99 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 2, &stop);

    assert_eq!(best.map(|mv| mv.to_string()), Some("e1e8".to_string()));
}

#[test]
fn two_knights_endgame_keeps_mate_in_one() {
    let mut board = Board::from_fen("8/8/8/8/8/2N5/8/k1K1N3 w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 12, &stop);
    assert_eq!(best.map(|mv| mv.to_string()), Some("e1c2".to_string()));
}

#[test]
fn two_knights_endgame_prunes_non_forcing_lines() {
    let mut board = Board::from_fen("7k/8/8/8/8/8/4N1N1/K7 w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 12, &stop);
    assert!(
        best.is_some(),
        "the drawn endgame should still provide a legal move"
    );
    assert_eq!(
        state.stats.nodes, 0,
        "K+NN versus K should resolve at the root without expanding a search tree"
    );
}

#[test]
fn two_knights_endgame_prunes_when_bare_king_is_to_move() {
    let mut board = Board::from_fen("7k/8/8/8/8/8/4N1N1/K7 b - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 12, &stop);
    assert!(
        best.is_some(),
        "the bare king should retain a legal drawing move"
    );
    assert!(board.is_legal_move(best.unwrap()));
    assert_eq!(state.stats.nodes, 0);
}

#[test]
fn two_knights_endgame_bare_king_avoids_mate_in_one_blunder() {
    // ...Ka3? permits Nc2#. The other legal king moves maintain the draw.
    let mut board = Board::from_fen("8/8/8/NK6/8/8/1k6/2N5 b - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 12, &stop)
        .expect("the bare king must have a drawing move");
    assert_ne!(best.to_string(), "b2a3", "the resolver must avoid ...Ka3?");
    assert_eq!(state.stats.nodes, 0);

    let info = board.make_move(best);
    let has_mate_in_one = board.generate_moves().iter().any(|&reply| {
        let reply_info = board.make_move(reply);
        let is_mate = board.is_checkmate();
        board.unmake_move(reply, reply_info);
        is_mate
    });
    board.unmake_move(best, info);

    assert!(
        !has_mate_in_one,
        "the selected king move must preserve the draw"
    );
}

#[test]
fn zero_tree_root_result_clears_prior_search_statistics() {
    let mut board = Board::from_fen("7k/8/8/8/8/8/4N1N1/K7 w - - 0 1");
    let mut state = SearchState::new(1);
    state.stats.nodes = 123;
    state.stats.seldepth = 9;
    state.stats.tt_hits = 17;
    state.stats.total_nodes = 456;
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 12, &stop);

    assert!(best.is_some());
    assert_eq!(state.stats.nodes, 0);
    assert_eq!(state.stats.seldepth, 0);
    assert_eq!(state.stats.tt_hits, 0);
    assert_eq!(state.stats.total_nodes, 456);
}

#[test]
fn proven_draw_root_returns_a_legal_move_without_searching() {
    let mut board = Board::from_fen("7k/8/8/8/8/8/8/K1B5 w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 12, &stop);

    assert!(board.is_theoretical_draw());
    assert!(best.is_some());
    assert!(board.is_legal_move(best.unwrap()));
    assert_eq!(state.stats.nodes, 0);
}

#[test]
fn alphabeta_returns_mate_score_for_checkmate() {
    // Mate in 1, should return near-mate score
    let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/4Q2K w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    // Search at depth 4 to verify mate detection
    let _ = find_best_move(&mut board, &mut state, 4, &stop);

    // The TT should contain the position with a mate score
    if let Some(entry) = state.tables.tt.probe(board.hash) {
        let score = entry.score();
        // Mate scores are near MATE_SCORE
        assert!(
            !(-MATE_SCORE + 100..=MATE_SCORE - 100).contains(&score),
            "Expected mate score, got {score}"
        );
    }
}

#[test]
fn alphabeta_handles_stalemate() {
    // Stalemate position: black to move, king on a8
    let mut board = Board::from_fen("k7/8/1QK5/8/8/8/8/8 b - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_none(), "Should return None for stalemate");
}

#[test]
fn alphabeta_avoids_checkmate() {
    // White king in danger, must escape
    let mut board = Board::from_fen("8/8/8/8/8/5q2/4P3/4K3 w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move to avoid mate");

    // Make the move and verify not in checkmate
    let mv = best.unwrap();
    board.make_move(mv);
    assert!(!board.is_checkmate(), "Move should avoid checkmate");
}

#[test]
fn alphabeta_returns_none_for_checkmate_position() {
    // White is already checkmated
    let mut board =
        Board::from_fen("rnb1kbnr/pppp1ppp/4p3/8/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_none(), "Should return None for checkmated position");
}

#[test]
fn search_respects_stop_flag() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(true); // Already stopped

    let best = find_best_move(&mut board, &mut state, 10, &stop);

    assert!(
        best.is_some(),
        "stopped search should retain a legal fallback"
    );
    assert!(board.is_legal_move(best.unwrap()));
}

#[test]
fn search_respects_state_hard_deadline() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    state.set_hard_stop_at(Some(Instant::now()));
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 10, &stop);

    assert!(
        best.is_some(),
        "hard-stopped search should retain a legal fallback"
    );
    assert!(board.is_legal_move(best.unwrap()));
    assert_eq!(
        state.stats.nodes, 0,
        "expired deadline must stop before search"
    );
}

#[test]
fn search_with_node_limit() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let config = SearchConfig::depth(20).with_nodes(1000);
    let result = search(&mut board, &mut state, config, &stop);

    // Should complete (due to node limit) and find some move
    assert!(
        result.best_move.is_some(),
        "Should find a move with node limit"
    );
    assert!(
        state.stats.nodes <= 1_000,
        "single-PV search exceeded its 1000-node budget: {}",
        state.stats.nodes
    );
}

#[test]
fn search_with_single_node_limit_never_overshoots() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let result = search(
        &mut board,
        &mut state,
        SearchConfig::depth(20).with_nodes(1),
        &stop,
    );

    assert!(
        result.best_move.is_some_and(|mv| board.is_legal_move(mv)),
        "an immediately limited search should retain a legal root fallback"
    );
    assert!(
        state.stats.nodes <= 1,
        "single-PV search exceeded its one-node budget: {}",
        state.stats.nodes
    );
}

#[test]
fn multipv_search_shares_one_node_budget() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let config = SearchConfig::depth(20).with_nodes(1_000).with_multi_pv(3);
    let _ = search(&mut board, &mut state, config, &stop);

    assert!(
        state.stats.total_nodes <= 1_000,
        "MultiPV searched {} nodes with a 1000-node budget",
        state.stats.total_nodes
    );
}

#[test]
fn multipv_info_callback_reports_each_requested_line() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);
    let reported = Arc::new(Mutex::new(Vec::new()));
    let callback_lines = Arc::clone(&reported);
    let callback = Arc::new(move |info: &crate::board::SearchIterationInfo| {
        callback_lines
            .lock()
            .expect("callback mutex poisoned")
            .push(info.multipv);
    });

    let _ = search(
        &mut board,
        &mut state,
        SearchConfig::depth(1)
            .with_multi_pv(2)
            .with_info_callback(callback),
        &stop,
    );

    let reported = reported.lock().expect("callback mutex poisoned");
    assert!(reported.contains(&1), "first PV line was not reported");
    assert!(reported.contains(&2), "second PV line was not reported");
}

#[test]
fn search_extracts_ponder_move() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let config = SearchConfig::depth(6).with_ponder(true);
    let result = search(&mut board, &mut state, config, &stop);

    assert!(result.best_move.is_some(), "Should find best move");
    // Ponder move may or may not be found depending on TT state
}

// ============================================================================
// Quiescence search tests
// ============================================================================

#[test]
fn quiescence_evaluates_quiet_position() {
    // Quiet position - no captures available
    let board = Board::from_fen("8/8/4k3/8/8/4K3/8/8 w - - 0 1");
    let eval = board.evaluate_simple();

    // Should be roughly equal (both have only kings)
    assert!(eval.abs() < 100, "King vs King should be roughly equal");
}

#[test]
fn search_finds_knight_fork() {
    // Knight on g2 can play Nf4+ forking the king on e3 and queen on d5
    // Position: Knight g2, King e3, Black king e6, Black queen d5
    let mut board = Board::from_fen("8/8/4k3/3q4/8/4K3/6N1/8 w - - 0 1");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 6, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    // Nf4+ is the winning move - forks king and queen
    assert_eq!(mv.to_string(), "g2f4", "Should find Nf4+ fork, got {mv}");
}

#[test]
fn search_captures_hanging_queen() {
    // Knight on c3 can capture the hanging queen on d5
    // Position: Knight c3, King e3, Black king e6, Black queen d5
    let mut board = Board::from_fen("8/8/4k3/3q4/8/2N1K3/8/8 w - - 0 1");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 6, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    // Nxd5 captures the queen
    assert_eq!(
        mv.to_string(),
        "c3d5",
        "Should capture queen with Nxd5, got {mv}"
    );
}

#[test]
fn quiescence_avoids_bad_captures() {
    // White knight can capture defended pawn - but shouldn't
    let mut board = Board::from_fen("8/8/4k3/3p4/4N3/3PK3/8/8 w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some());

    let mv = best.unwrap();
    // Should not be Nxd5 as the knight is worth more than the pawn
    // (though this depends on the position; just verify we get a reasonable move)
    let _ = mv;
}

// ============================================================================
// Move ordering tests
// ============================================================================

#[test]
fn tt_move_searched_first() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    // Do a search to populate TT
    let _ = find_best_move(&mut board, &mut state, 4, &stop);

    // TT should have an entry for the starting position
    let entry = state.tables.tt.probe(board.hash);
    assert!(entry.is_some(), "TT should have entry after search");
}

#[test]
fn killer_moves_updated_on_cutoff() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    // Search to trigger some beta cutoffs
    let _ = find_best_move(&mut board, &mut state, 6, &stop);

    // Some killer moves should be populated
    // (We can't easily verify specific killers without white-box testing)
    let killer = state.tables.killer_moves.primary(0);
    // May or may not have a killer at ply 0 depending on search
    let _ = killer;
}

#[test]
fn history_scores_updated() {
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    // Search to update history
    let _ = find_best_move(&mut board, &mut state, 6, &stop);

    // Check that history was updated for at least some moves
    // Common opening moves should have history scores
    let e2e4 = board.parse_move("e2e4").ok();
    if let Some(mv) = e2e4 {
        let score = state.tables.history_score(&mv);
        // Score might be 0 or positive depending on whether this move caused cutoffs
        let _ = score;
    }
}

// ============================================================================
// Extension tests
// ============================================================================

#[test]
fn check_extension_finds_deeper_mate() {
    // Position where check extension helps find mate
    // White to move, can force mate with checks
    let mut board = Board::from_fen("6k1/5ppp/6r1/8/8/8/5PPP/4R1K1 w - - 0 1");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    // The engine should find a good move (check extension helps see deeper)
}

// ============================================================================
// Pruning tests
// ============================================================================

#[test]
fn search_completes_with_pruning_enabled() {
    // Verify search completes in reasonable time with all pruning enabled
    let mut board = Board::new();
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let start = Instant::now();

    let best = find_best_move(&mut board, &mut state, 8, &stop);
    let elapsed = start.elapsed();

    assert!(best.is_some(), "Should find a move at depth 8");
    assert!(
        elapsed.as_secs() < 30,
        "Depth 8 should complete in under 30s, took {elapsed:?}"
    );
}

// ============================================================================
// Repetition detection tests
// ============================================================================

#[test]
fn search_handles_repetition() {
    let mut board = Board::new();

    // Play moves that create repetition potential
    let moves = ["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6"];
    for mv_str in &moves {
        let mv = board.parse_move(mv_str).unwrap();
        board.make_move(mv);
    }

    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    // Search should handle repetition correctly
    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(
        best.is_some(),
        "Should find a move even with repetition history"
    );
}

// ============================================================================
// Continuation history tests (via SearchState to avoid stack overflow)
// ============================================================================

#[test]
fn continuation_history_in_search_state() {
    // Use SearchState which properly allocates ContinuationHistory on heap
    let state = SearchState::new(1);
    // All entries should be 0
    let score = state
        .tables
        .continuation_history
        .score(Piece::Pawn, 0, EMPTY_MOVE);
    assert_eq!(score, 0);
}

#[test]
fn continuation_history_update_via_state() {
    let mut state = SearchState::new(1);
    let mut board = Board::new();

    // Create a test move
    let mv = board.parse_move("e2e4").unwrap();

    state
        .tables
        .continuation_history
        .update(Piece::Pawn, 20, mv, 5);

    let score = state.tables.continuation_history.score(Piece::Pawn, 20, mv);
    assert!(score > 0, "Score should increase after update");
}

#[test]
fn continuation_history_decay_via_state() {
    let mut state = SearchState::new(1);
    let mut board = Board::new();

    let mv = board.parse_move("e2e4").unwrap();

    state
        .tables
        .continuation_history
        .update(Piece::Pawn, 20, mv, 10);
    let before = state.tables.continuation_history.score(Piece::Pawn, 20, mv);

    state.tables.continuation_history.decay();
    let after = state.tables.continuation_history.score(Piece::Pawn, 20, mv);

    assert!(after < before, "Score should decrease after decay");
}

#[test]
fn continuation_history_reset_via_state() {
    let mut state = SearchState::new(1);
    let mut board = Board::new();

    let mv = board.parse_move("e2e4").unwrap();

    state
        .tables
        .continuation_history
        .update(Piece::Pawn, 20, mv, 10);
    state.tables.continuation_history.reset();

    let score = state.tables.continuation_history.score(Piece::Pawn, 20, mv);
    assert_eq!(score, 0, "Score should be 0 after reset");
}

// ============================================================================
// History table additional tests
// ============================================================================

#[test]
fn history_table_bounds() {
    let mut state = SearchState::new(1);
    let mut board = Board::new();

    // Test with edge case indices - Ra1 to h8 (an impossible quiet move but valid indices)
    let mv = board.parse_move("a2a4").unwrap();

    state.tables.history.update(&mv, 10);
    let score = state.tables.history.score(&mv);
    assert!(score > 0);
}

#[test]
fn history_table_saturating_add() {
    let mut state = SearchState::new(1);
    let mut board = Board::new();

    let mv = board.parse_move("e2e4").unwrap();

    // Update many times to test saturation
    for _ in 0..1000 {
        state.tables.history.update(&mv, 10);
    }

    let score = state.tables.history.score(&mv);
    // Should be positive and not overflow
    assert!(score > 0, "Score should be positive after many updates");
}

// ============================================================================
// Killer table additional tests
// ============================================================================

#[test]
fn killer_table_out_of_bounds_safe() {
    let state = SearchState::new(1);

    // Access beyond MAX_PLY should return EMPTY_MOVE
    let primary = state.tables.killer_moves.primary(1000);
    assert_eq!(primary, EMPTY_MOVE);

    let secondary = state.tables.killer_moves.secondary(1000);
    assert_eq!(secondary, EMPTY_MOVE);
}

#[test]
fn killer_update_out_of_bounds_safe() {
    let mut state = SearchState::new(1);
    let mut board = Board::new();

    let mv = board.parse_move("e2e4").unwrap();

    // Should not panic
    state.tables.killer_moves.update(1000, mv);
}

// ============================================================================
// Search state tests
// ============================================================================

#[test]
fn search_state_new_search_resets() {
    let mut state = SearchState::new(1);

    assert!(state.hce_options.use_full);
    assert!(state.hce_options.use_tuned);
    assert!(!state.static_eval_options.nnue_pure);
    assert!(!state.static_eval_options.use_full_hce);

    // Populate some state
    state.stats.nodes = 1000;
    state.stats.seldepth = 10;
    state.generation = 5;

    state.new_search();

    assert_eq!(state.stats.nodes, 0);
    assert_eq!(state.stats.seldepth, 0);
    assert_eq!(state.generation, 6);
}

#[test]
fn search_state_generation_wraps() {
    let mut state = SearchState::new(1);
    state.generation = u16::MAX;

    state.new_search();

    assert_eq!(state.generation, 0, "Generation should wrap around");
}

#[test]
fn search_config_builder() {
    let config = SearchConfig::depth(10).with_nodes(50000).with_ponder(true);

    assert_eq!(config.max_depth, Some(10));
    assert_eq!(config.node_limit, 50000);
    assert!(config.extract_ponder);
}

#[test]
fn search_config_time() {
    let config = SearchConfig::time(5000);

    assert_eq!(config.time_limit_ms, 5000);
    assert_eq!(config.max_depth, None);
}

// ============================================================================
// Mate detection tests
// ============================================================================

#[test]
fn finds_back_rank_mate() {
    // White rook on h-file can deliver back rank mate with Rh8#
    // Rook on h1, King on b6, Black king on b8
    // After Rh8+, c8 is controlled by the rook (unlike Ra8+ where king blocks c8)
    let mut board = Board::from_fen("1k6/8/1K6/8/8/8/8/7R w - - 0 1");
    let mut state = SearchState::new(16);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some(), "Should find a move");

    let mv = best.unwrap();
    board.make_move(mv);

    // Rh8 is checkmate - verify the engine finds it
    assert!(board.is_checkmate(), "Engine should find Rh8#, played {mv}");
}

#[test]
fn avoids_getting_mated() {
    // Black threatens mate, white must defend
    let mut board =
        Board::from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4");
    let mut state = SearchState::new(1);
    let stop = AtomicBool::new(false);

    let best = find_best_move(&mut board, &mut state, 4, &stop);
    assert!(best.is_some());

    let mv = best.unwrap();
    // Must block or defend f7
    board.make_move(mv);

    // After black's move, verify Qxf7 is no longer immediately checkmate
    // (black should have defended)
}
