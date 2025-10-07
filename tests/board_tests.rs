use chess_engine::board::Board;
use chess_engine::transposition_table::TranspositionTable;
use chess_engine::types::{Square, Move};

#[test]
fn perft_positions() {
    // Reuse the perft test set from the inlined tests
    struct TestPosition {
        name: &'static str,
        fen: &'static str,
        depths: &'static [(usize, u64)],
    }

    const TEST_POSITIONS: &[TestPosition] = &[
        TestPosition {
            name: "Initial Position",
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depths: &[(1, 20), (2, 400), (3, 8902), (4, 197281), (5, 4865609)],
        },
        TestPosition {
            name: "Kiwipete",
            fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            depths: &[(1, 48), (2, 2039), (3, 97862), (4, 4085603)],
        },
        // ... other positions omitted for brevity; keep core checks small in integration test
    ];

    for position in TEST_POSITIONS {
        let mut board = Board::from_fen(position.fen);
        for &(depth, expected) in position.depths {
            let nodes = board.perft(depth);
            assert_eq!(nodes, expected, "Perft failed for {} at depth {}", position.name, depth);
        }
    }
}

// Move other smaller unit tests similarly — keep a selection to validate make/unmake and draw detection

#[test]
fn test_draw_detection_50_move() {
    let mut board = Board::from_fen("8/8/8/8/8/8/8/K6k w - - 0 1");
    board.halfmove_clock = 99;
    board.position_history.clear();
    board.position_history.push(board.hash);
    let mv = Move {
        from: Square(0, 0),
        to: Square(0, 1),
        promotion: None,
        is_castling: false,
        is_en_passant: false,
        captured_piece: None,
    };
    let info = board.make_move(&mv);
    assert!(board.is_draw());
    board.unmake_move(&mv, info);
}

#[test]
fn test_transposition_table_store_probe() {
    let mut tt = TranspositionTable::new(1);
    let hash = 0xdeadbeefu64;
    use chess_engine::transposition_table::BoundType;
    tt.store(hash, 1, 100, BoundType::Exact, None);
    let entry = tt.probe(hash).expect("Entry missing");
    assert_eq!(entry.depth, 1);
    tt.store(hash, 0, 50, BoundType::Exact, None);
    let entry2 = tt.probe(hash).expect("Entry missing after shallower store");
    assert_eq!(entry2.depth, 1);
    tt.store(hash, 5, 200, BoundType::Exact, None);
    let entry3 = tt.probe(hash).expect("Entry missing after deeper store");
    assert_eq!(entry3.depth, 5);
}
