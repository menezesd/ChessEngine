//! Perft (performance test) for move generation correctness.

use crate::board::Board;
use std::time::Instant;

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
    TestPosition {
        name: "Position 3",
        fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        depths: &[(1, 14), (2, 191), (3, 2812), (4, 43238), (5, 674624)],
    },
    TestPosition {
        name: "Position 4",
        fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        depths: &[(1, 6), (2, 264), (3, 9467), (4, 422333)],
    },
    TestPosition {
        name: "Position 5",
        fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        depths: &[(1, 44), (2, 1486), (3, 62379), (4, 2103487)],
    },
    TestPosition {
        name: "Position 6 (Win at Chess)",
        fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        depths: &[(1, 46), (2, 2079), (3, 89890)],
    },
    TestPosition {
        name: "En Passant Capture",
        fen: "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
        depths: &[(1, 31), (2, 707), (3, 21637)],
    },
    TestPosition {
        name: "Promotion",
        fen: "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
        depths: &[(1, 24), (2, 496), (3, 9483)],
    },
    TestPosition {
        name: "Castling",
        fen: "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
        depths: &[(1, 26), (2, 568), (3, 13744)],
    },
];

#[test]
fn test_all_perft_positions() {
    for position in TEST_POSITIONS {
        let mut board = Board::from_fen(position.fen);

        for &(depth, expected) in position.depths {
            let start = Instant::now();
            let nodes = board.perft(depth);
            let duration = start.elapsed();

            println!("  Depth {}: {} nodes in {:?}", depth, nodes, duration);

            assert_eq!(
                nodes, expected,
                "Perft failed for position '{}' at depth {}. Expected: {}, Got: {}",
                position.name, depth, expected, nodes
            );
        }
    }
}
