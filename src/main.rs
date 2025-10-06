mod bitboard;
mod board;
mod perft;
mod types;
mod zobrist;

use std::io;
use std::time::Instant;

// Import from modules
use crate::board::*;
use crate::types::*;

// Piece-square tables (from white's perspective, flip for black)
const PAWN_PST: [i32; 64] = [
    0,   0,   0,   0,   0,   0,   0,   0,
    50,  50,  50,  50,  50,  50,  50,  50,
    10,  10,  20,  30,  30,  20,  10,  10,
    5,   5,   10,  25,  25,  10,  5,   5,
    0,   0,   0,   20,  20,   0,   0,   0,
    5,   -5,  -10,  0,   0,   -10, -5,  5,
    5,   10,  10,  -20, -20,  10,  10,  5,
    0,   0,   0,   0,   0,   0,   0,   0
];

const KNIGHT_PST: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,  0,   0,   0,   0,   -20, -40,
    -30,  0,   10,  15,  15,  10,  0,   -30,
    -30,  5,   15,  20,  20,  15,  5,   -30,
    -30,  0,   15,  20,  20,  15,  0,   -30,
    -30,  5,   10,  15,  15,  10,  5,   -30,
    -40, -20,  0,   5,   5,   0,   -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50
];

const BISHOP_PST: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,  0,   0,   0,   0,   0,   0,   -10,
    -10,  0,   5,   10,  10,  5,   0,   -10,
    -10,  5,   5,   10,  10,  5,   5,   -10,
    -10,  0,   10,  10,  10,  10,  0,   -10,
    -10,  10,  10,  10,  10,  10,  10,  -10,
    -10,  5,   0,   0,   0,   0,   5,   -10,
    -20, -10, -10, -10, -10, -10, -10, -20
];

const ROOK_PST: [i32; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,
    5,  10, 10, 10, 10, 10, 10, 5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    0,  0,  0,  5,  5,  0,  0,  0
];

const QUEEN_PST: [i32; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20,
    -10,  0,   0,   0,  0,   0,   0,   -10,
    -10,  0,   5,   5,  5,   5,   0,   -10,
    -5,   0,   5,   5,  5,   5,   0,   -5,
    0,    0,   5,   5,  5,   5,   0,   -5,
    -10,  5,   5,   5,  5,   5,   0,   -10,
    -10,  0,   5,   0,  0,   0,   0,   -10,
    -20, -10, -10, -5, -5, -10, -10, -20
];

const KING_PST: [i32; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -10, -20, -20, -20, -20, -20, -20, -10,
    20,  20,   0,   0,   0,   0,  20,  20,
    20,  30,  10,  0,   0,  10,  30,  20
];

// MATE_SCORE is used for mate detection
pub const MATE_SCORE: i32 = 100000;

// Material values
const PAWN_VALUE: i32 = 100;
const KNIGHT_VALUE: i32 = 320;
const BISHOP_VALUE: i32 = 330;
const ROOK_VALUE: i32 = 500;
const QUEEN_VALUE: i32 = 900;
const KING_VALUE: i32 = 20000;

// Helper functions
pub fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => PAWN_VALUE,
        Piece::Knight => KNIGHT_VALUE,
        Piece::Bishop => BISHOP_VALUE,
        Piece::Rook => ROOK_VALUE,
        Piece::Queen => QUEEN_VALUE,
        Piece::King => KING_VALUE,
    }
}

pub fn mvv_lva_score(attacker: Piece, victim: Piece) -> i32 {
    piece_value(victim) - piece_value(attacker) / 100
}

// Get piece-square table value for a piece at a square
fn pst_value(piece: Piece, sq: usize, color: usize) -> i32 {
    let table_sq = if color == 0 { sq } else { sq ^ 56 }; // Flip for black
    match piece {
        Piece::Pawn => PAWN_PST[table_sq],
        Piece::Knight => KNIGHT_PST[table_sq],
        Piece::Bishop => BISHOP_PST[table_sq],
        Piece::Rook => ROOK_PST[table_sq],
        Piece::Queen => QUEEN_PST[table_sq],
        Piece::King => KING_PST[table_sq],
    }
}

