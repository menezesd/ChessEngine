use crate::types::{Color, Move, Square, Piece, TranspositionTable, MATE_SCORE, file_to_index, rank_to_index};
use crate::board::Board;
use std::time::{Duration, Instant};

/// Get the value of a piece for MVV-LVA and evaluation
pub fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000,
    }
}

/// Calculate MVV-LVA (Most Valuable Victim - Least Valuable Attacker) score
pub fn mvv_lva_score(mv: &Move, board: &Board) -> i32 {
    if let Some(victim) = mv.captured_piece {
        let attacker = board.squares[mv.from.0][mv.from.1].unwrap().1;
        let victim_val = piece_value(victim);
        let attacker_val = piece_value(attacker);
        victim_val * 10 - attacker_val
    } else {
        0
    }
}

/// Format a square as UCI string (e.g., "e2")
pub fn format_square(sq: Square) -> String {
    format!("{}{}", (sq.1 as u8 + b'a') as char, sq.0 + 1)
}

/// Format a move as UCI string (e.g., "e2e4", "e7e8q")
pub fn format_uci_move(mv: &Move) -> String {
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

/// Parse a UCI move string (e.g., "e2e4", "e7e8q") into a Move
pub fn parse_uci_move(board: &mut Board, uci_string: &str) -> Option<Move> {
    if uci_string.len() < 4 || uci_string.len() > 5 {
        return None;
    }

    let from_chars: Vec<char> = uci_string.chars().take(2).collect();
    let to_chars: Vec<char> = uci_string.chars().skip(2).take(2).collect();

    if from_chars.len() != 2 || to_chars.len() != 2 {
        return None;
    }

    if !('a'..='h').contains(&from_chars[0])
        || !('1'..='8').contains(&from_chars[1])
        || !('a'..='h').contains(&to_chars[0])
        || !('1'..='8').contains(&to_chars[1])
    {
        return None;
    }

    let from_file = file_to_index(from_chars[0]);
    let from_rank = rank_to_index(from_chars[1]);
    let to_file = file_to_index(to_chars[0]);
    let to_rank = rank_to_index(to_chars[1]);

    let from_sq = Square(from_rank, from_file);
    let to_sq = Square(to_rank, to_file);

    let promotion_piece = if uci_string.len() == 5 {
        match uci_string.chars().nth(4) {
            Some('q') => Some(Piece::Queen),
            Some('r') => Some(Piece::Rook),
            Some('b') => Some(Piece::Bishop),
            Some('n') => Some(Piece::Knight),
            _ => return None,
        }
    } else {
        None
    };

    let legal_moves = board.generate_moves();

    for legal_move in legal_moves {
        if legal_move.from == from_sq && legal_move.to == to_sq {
            if legal_move.promotion == promotion_piece {
                return Some(legal_move.clone());
            } else if promotion_piece.is_none() && legal_move.promotion.is_none() {
                return Some(legal_move.clone());
            }
        }
    }

    None
}

/// Parse a "position" UCI command
pub fn parse_position_command(board: &mut Board, parts: &[&str]) {
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

/// Find the best move for the current position using iterative deepening
pub fn find_best_move(board: &mut Board, tt: &mut TranspositionTable, max_depth: u32) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut best_score = -MATE_SCORE * 2;

    let legal_moves = board.generate_moves();
    if legal_moves.is_empty() {
        return None;
    }
    if legal_moves.len() == 1 {
        return Some(legal_moves[0]);
    }
    let mut root_moves = legal_moves.clone();

    for depth in 1..=max_depth {
        let mut alpha = -MATE_SCORE * 2;
        let beta = MATE_SCORE * 2;
        let mut current_best_score = -MATE_SCORE * 2;
        let mut current_best_move: Option<Move> = None;

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

            if let Some(pos) = root_moves.iter().position(|m| *m == mv) {
                root_moves.swap(0, pos);
            }
        }
    }

    best_move
}

/// Find the best move with a time limit
pub fn find_best_move_with_time(
    board: &mut Board,
    tt: &mut TranspositionTable,
    max_time: Duration,
    start_time: Instant,
) -> Option<Move> {
    let mut best_move: Option<Move> = None;
    let mut depth = 1;
    let mut last_depth_time = Duration::from_millis(1);

    const SAFETY_MARGIN: Duration = Duration::from_millis(5);
    const TIME_GROWTH_FACTOR: f32 = 2.0;

    while start_time.elapsed() + SAFETY_MARGIN < max_time {
        let elapsed = start_time.elapsed();
        let time_remaining = max_time.checked_sub(elapsed).unwrap_or_default();

        let estimated_next_time = last_depth_time.mul_f32(TIME_GROWTH_FACTOR);
        if estimated_next_time + SAFETY_MARGIN > time_remaining {
            break;
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
            return Some(legal_moves[0]);
        }

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
