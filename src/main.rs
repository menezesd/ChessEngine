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
    beta: i32,
) -> i32 {
    if depth == 0 {
        return quiescence(board, alpha, beta);
    }

    let mut best_score = -MATE_SCORE;

    let mut moves = board.generate_pseudo_moves();

    // Sort moves: captures first (MVV-LVA), then other moves
    moves.sort_by(|a, b| {
        let a_is_capture = a.captured_piece.is_some();
        let b_is_capture = b.captured_piece.is_some();

        if a_is_capture && !b_is_capture {
            return std::cmp::Ordering::Less; // a comes first
        } else if !a_is_capture && b_is_capture {
            return std::cmp::Ordering::Greater; // b comes first
        } else if a_is_capture && b_is_capture {
            // Both are captures, sort by MVV-LVA
            let a_attacker = board.piece_at(a.from.0 * 8 + a.from.1).unwrap().1;
            let b_attacker = board.piece_at(b.from.0 * 8 + b.from.1).unwrap().1;
            let a_score = mvv_lva_score(a_attacker, a.captured_piece.unwrap());
            let b_score = mvv_lva_score(b_attacker, b.captured_piece.unwrap());
            return b_score.cmp(&a_score); // Higher MVV-LVA first
        }
        std::cmp::Ordering::Equal
    });

    for m in moves {
        let info = board.make_move(&m);
        let score = -negamax(board, tt, depth - 1, -beta, -alpha);
        board.unmake_move(&m, info);

        if score > best_score {
            best_score = score;
        }

        if score > alpha {
            alpha = score;
        }

        if alpha >= beta {
            break;
        }
    }

    best_score
}

fn evaluate(board: &mut Board) -> i32 {
    let mut score = 0;

    // Material and positional evaluation
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

            // Count pieces and add material + positional value
            let mut bb = piece_bb;
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1; // Clear least significant bit

                let material_value = piece_value(piece);
                let positional_value = pst_value(piece, sq, color_idx);

                if color_idx == 0 {
                    score += material_value + positional_value;
                } else {
                    score -= material_value + positional_value;
                }
            }
        }
    }

    // Mobility evaluation (simplified - just count legal moves)
    let white_mobility = board.generate_moves().len() as i32;
    let black_mobility = {
        // Temporarily flip the board to count black's moves
        let mut temp_board = board.clone();
        temp_board.white_to_move = !temp_board.white_to_move;
        temp_board.generate_moves().len() as i32
    };

    score += (white_mobility - black_mobility) * 2; // Small bonus for mobility

    score
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
