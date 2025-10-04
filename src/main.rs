// use std::time::Instant; // Only used in tests below


mod attack_tables;
mod bench;
mod board;
mod bitboards;
mod pst;
mod search;

mod tactics;
mod tt;
mod uci;
mod feature_export;
mod selfplay;
#[allow(dead_code)]
#[allow(dead_code)]
// (Tuning modules removed; using only Publius values)

pub(crate) use board::{Board, Move, Piece, Square};
pub(crate) use board::piece_value;
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
    println!("    export-features <fenfile> <out.csv>  Export feature CSV for positions (one FEN per line)");
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
            // (Tuning command removed)
            }
            "tactics" => {
                let tactics_args = if args.len() > 2 { &args[2..] } else { &[] };
                tactics::run_tactical_test(tactics_args);
            }
            "search" => {
                println!("Testing search engine...");
                test_search_engine();
            }
            "export-features" => {
                if args.len() < 4 { eprintln!("Usage: chess_engine export-features <fenfile> <out.csv>"); return; }
                let fenfile = &args[2]; let outfile = &args[3];
                let data = std::fs::read_to_string(fenfile).expect("cannot read fen file");
                let mut boards = Vec::new();
                for line in data.lines() { let l=line.trim(); if l.is_empty() { continue; } boards.push(Board::from_fen(l)); }
                let out = std::fs::File::create(outfile).expect("cannot create output");
                feature_export::export_positions_to_csv(&boards, out, true).expect("write csv");
                println!("Exported {} positions to {}", boards.len(), outfile);
            }
            "selfplay-export" => {
                // Usage: chess_engine selfplay-export <plies> <sf_depth> <out.csv> [topK] [adjud_cp] [adjud_repeats] [min_plies] [rand_open_plies] [cp_clamp] [append]
                if args.len() < 5 { eprintln!("Usage: chess_engine selfplay-export <plies> <sf_depth> <out.csv> [topK] [adjud_cp] [adjud_repeats] [min_plies] [rand_open_plies] [cp_clamp] [append]"); return; }
                let plies: usize = args[2].parse().unwrap_or(60);
                let depth: u32 = args[3].parse().unwrap_or(8);
                let outfile = &args[4];
                let top_k = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(3);
                let adjud_cp = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(1200);
                let adjud_rep = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(6);
                let min_plies = args.get(8).and_then(|s| s.parse().ok()).unwrap_or(50);
                let random_open = args.get(9).and_then(|s| s.parse().ok()).unwrap_or(6);
                let cp_clamp = args.get(10).and_then(|s| s.parse().ok());
                let append = args.get(11).map(|s| s=="1" || s.eq_ignore_ascii_case("true")).unwrap_or(false);
                let cfg = selfplay::SelfPlayConfig { plies, sf_depth: depth, top_k_random: top_k, adjudication_cp: adjud_cp, adjudication_repeats: adjud_rep, min_plies_before_adjudication: min_plies, random_opening_plies: random_open, clamp_cp: cp_clamp, ..Default::default() };
                let boards = selfplay::generate_positions_vs_stockfish(&cfg);
                let path_exists = std::path::Path::new(outfile).exists();
                let mut out = std::fs::OpenOptions::new().create(true).append(append && path_exists).write(true).truncate(!(append && path_exists)).open(outfile).expect("cannot open output");
                feature_export::export_positions_to_csv(&boards, &mut out, !(append && path_exists)).expect("write csv");
                println!("Self-play (vs Stockfish) exported {} positions to {} (append={})", boards.len(), outfile, append);
            }
            "selfplay-labeled-export" => {
                // Usage: chess_engine selfplay-labeled-export <plies> <sf_depth> <out.csv> [topK] [adjud_cp] [adjud_repeats] [min_plies] [rand_open_plies] [cp_clamp] [append]
                if args.len() < 5 { eprintln!("Usage: chess_engine selfplay-labeled-export <plies> <sf_depth> <out.csv> [topK] [adjud_cp] [adjud_repeats] [min_plies] [rand_open_plies] [cp_clamp] [append]"); return; }
                let plies: usize = args[2].parse().unwrap_or(60);
                let depth: u32 = args[3].parse().unwrap_or(10);
                let outfile = &args[4];
                let top_k = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(3);
                let adjud_cp = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(1200);
                let adjud_rep = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(6);
                let min_plies = args.get(8).and_then(|s| s.parse().ok()).unwrap_or(50);
                let random_open = args.get(9).and_then(|s| s.parse().ok()).unwrap_or(6);
                let cp_clamp = args.get(10).and_then(|s| s.parse().ok());
                let append = args.get(11).map(|s| s=="1" || s.eq_ignore_ascii_case("true")).unwrap_or(false);
                let cfg = selfplay::SelfPlayConfig { plies, sf_depth: depth, top_k_random: top_k, adjudication_cp: adjud_cp, adjudication_repeats: adjud_rep, min_plies_before_adjudication: min_plies, random_opening_plies: random_open, clamp_cp: cp_clamp, ..Default::default() };
                let labeled = selfplay::generate_labeled_positions_vs_stockfish(&cfg);
                let just_boards: Vec<(Board, Option<i32>, Option<i32>, Option<i8>)> = labeled.into_iter().collect();
                let path_exists = std::path::Path::new(outfile).exists();
                let mut out = std::fs::OpenOptions::new().create(true).append(append && path_exists).write(true).truncate(!(append && path_exists)).open(outfile).expect("cannot open output");
                feature_export::export_labeled_positions_to_csv(&just_boards, &mut out, !(append && path_exists)).expect("write csv");
                println!("Labeled self-play exported {} positions to {} (append={})", just_boards.len(), outfile, append);
            }
            "selfplay-batch" => {
                // Usage: chess_engine selfplay-batch <games> <plies> <sf_depth> <out.csv> [topK] [rand_open] [cp_clamp] [adjud_cp] [adjud_repeats] [min_plies]
                if args.len() < 6 { eprintln!("Usage: chess_engine selfplay-batch <games> <plies> <sf_depth> <out.csv> [topK] [rand_open] [cp_clamp] [adjud_cp] [adjud_repeats] [min_plies]"); return; }
                let games: usize = args[2].parse().unwrap_or(50);
                let plies: usize = args[3].parse().unwrap_or(80);
                let depth: u32 = args[4].parse().unwrap_or(10);
                let outfile = &args[5];
                let top_k = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(5);
                let rand_open = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(8);
                let cp_clamp = args.get(8).and_then(|s| s.parse().ok());
                let adjud_cp = args.get(9).and_then(|s| s.parse().ok()).unwrap_or(1200);
                let adjud_repeats = args.get(10).and_then(|s| s.parse().ok()).unwrap_or(6);
                let min_plies = args.get(11).and_then(|s| s.parse().ok()).unwrap_or(50);
                let mut total_positions = 0usize;
                for g in 0..games {
                    let cfg = selfplay::SelfPlayConfig {
                        plies,
                        sf_depth: depth,
                        top_k_random: top_k,
                        random_opening_plies: rand_open,
                        clamp_cp: cp_clamp,
                        adjudication_cp: adjud_cp,
                        adjudication_repeats: adjud_repeats,
                        min_plies_before_adjudication: min_plies,
                        ..Default::default()
                    };
                    let labeled = selfplay::generate_labeled_positions_vs_stockfish(&cfg);
                    let just_boards: Vec<(Board, Option<i32>, Option<i32>, Option<i8>)> = labeled.into_iter().collect();
                    total_positions += just_boards.len();
                    let path_exists = std::path::Path::new(outfile).exists();
                    let append = true;
                    let mut out = std::fs::OpenOptions::new().create(true).append(append && path_exists).write(true).truncate(false).open(outfile).expect("cannot open output");
                    feature_export::export_labeled_positions_to_csv(&just_boards, &mut out, !path_exists).expect("write csv");
                    println!("[batch {}/{}] positions={} total={} file={}", g+1, games, just_boards.len(), total_positions, outfile);
                }
                println!("Batch complete: games={} total_positions={} output={}", games, total_positions, outfile);
            }
            "show-tuned-params" => {
            // (Show-tuned-params command removed)
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

// (Tuning-related eval tests removed)
