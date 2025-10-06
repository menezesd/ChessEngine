use std::io;
use std::io::BufRead;
use std::time::{Duration, Instant};

// Module declarations
mod types;
mod zobrist;
mod utils;
mod board;
mod perft;

// Imports from modules
use board::{Board, mvv_lva_score};
use perft::perft;
use types::*;

// Material values
const MATE_SCORE: i32 = 200000;

// Parses a move in UCI format (e.g., "e2e4", "e7e8q")
// Needs the current board state to find the matching legal move object.
fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    if uci_string.len() < 4 || uci_string.len() > 5 {
        return None; // Invalid length
    }

    let from_chars: Vec<char> = uci_string.chars().take(2).collect();
    let to_chars: Vec<char> = uci_string.chars().skip(2).take(2).collect();

    if from_chars.len() != 2 || to_chars.len() != 2 {
        return None; // Should not happen with length check, but be safe
    }

    // Basic validation of chars
    if !('a'..='h').contains(&from_chars[0])
        || !('1'..='8').contains(&from_chars[1])
        || !('a'..='h').contains(&to_chars[0])
        || !('1'..='8').contains(&to_chars[1])
    {
        return None; // Invalid algebraic notation characters
    }

    let from_file = file_to_index(from_chars[0]);
    let from_rank = rank_to_index(from_chars[1]);
    let to_file = file_to_index(to_chars[0]);
    let to_rank = rank_to_index(to_chars[1]);

    let from_sq = Square(from_rank, from_file);
    let to_sq = Square(to_rank, to_file);

    // Handle promotion
    let promotion_piece = if uci_string.len() == 5 {
        match uci_string.chars().nth(4) {
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            _ => return None, // Invalid promotion character
        }
    } else {
        None
    };

    // Find the matching legal move
    // We need generate_moves, which takes &mut self. This is slightly awkward
    // if we just want to *find* the move without changing state yet.
    // A temporary clone *might* be acceptable here, or we pass the pre-generated list.
    // Let's generate moves here.
    let legal_moves = board.generate_moves(); // Needs &mut borrow

    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            // Check for promotion match
            if legal_move.promotion == promotion_piece {
                // Found the move! Return a clone of it.
                return Some(legal_move.clone());
            }
            // If no promotion specified by user AND move is not a promotion, it's a match
            else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move.clone());
            }
        }
    }

    None // No matching legal move found
}

fn find_best_move(board: &mut Board, tt: &mut TranspositionTable, max_depth: u32) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut best_score = -MATE_SCORE * 2;

    let legal_moves = board.generate_moves();
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves[0]); // No need to search further
    }
    let mut root_moves = legal_moves.clone(); // Reuse for move ordering

    // Iterative Deepening Loop
    for depth in 1..=max_depth {
        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut current_best_score = -MATE_SCORE * 2;
        let mut current_best_move: Option<Move> = None;

        // Optional: order moves using hash move from TT
        if let Some(entry) = tt.probe(board.hash) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = root_moves.iter().position(|m| m == hm) {
                    root_moves.swap(0, pos);
                }
            }
        }

        for m in &root_moves {
            let info = board.make_move(m);
            let score = -board.negamax(tt, depth - 1, -beta, -alpha);
            board.unmake_move(m, info);

            if score > current_best_score {
                current_best_score = score;
                current_best_move = Some(*m);
            }

            alpha = alpha.max(current_best_score);
        }

        if let Some(mv) = current_best_move {
            best_score = current_best_score;
            best_move = Some(mv);

            // Optional: reorder root_moves so best move is searched first in next iteration
            if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                root_moves.swap(0, pos);
            }
        }
    }

    best_move
}

fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}

fn parse_position_command(board: &mut Board, parts: &[&str]) {
    let mut i = 1;
    if i < parts.len() && parts[i] == "startpos" {
        *board = Board::new();
        i += 1;
    } else if i < parts.len() && parts[i] == "fen" {
        let fen = parts[i + 1..i + 7].join(" ");
        *board = Board::from_fen(&fen);
        i += 7;
    }

    if i < parts.len() && parts[i] == "moves" {
        i += 1;
        while i < parts.len() {
            if let Some(mv) = parse_uci_move(board, parts[i]) {
                board.make_move(&mv);
            } else {
                eprintln!("Invalid move: {}", parts[i]);
            }
            i += 1;
        }
    }
}

fn format_uci_move(mv: &Move) -> String {
    let mut s = format!("{}{}", format_square(mv.from), format_square(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => '?',
        });
    }
    s
}

