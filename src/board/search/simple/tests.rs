use super::*;

#[test]
fn increment_node_count_saturates_at_u64_max() {
    let mut nodes = u64::MAX;

    increment_node_count(&mut nodes);

    assert_eq!(nodes, u64::MAX);
}

#[test]
fn is_pv_window_handles_extreme_alpha() {
    assert!(!is_pv_window(i32::MAX, i32::MAX));
    assert!(is_pv_window(0, 2));
    assert!(!is_pv_window(0, 1));
}

#[test]
fn mate_score_for_ply_saturates_extreme_ply() {
    assert_eq!(mate_score_for_ply(-1, 0), -MATE_SCORE);
    assert_eq!(mate_score_for_ply(1, 0), MATE_SCORE);
    assert_eq!(mate_score_for_ply(-1, usize::MAX), i32::MAX);
    assert_eq!(mate_score_for_ply(1, usize::MAX), i32::MIN);
}

#[test]
fn singular_beta_saturates_extreme_margin() {
    assert_eq!(singular_beta(i32::MIN, i32::MAX, u32::MAX), i32::MIN);
    assert_eq!(singular_beta(i32::MAX, i32::MIN, u32::MAX), i32::MAX);
}

#[cfg(feature = "embedded_nnue")]
use crate::board::nnue::{NnueAccumulator, NnueNetwork};
#[cfg(feature = "embedded_nnue")]
use crate::board::Color;
#[cfg(feature = "embedded_nnue")]
use std::sync::Arc;

#[cfg(feature = "embedded_nnue")]
fn assert_incremental_eval_matches(fen: &str, uci: &str) {
    let Ok(loaded) = NnueNetwork::from_bytes(crate::board::nnue::network::EMBEDDED_NETWORK) else {
        return;
    };
    let network = Arc::new(loaded);
    let mut board = Board::from_fen(fen);
    let mut state = SearchState::new(16);
    state.tables.nnue = Some(Arc::clone(&network));
    let stop = AtomicBool::new(false);
    let mut ctx = SimpleSearchContext {
        board: &mut board,
        state: &mut state,
        stop: &stop,
        start_time: Instant::now(),
        time_limit_ms: 0,
        node_limit: 0,
        nodes: 0,
        initial_depth: 1,
        static_eval: [0; MAX_PLY],
        previous_move: [EMPTY_MOVE; MAX_PLY],
        previous_piece: [None; MAX_PLY],
        info_callback: None,
        root_moves: Vec::new(),
        acc_stack: vec![NnueAccumulator::default(); MAX_PLY].into_boxed_slice(),
        static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY].into_boxed_slice(),
    };

    ctx.init_accumulator(0);
    let mv = ctx.board.parse_move(uci).unwrap();
    let (moving_color, moving_piece) = ctx.board.piece_at(mv.from()).unwrap();
    ctx.update_accumulator_for_move(0, mv, moving_piece, moving_color);
    let _info = ctx.board.make_move(mv);

    let stm_is_white = ctx.board.side_to_move() == Color::White;
    let incremental = network.evaluate(&ctx.acc_stack[1], stm_is_white);
    let recomputed = ctx.board.evaluate_nnue(&network);

    assert_eq!(
        incremental, recomputed,
        "incremental NNUE mismatch for fen={fen} move={uci}"
    );
}

#[cfg(feature = "embedded_nnue")]
#[test]
fn incremental_nnue_matches_full_recompute_for_representative_moves() {
    assert_incremental_eval_matches(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "e2e4",
    );
    assert_incremental_eval_matches(
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 1",
        "e5d6",
    );
    assert_incremental_eval_matches("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1");
    assert_incremental_eval_matches("8/P7/8/8/8/8/8/K1k5 w - - 0 1", "a7a8q");
    assert_incremental_eval_matches(
        "rnbqkbnr/pppppppp/8/8/3p4/8/PPP1PPPP/RNBQKBNR w KQkq - 0 1",
        "c2c4",
    );
}
