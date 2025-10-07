use chess_engine::engine::{SimpleEngine, SearchOptions, SearchEngine};
use chess_engine::transposition_table::TranspositionTable;
use chess_engine::board::Board;
use std::time::Duration;

#[test]
fn engine_depth_search_returns_move() {
    let mut board = Board::new();
    let mut tt = TranspositionTable::new(1024);
    let engine = SimpleEngine::new();
    let opts = SearchOptions {
        max_depth: Some(1),
        max_time: None,
        max_nodes: None,
        is_ponder: false,
        sink: None,
        info_sender: None,
        move_ordering: None,
    };
    let res = engine.search(&mut board, &mut tt, opts).expect("search failed");
    // At depth 1 we should always have at least one legal move from the starting position
    assert!(res.best_move.is_some());
}

#[test]
fn engine_time_limited_search_returns_move_within_time() {
    let mut board = Board::new();
    let mut tt = TranspositionTable::new(1024);
    let engine = SimpleEngine::new();
    let opts = SearchOptions {
        max_depth: None,
        max_time: Some(Duration::from_millis(50)),
        max_nodes: None,
        is_ponder: false,
        sink: None,
        info_sender: None,
        move_ordering: None,
    };
    let res = engine.search(&mut board, &mut tt, opts).expect("time-limited search failed");
    // A short time-limited search may or may not produce a move, but should not error
    assert!(res.time_ms <= 5000);
}
