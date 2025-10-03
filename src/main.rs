// use std::time::Instant; // Only used in tests below


mod attack_tables;
mod bench;
mod board;
mod bitboards;

mod search;

mod tactics;
mod tuned_configs;
mod tuning;
mod tt;
mod uci;

pub(crate) use board::{Board, Move, Piece, Square};
pub(crate) use board::{color_to_zobrist_index, square_to_zobrist_index};
pub(crate) use board::{mvv_lva_score, piece_value};
pub(crate) use tt::TranspositionTable;
use search::SearchEngine;

// Material values (used for mate score scaling only)
const KING_VALUE: i32 = 20000;
const MATE_SCORE: i32 = KING_VALUE * 10;

fn test_search_engine() {
    println!("Testing advanced search engine...");
    
    // Test with a tactical position (Kiwipete)
    let mut board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
    let mut engine = SearchEngine::new();
    
    println!("Position: Kiwipete (rich tactical position)");
    board.print();
    
    let time_limit = Some(std::time::Instant::now() + std::time::Duration::from_millis(2000));
    let best_move = engine.think(&mut board, time_limit);
    
    println!("Best move found: {}{}", 
        format_square(best_move.from), 
        format_square(best_move.to));
}

fn format_square(sq: Square) -> String {
    let file = (b'a' + sq.1 as u8) as char;
    let rank = (b'1' + sq.0 as u8) as char;
    format!("{}{}", file, rank)
}

fn print_help() {
    println!("Chess Engine - Advanced UCI-compliant chess engine");
    println!();
    println!("USAGE:");
    println!("    chess_engine [COMMAND] [OPTIONS]");
    println!();
    println!("COMMANDS:");
    println!("    uci                          Start UCI mode (default)");
    println!("    bench [compare]              Run performance benchmarks");
    println!("    tune [config]                Run parameter tuning");
    println!("    tactics [SOURCE] [N] [TIME]  Test tactical puzzle solving");
    println!("    search                       Test search engine");
    // Debug commands are intentionally not exposed in production builds
    println!("    help                         Show this help message");
    println!();
    println!("TACTICAL TESTING:");
    println!("    tactics                      Test 1001 brilliant checkmates (default)");
    println!("    tactics brilliant [N] [T]    Test N puzzles from brilliant collection (T ms each)");
    println!("    tactics famous [N] [T]       Test built-in famous mate positions");
    println!("    tactics FILE.pgn [N] [T]     Test puzzles from custom PGN file");
    println!();
    println!("EXAMPLES:");
    println!("    chess_engine                 # Start UCI mode");
    println!("    chess_engine bench            # Run standard benchmark");
    println!("    chess_engine bench compare    # Compare all configurations");
    println!("    chess_engine tactics          # Test brilliant checkmates");
    println!("    chess_engine tactics brilliant 25 4000  # Test 25 puzzles, 4s each");
    println!("    chess_engine tactics famous   # Test 5 built-in positions");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "bench" => {
                let bench_args = if args.len() > 2 { &args[2..] } else { &[] };
                bench::run_bench_command(bench_args);
            }
            "tune" => {
                let tune_args = if args.len() > 2 { &args[2..] } else { &[] };
                tuning::run_tuning_command(tune_args);
            }
            "tactics" => {
                let tactics_args = if args.len() > 2 { &args[2..] } else { &[] };
                tactics::run_tactical_test(tactics_args);
            }
            "search" => {
                println!("Testing search engine...");
                test_search_engine();
            }
            "help" | "--help" | "-h" => {
                print_help();
            }
            _ => uci::run(),
        }
    } else {
        uci::run();
    }
}

#[cfg(test)]
mod perft_tests {
    use super::*;
    use std::time::Instant;

    struct TestPosition {
        name: &'static str,
        fen: &'static str,
        depths: &'static [(usize, u64)], // (depth, expected node count)
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
                let _duration = start.elapsed();
                assert_eq!(nodes, expected,
                    "Perft failed for position '{}' at depth {}. Expected: {}, Got: {}",
                    position.name, depth, expected, nodes);
            }
        }
    }
}