fn find_best_move_with_time(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1;
    let mut last_depth_time = Duration::from_millis(1); // Prevent div-by-zero on first estimate

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0; // Each depth takes ~2× longer

    while start_time.elapsed() + SAFETY_MARGIN < max_time {
        let elapsed = start_time.elapsed();
        let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();

        // Estimate whether we have enough time for the next depth
        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if estimated_next_time + SAFETY_MARGIN > time_remaining {
            break; // Not enough time for another full depth
        }

        let depth_start = Instant::now();

        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut best_score = -MATE_SCORE * 2;
        let mut legal_moves = board.generate_moves();

        if legal_moves.is_empty() {
            return None;
        }

        if legal_moves.len() == 1 {
            return Some(legal_moves[0]); // No need to search further
        }

        // MVV-LVA and TT move ordering
        legal_moves.sort_by_key(|m| -mvv_lva_score(m, board));
        if let Some(entry) = tt.probe(board.hash) {
            if let Some(hm) = &entry.best_move {
                if let Some(pos) = legal_moves.iter().position(|m| m == hm) {
                    legal_moves.swap(0, pos);
                }
            }
        }

        let mut new_best_move = None;

        for m in &legal_moves {
            if start_time.elapsed() + SAFETY_MARGIN >= max_time {
                break;
            }

            let info = board.make_move(m);
            let score = -board.negamax(tt, depth - 1, -beta, -alpha);
            board.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                new_best_move = Some(*m);
            }

            alpha = alpha.max(best_score);
        }

        // Only update result if completed full depth in time
        if start_time.elapsed() + SAFETY_MARGIN < max_time {
            best_move = new_best_move;
            last_depth_time = depth_start.elapsed();
            depth += 1;
        } else {
            break;
        }
    }

    best_move
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut board = Board::new();
    let mut tt = TranspositionTable::new(1024); // 1024MB TT
    let search_depth = 6;

    let mut time_left = Duration::from_secs(5); // fallback
    let mut inc = Duration::ZERO;
    let mut movetime: Option<Duration> = None;

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "uci" => {
                println!("id name MyRustEngine");
                println!("id author Dean Menezes");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "ucinewgame" => {
                board = Board::new();
            }
            "position" => {
                parse_position_command(&mut board, &parts);
            }
            "go" => {
                let mut i = 1;
                while i < parts.len() {
                    match parts[i] {
                        "wtime" if board.white_to_move => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "btime" if !board.white_to_move => {
                            time_left = Duration::from_millis(parts[i + 1].parse().unwrap_or(1000));
                            i += 2;
                        }
                        "winc" if board.white_to_move => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "binc" if !board.white_to_move => {
                            inc = Duration::from_millis(parts[i + 1].parse().unwrap_or(0));
                            i += 2;
                        }
                        "movetime" => {
                            movetime =
                                Some(Duration::from_millis(parts[i + 1].parse().unwrap_or(100)));
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }

                let max_time = movetime.unwrap_or_else(|| time_left / 30 + inc);
                let start = Instant::now();

                if let Some(best_move) =
                    find_best_move_with_time(&mut board, &mut tt, max_time, start)
                {
                    let uci_move = format_uci_move(&best_move);
                    println!("bestmove {}", uci_move);
                } else {
                    println!("bestmove 0000");
                }
            }
            "quit" => break,
            _ => {
                // Ignore unknown commands or log them if needed
            }
        }

        stdout.flush().unwrap();
    }
}

#[cfg(test)]
mod perft_tests {
    use super::*;

    struct TestPosition {
        name: &'static str,
        fen: &'static str,
        depths: &'static [(usize, u64)], // (depth, expected node count)
    }

    // Common test positions with known perft results
    const TEST_POSITIONS: &[TestPosition] = &[
        // Initial position
        TestPosition {
            name: "Initial Position",
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            depths: &[
                (1, 20),      // 20 possible moves from initial position
                (2, 400),     // 400 positions after 2 plies
                (3, 8902),    // 8,902 positions after 3 plies
                (4, 197281),  // 197,281 positions after 4 plies
                (5, 4865609), // 4,865,609 positions after 5 plies
            ],
        },
        // Position 2 (Kiwipete)
        TestPosition {
            name: "Kiwipete",
            fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            depths: &[(1, 48), (2, 2039), (3, 97862), (4, 4085603)],
        },
        // Position 3
        TestPosition {
            name: "Position 3",
            fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            depths: &[(1, 14), (2, 191), (3, 2812), (4, 43238), (5, 674624)],
        },
        // Position 4
        TestPosition {
            name: "Position 4",
            fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            depths: &[(1, 6), (2, 264), (3, 9467), (4, 422333)],
        },
        // Position 5
        TestPosition {
            name: "Position 5",
            fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            depths: &[(1, 44), (2, 1486), (3, 62379), (4, 2103487)],
        },
        // Position 6 (Win at Chess)
        TestPosition {
            name: "Position 6 (Win at Chess)",
            fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            depths: &[
                (1, 46),
                (2, 2079),
                (3, 89890),
                //(4, 3894594), // Commented out as it may take too long
            ],
        },
        // Additional edge cases
        TestPosition {
            name: "En Passant Capture",
            fen: "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
            depths: &[
                (1, 31), // Includes en passant capture
                (2, 707),
                (3, 21637),
            ],
        },
        TestPosition {
            name: "Promotion",
            fen: "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
            depths: &[
                (1, 24), // Many promotion options
                (2, 496),
                (3, 9483),
            ],
        },
        TestPosition {
            name: "Castling",
            fen: "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
            depths: &[
                (1, 26), // Both sides can castle in both directions
                (2, 568),
                (3, 13744),
            ],
        },
    ];

    #[test]
    fn test_all_perft_positions() {
        for position in TEST_POSITIONS {
            println!("Testing position: {}", position.name);
            println!("FEN: {}", position.fen);

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
            println!("------------------------------");
        }
    }
}