pub fn format_square(sq: Square) -> String {
    format!(
        "{}{}",
        (b'a' + sq.1 as u8) as char,
        (b'1' + sq.0 as u8) as char
    )
}

fn main() {
    let stdin = io::stdin();
    let _stdout = io::stdout();
    let mut board = Board::new();
    let mut tt_size_mb = 64; // Default TT size in MB
    let mut tt = TranspositionTable::new(tt_size_mb);

    println!("Chess Engine - Bitboard Version");
    println!("Type 'uci' to start UCI mode");

    for line in stdin.lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "uci" => {
                println!("id name ChessEngine Bitboard");
                println!("id author Dean Menezes");
                println!("option name Hash type spin default 64 min 1 max 1024");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "setoption" => {
                if parts.len() >= 4 && parts[1] == "name" && parts[2] == "Hash" && parts[3] == "value" {
                    if let Ok(size) = parts[4].parse::<usize>() {
                        if size >= 1 && size <= 1024 {
                            tt_size_mb = size;
                            tt = TranspositionTable::new(tt_size_mb);
                            println!("info string Hash set to {} MB", tt_size_mb);
                        }
                    }
                }
            }
            "ucinewgame" => {
                tt.clear();
                board = Board::new();
            }
            "position" => {
                if parts.len() > 1 {
                    if parts[1] == "startpos" {
                        board = Board::new();
                    } else if parts[1] == "fen" && parts.len() > 2 {
                        let fen = parts[2..].join(" ");
                        board = Board::from_fen(&fen);
                    }
                    // Handle moves
                    if let Some(move_idx) = parts.iter().position(|&x| x == "moves") {
                        for move_str in &parts[move_idx + 1..] {
                            if let Some(m) = parse_uci_move(&board, move_str) {
                                let _info = board.make_move(&m);
                            }
                        }
                    }
                }
            }
            "go" => {
                let mut wtime = 0;
                let mut btime = 0;
                let mut movestogo = 40; // Default for tournament time controls
                let mut movetime = 0; // Exact time for this move in ms
                let mut depth = 0; // Maximum search depth

                // Parse time controls
                let mut i = 1;
                while i < parts.len() {
                    match parts[i] {
                        "wtime" => {
                            if i + 1 < parts.len() {
                                wtime = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "btime" => {
                            if i + 1 < parts.len() {
                                btime = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "movestogo" => {
                            if i + 1 < parts.len() {
                                movestogo = parts[i + 1].parse().unwrap_or(40);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "movetime" => {
                            if i + 1 < parts.len() {
                                movetime = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        "depth" => {
                            if i + 1 < parts.len() {
                                depth = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                            } else {
                                i += 1;
                            }
                        }
                        _ => i += 1,
                    }
                }

                // Determine search mode and parameters
                let best_move = if depth > 0 {
                    // Fixed depth search
                    let (move_result, _score) = find_best_move_at_depth(&mut board, &mut tt, depth);
                    if let Some(_m) = move_result {
                        // Output info line with score and PV
                        let pv = extract_pv(&mut board, &mut tt, depth);
                        let pv_string = pv.iter().map(|mv| move_to_uci(mv)).collect::<Vec<String>>().join(" ");
                        println!("info depth {} score cp {} pv {}", depth, _score, pv_string);
                    }
                    move_result
                } else if movetime > 0 {
                    // Fixed time search
                    find_best_move_timed(&mut board, &mut tt, movetime)
                } else {
                    // Time-controlled search
                    let time_for_move = if board.white_to_move {
                        if wtime > 0 {
                            wtime / movestogo.max(1)
                        } else {
                            1000 // Default 1 second
                        }
                    } else {
                        if btime > 0 {
                            btime / movestogo.max(1)
                        } else {
                            1000 // Default 1 second
                        }
                    };
                    find_best_move_timed(&mut board, &mut tt, time_for_move)
                };

                if let Some(m) = best_move {
                    println!("bestmove {}", move_to_uci(&m));
                } else {
                    println!("bestmove 0000");
                }
            }
            "perft" => {
                if parts.len() > 1 {
                    if let Ok(depth) = parts[1].parse::<u32>() {
                        use std::time::Instant;
                        let start = Instant::now();
                        let nodes = crate::perft::perft(&mut board, depth);
                        let elapsed = start.elapsed();
                        println!("Nodes: {}", nodes);
                        println!("Time: {:.3}s", elapsed.as_secs_f64());
                        println!("NPS: {:.0}", nodes as f64 / elapsed.as_secs_f64());
                    }
                }
            }
            "perftdivide" => {
                if parts.len() > 1 {
                    if let Ok(depth) = parts[1].parse::<u32>() {
                        crate::perft::perft_divide(&mut board, depth);
                    }
                }
            }
            "quit" => break,
            "d" => {
                board.print();
            }
            _ => {}
        }
    }
}

fn parse_uci_move(board: &Board, move_str: &str) -> Option<Move> {
    if move_str.len() < 4 {
        return None;
    }

    let from_file = (move_str.as_bytes()[0] - b'a') as usize;
    let from_rank = (move_str.as_bytes()[1] - b'1') as usize;
    let to_file = (move_str.as_bytes()[2] - b'a') as usize;
    let to_rank = (move_str.as_bytes()[3] - b'1') as usize;

    let from = Square(from_rank, from_file);
    let to = Square(to_rank, to_file);

    let promotion = if move_str.len() == 5 {
        match move_str.as_bytes()[4] as char {
            'q' => Some(Piece::Queen),
            'r' => Some(Piece::Rook),
            'b' => Some(Piece::Bishop),
            'n' => Some(Piece::Knight),
            _ => None,
        }
    } else {
        None
    };

    Some(board.create_move(from, to, promotion, false, false))
}

fn move_to_uci(m: &Move) -> String {
    let from = format_square(m.from);
    let to = format_square(m.to);
    let mut result = format!("{}{}", from, to);
    if let Some(promo) = m.promotion {
        let promo_char = match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => 'q',
        };
        result.push(promo_char);
    }
    result
}

fn find_best_move_timed(board: &mut Board, tt: &mut TranspositionTable, time_limit_ms: u32) -> Option<Move> {
    let start_time = Instant::now();
    let mut best_move = None;
    let mut current_depth = 1;

    loop {
        let elapsed = start_time.elapsed().as_millis() as u32;
        if elapsed >= time_limit_ms {
            break;
        }

        let (move_result, score) = find_best_move_at_depth(board, tt, current_depth);
        if let Some(m) = move_result {
            best_move = Some(m);
            // Output info line with score and PV
            let pv = extract_pv(board, tt, current_depth);
            let pv_string = pv.iter().map(|mv| move_to_uci(mv)).collect::<Vec<String>>().join(" ");
            println!("info depth {} score cp {} pv {}", current_depth, score, pv_string);
        }

        current_depth += 1;

        // Safety check: don't go too deep
        if current_depth > 20 {
            break;
        }
    }

    best_move
}

fn find_best_move_at_depth(board: &mut Board, tt: &mut TranspositionTable, max_depth: u32) -> (Option<Move>, i32) {
    let mut best_score = -MATE_SCORE * 2;
    let mut best_move = None;

    let moves = board.generate_pseudo_moves();
    for m in moves {
        let info = board.make_move(&m);
        let score = -negamax(board, tt, max_depth - 1, -MATE_SCORE, MATE_SCORE);
        board.unmake_move(&m, info);

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }
    }

    (best_move, best_score)
}

fn extract_pv(_board: &Board, _tt: &TranspositionTable, _depth: u32) -> Vec<Move> {
    let mut pv = Vec::new();
    let temp_board = _board.clone();

    // For now, just return the best move from the root
    // In a full implementation, this would extract the PV from the TT
    let moves = temp_board.generate_pseudo_moves();
    if moves.is_empty() {
        return pv;
    }

    // Find the best move by evaluating each one at depth 1
    let mut best_move = None;
    let mut best_score = -MATE_SCORE * 2;

    for m in &moves {
        let mut temp_board_copy = temp_board.clone();
        let info = temp_board_copy.make_move(m);
        let score = -evaluate(&mut temp_board_copy);
        temp_board_copy.unmake_move(m, info);

        if score > best_score {
            best_score = score;
            best_move = Some(*m);
        }
    }

    if let Some(m) = best_move {
        pv.push(m);
    }

    pv
}

fn negamax(
    board: &mut Board,
    tt: &mut TranspositionTable,
    depth: u32,
    mut alpha: i32,
    mut beta: i32,
) -> i32 {
    let original_alpha = alpha;
    let current_hash = board.hash;

    // --- Transposition Table Probe ---
    let mut hash_move: Option<Move> = None;
    if let Some(entry) = tt.probe(current_hash) {
        if entry.depth >= depth {
            match entry.bound_type {
                BoundType::Exact => return entry.score,
                BoundType::LowerBound => alpha = alpha.max(entry.score),
                BoundType::UpperBound => beta = beta.min(entry.score),
            }
            if alpha >= beta {
                return entry.score;
            }
        }
        hash_move = entry.best_move.clone();
    }

    // --- Base Case: Depth 0 ---
    if depth == 0 {
        return quiescence(board, alpha, beta);
    }

    // --- Generate and Order Moves ---
    let mut moves = board.generate_pseudo_moves();

    // Sort moves: hash move first, then captures (MVV-LVA), then other moves
    if let Some(hm) = &hash_move {
        if let Some(pos) = moves.iter().position(|m| m == hm) {
            moves.swap(0, pos);
        }
    }

    moves.sort_by(|a, b| {
        let a_is_capture = a.captured_piece.is_some();
        let b_is_capture = b.captured_piece.is_some();

        if a_is_capture && !b_is_capture {
            return std::cmp::Ordering::Less;
        } else if !a_is_capture && b_is_capture {
            return std::cmp::Ordering::Greater;
        } else if a_is_capture && b_is_capture {
            let a_attacker = board.piece_at(a.from.0 * 8 + a.from.1).unwrap().1;
            let b_attacker = board.piece_at(b.from.0 * 8 + b.from.1).unwrap().1;
            let a_score = mvv_lva_score(a_attacker, a.captured_piece.unwrap());
            let b_score = mvv_lva_score(b_attacker, b.captured_piece.unwrap());
            return b_score.cmp(&a_score);
        }
        std::cmp::Ordering::Equal
    });

    // --- Check for Checkmate / Stalemate ---
    if moves.is_empty() {
        let current_color = if board.white_to_move { Color::White } else { Color::Black };
        return if board.is_in_check(current_color) {
            -(MATE_SCORE - (100 - depth as i32))
        } else {
            0
        };
    }

    // --- PVS Search ---
    let mut best_score = -MATE_SCORE * 2;
    let mut best_move_found: Option<Move> = None;

    for (i, m) in moves.iter().enumerate() {
        let info = board.make_move(m);
        let score = if i == 0 {
            // First move (PV move): full window search
            -negamax(board, tt, depth - 1, -beta, -alpha)
        } else {
            // Non-PV moves: null window search
            let mut score = -negamax(board, tt, depth - 1, -alpha - 1, -alpha);
            if score > alpha && score < beta {
                // Research with full window if null window failed
                score = -negamax(board, tt, depth - 1, -beta, -alpha);
            }
            score
        };
        board.unmake_move(m, info);

        if score > best_score {
            best_score = score;
            best_move_found = Some(m.clone());
        }

        alpha = alpha.max(best_score);

        if alpha >= beta {
            break;
        }
    }

    // --- Transposition Table Store ---
    let bound_type = if best_score <= original_alpha {
        BoundType::UpperBound
    } else if best_score >= beta {
        BoundType::LowerBound
    } else {
        BoundType::Exact
    };

    tt.store(current_hash, depth, best_score, bound_type, best_move_found);

    best_score
}

fn evaluate(board: &mut Board) -> i32 {
    let mut score = 0;

    // Material values for middlegame and endgame (from d08a060)
    const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000]; // P, N, B, R, Q, K
    const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000]; // P, N, B, R, Q, K

    // Piece-square tables (middlegame) - from d08a060
    const PST_MG: [[i32; 64]; 6] = [
        // Pawn
        [
            0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3,
            3, 33, -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6,
            7, 26, 31, 65, 56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0,
            0,
        ],
        // Knight
        [
            -105, -21, -58, -33, -17, -28, -19, -23, -29, -53, -12, -3, -1, 18, -14, -19, -23,
            -9, 12, 10, 19, 17, 25, -16, -13, 4, 16, 13, 28, 19, 21, -8, -9, 17, 19, 53, 37,
            69, 18, 22, -47, 60, 37, 65, 84, 129, 73, 44, -73, -41, 72, 36, 23, 62, 7, -17,
            -167, -89, -34, -49, 61, -97, -15, -107,
        ],
        // Bishop
        [
            -33, -3, -14, -21, -13, -12, -39, -21, 4, 15, 16, 0, 7, 21, 33, 1, 0, 15, 15, 15,
            14, 27, 18, 10, -6, 13, 13, 26, 34, 12, 10, 4, -4, 5, 19, 50, 37, 37, 7, -2, -16,
            37, 43, 40, 35, 50, 37, -2, -26, 16, -18, -13, 30, 59, 18, -47, -29, 4, -82, -37,
            -25, -42, 7, -8,
        ],
        // Rook
        [
            -19, -13, 1, 17, 16, 7, -37, -26, -44, -16, -20, -9, -1, 11, -6, -71, -45, -25,
            -16, -17, 3, 0, -5, -33, -36, -26, -12, -1, 9, -7, 6, -23, -24, -11, 7, 26, 24, 35,
            -8, -20, -5, 19, 26, 36, 17, 45, 61, 16, 27, 32, 58, 62, 80, 67, 26, 44, 32, 42,
            32, 51, 63, 9, 31, 43,
        ],
        // Queen
        [
            -1, -18, -9, 10, -15, -25, -31, -50, -35, -8, 11, 2, 8, 15, -3, 1, -14, 2, -11, -2,
            -5, 2, 14, 5, -9, -26, -9, -10, -2, -4, 3, -3, -27, -27, -16, -16, -1, 17, -2, 1,
            -13, -17, 7, 8, 29, 56, 47, 57, -24, -39, -5, 1, -16, 57, 28, 54, -28, 0, 29, 12,
            59, 44, 43, 45,
        ],
        // King
        [
            -15, 36, 12, -54, 8, -28, 34, 14, 1, 7, -8, -64, -43, -16, 9, 8, -14, -14, -22,
            -46, -44, -30, -15, -27, -49, -1, -27, -39, -46, -44, -33, -51, -17, -20, -12, -27,
            -30, -25, -14, -36, -9, 24, 2, -16, -20, 6, 22, -22, 29, -1, -20, -7, -8, -4, -38,
            -29, -65, 23, 16, -15, -56, -34, 2, 13,
        ],
    ];

    // Piece-square tables (endgame) - from d08a060
    const PST_EG: [[i32; 64]; 6] = [
        // Pawn
        [
            0, 0, 0, 0, 0, 0, 0, 0, 13, 8, 8, 10, 13, 0, 2, -7, 4, 7, -6, 1, 0, -5, -1, -8, 13,
            9, -3, -7, -7, -8, 3, -1, 32, 24, 13, 5, -2, 4, 17, 17, 94, 100, 85, 67, 56, 53,
            82, 84, 178, 173, 158, 134, 147, 132, 165, 187, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        // Knight
        [
            -29, -51, -23, -15, -22, -18, -50, -64, -42, -20, -10, -5, -2, -20, -23, -44, -23,
            -3, -1, 15, 10, -3, -20, -22, -18, -6, 16, 25, 16, 17, 4, -18, -17, 3, 22, 22, 22,
            11, 8, -18, -24, -20, 10, 9, -1, -9, -19, -41, -25, -8, -25, -2, -9, -25, -24, -52,
            -58, -38, -13, -28, -31, -27, -63, -99,
        ],
        // Bishop
        [
            -23, -9, -23, -5, -9, -16, -5, -17, -14, -18, -7, -1, 4, -9, -15, -27, -12, -3, 8,
            10, 13, 3, -7, -15, -6, 3, 13, 19, 7, 10, -3, -9, -3, 9, 12, 9, 14, 10, 3, 2, 2,
            -8, 0, -1, -2, 6, 0, 4, -8, -4, 7, -12, -3, -13, -4, -14, -14, -21, -11, -8, -7,
            -9, -17, -24,
        ],
        // Rook
        [
            -9, 2, 3, -1, -5, -13, 4, -20, -6, -6, 0, 2, -9, -9, -11, -3, -4, 0, -5, -1, -7,
            -12, -8, -16, 3, 5, 8, 4, -5, -6, -8, -11, 4, 3, 13, 1, 2, 1, -1, 2, 7, 7, 7, 5, 4,
            -3, -5, -3, 11, 13, 13, 11, -3, 3, 8, 3, 13, 10, 18, 15, 12, 12, 8, 5,
        ],
        // Queen
        [
            -33, -28, -22, -43, -5, -32, -20, -41, -22, -23, -30, -16, -16, -23, -36, -32, -16,
            -27, 15, 6, 9, 17, 10, 5, -18, 28, 19, 47, 31, 34, 39, 23, 3, 22, 24, 45, 57, 40,
            57, 36, -20, 6, 9, 49, 47, 35, 19, 9, -17, 20, 32, 41, 58, 25, 30, 0, -9, 22, 22,
            27, 27, 19, 10, 20,
        ],
        // King
        [
            -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, -5, -17, -19, -3,
            11, 21, 23, 16, 7, -9, -18, -4, 21, 24, 27, 23, 9, -11, -8, 22, 24, 27, 26, 33, 26,
            3, 10, 17, 23, 15, 20, 45, 44, 13, -12, 17, 14, 17, 17, 38, 23, 11, -74, -35, -18,
            -18, -11, 15, 4, -17,
        ],
    ];

    // Count pieces for game phase detection and evaluation features
    let mut white_material_mg = 0;
    let mut black_material_mg = 0;
    let mut white_material_eg = 0;
    let mut black_material_eg = 0;
    let mut white_bishop_count = 0;
    let mut black_bishop_count = 0;
    let mut white_pawns_by_file = [0; 8];
    let mut black_pawns_by_file = [0; 8];
    let mut white_king_pos = (0, 0);
    let mut black_king_pos = (0, 0);

    // First pass: Count pieces and positions using bitboards
    for color_idx in 0..2 {
        for piece_idx in 0..6 {
            let piece_bb = board.pieces[color_idx][piece_idx];
            let piece = match piece_idx {
                0 => Piece::Pawn,
                1 => Piece::Knight,
                2 => Piece::Bishop,
                3 => Piece::Rook,
                4 => Piece::Queen,
                5 => Piece::King,
                _ => continue,
            };

            let mut bb = piece_bb;
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                let rank = sq / 8;
                let file = sq % 8;
                bb &= bb - 1;

                if color_idx == 0 {
                    if piece == Piece::Bishop {
                        white_bishop_count += 1;
                    } else if piece == Piece::King {
                        white_king_pos = (rank, file);
                    } else if piece == Piece::Pawn {
                        white_pawns_by_file[file] += 1;
                    }

                    white_material_mg += MATERIAL_MG[piece_idx];
                    white_material_eg += MATERIAL_EG[piece_idx];
                } else {
                    if piece == Piece::Bishop {
                        black_bishop_count += 1;
                    } else if piece == Piece::King {
                        black_king_pos = (rank, file);
                    } else if piece == Piece::Pawn {
                        black_pawns_by_file[file] += 1;
                    }

                    black_material_mg += MATERIAL_MG[piece_idx];
                    black_material_eg += MATERIAL_EG[piece_idx];
                }
            }
        }
    }

    // Calculate game phase based on remaining material
    let total_material_mg = white_material_mg + black_material_mg;
    let max_material = 2 * (MATERIAL_MG[1] * 2 + MATERIAL_MG[2] * 2 + MATERIAL_MG[3] * 2 + MATERIAL_MG[4] + MATERIAL_MG[0] * 8);
    let phase = (total_material_mg as f32) / (max_material as f32);
    let phase = phase.min(1.0).max(0.0);

    // Second pass: Evaluate pieces with position
    let mut mg_score = 0;
    let mut eg_score = 0;

    for color_idx in 0..2 {
        for piece_idx in 0..6 {
            let piece_bb = board.pieces[color_idx][piece_idx];
            let mut bb = piece_bb;
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;

                // Get 1D index for piece square tables
                let sq_idx = if color_idx == 0 {
                    sq ^ 56 // White pieces are flipped vertically (7-rank * 8 + file)
                } else {
                    sq // Black pieces use the table as-is
                };

                let mg_value = MATERIAL_MG[piece_idx] + PST_MG[piece_idx][sq_idx];
                let eg_value = MATERIAL_EG[piece_idx] + PST_EG[piece_idx][sq_idx];

                if color_idx == 0 {
                    mg_score += mg_value;
                    eg_score += eg_value;
                } else {
                    mg_score -= mg_value;
                    eg_score -= eg_value;
                }
            }
        }
    }

    // Interpolate between middlegame and endgame scores based on phase
    let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
    score += position_score;

    // Additional evaluation factors

    // 1. Bishop pair bonus
    if white_bishop_count >= 2 {
        score += 30;
    }
    if black_bishop_count >= 2 {
        score -= 30;
    }

    // 2. Rook on open files
    for file in 0..8 {
        let white_rooks_on_file = (board.pieces[0][3] & bitboard::file_mask(file)) != 0;
        let black_rooks_on_file = (board.pieces[1][3] & bitboard::file_mask(file)) != 0;

        if white_rooks_on_file || black_rooks_on_file {
            let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];

            if file_pawns == 0 {
                // Open file
                let bonus = 15;
                if white_rooks_on_file {
                    score += bonus;
                }
                if black_rooks_on_file {
                    score -= bonus;
                }
            } else if (white_rooks_on_file && black_pawns_by_file[file] == 0)
                || (black_rooks_on_file && white_pawns_by_file[file] == 0) {
                // Semi-open file
                let bonus = 7;
                if white_rooks_on_file {
                    score += bonus;
                }
                if black_rooks_on_file {
                    score -= bonus;
                }
            }
        }
    }

    // 3. Pawn structure
    for file in 0..8 {
        // Isolated pawns
        if white_pawns_by_file[file] > 0 {
            let left_file = if file > 0 { white_pawns_by_file[file - 1] } else { 0 };
            let right_file = if file < 7 { white_pawns_by_file[file + 1] } else { 0 };

            if left_file == 0 && right_file == 0 {
                score -= 12; // Isolated pawn penalty
            }
        }

        if black_pawns_by_file[file] > 0 {
            let left_file = if file > 0 { black_pawns_by_file[file - 1] } else { 0 };
            let right_file = if file < 7 { black_pawns_by_file[file + 1] } else { 0 };

            if left_file == 0 && right_file == 0 {
                score += 12; // Isolated pawn penalty for black
            }
        }

        // Doubled pawns penalty
        if white_pawns_by_file[file] > 1 {
            score -= 12 * (white_pawns_by_file[file] - 1);
        }
        if black_pawns_by_file[file] > 1 {
            score += 12 * (black_pawns_by_file[file] - 1);
        }

        // Passed pawns (simplified version)
        // This is a simplified implementation - the original had more sophisticated passed pawn detection
        for rank in 0..8 {
            let sq = rank * 8 + file;
            let white_pawn_here = (board.pieces[0][0] & (1u64 << sq)) != 0;
            let black_pawn_here = (board.pieces[1][0] & (1u64 << sq)) != 0;

            if white_pawn_here {
                let mut is_passed = true;
                // Check if there are any black pawns ahead on same or adjacent files
                for check_rank in 0..rank {
                    for df in -1i32..=1 {
                        let check_file = file as i32 + df;
                        if check_file >= 0 && check_file < 8 {
                            let check_sq = check_rank * 8 + check_file as usize;
                            if (board.pieces[1][0] & (1u64 << check_sq)) != 0 {
                                is_passed = false;
                                break;
                            }
                        }
                    }
                    if !is_passed { break; }
                }

                if is_passed {
                    let bonus = 10 + (7 - rank as i32) * 7;
                    score += bonus;
                }
            }

            if black_pawn_here {
                let mut is_passed = true;
                // Check if there are any white pawns ahead on same or adjacent files
                for check_rank in (rank + 1)..8 {
                    for df in -1i32..=1 {
                        let check_file = file as i32 + df;
                        if check_file >= 0 && check_file < 8 {
                            let check_sq = check_rank * 8 + check_file as usize;
                            if (board.pieces[0][0] & (1u64 << check_sq)) != 0 {
                                is_passed = false;
                                break;
                            }
                        }
                    }
                    if !is_passed { break; }
                }

                if is_passed {
                    let bonus = 10 + rank as i32 * 7;
                    score -= bonus;
                }
            }
        }
    }

    // Return score relative to the current player to move
    if board.white_to_move {
        score
    } else {
        -score
    }
}

fn quiescence(board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
    let stand_pat = evaluate(board);

    // Stand pat: if current position is good enough, return it
    if stand_pat >= beta {
        return beta;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    let moves = board.generate_moves();
    let mut captures = Vec::new();

    // Collect only captures and promotions
    for m in moves {
        if m.captured_piece.is_some() || m.promotion.is_some() {
            captures.push(m);
        }
    }

    // Sort captures by MVV-LVA for better ordering
    captures.sort_by(|a, b| {
        let a_score = if let Some(victim) = a.captured_piece {
            if let Some(attacker) = board.piece_at(a.from.0 * 8 + a.from.1) {
                mvv_lva_score(attacker.1, victim)
            } else {
                0
            }
        } else {
            0
        };

        let b_score = if let Some(victim) = b.captured_piece {
            if let Some(attacker) = board.piece_at(b.from.0 * 8 + b.from.1) {
                mvv_lva_score(attacker.1, victim)
            } else {
                0
            }
        } else {
            0
        };

        b_score.cmp(&a_score) // Sort descending
    });

    for m in captures {
        let info = board.make_move(&m);
        let score = -quiescence(board, -beta, -alpha);
        board.unmake_move(&m, info);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
