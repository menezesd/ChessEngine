use std::time::Instant;

use crate::board::Board;
use crate::evaluation::{evaluate, mvv_lva_score, quiescence, MATE_SCORE};
use crate::types::*;
use crate::utils::move_to_uci;

/// Find the best move within a time limit using iterative deepening
pub fn find_best_move_timed(board: &mut Board, tt: &mut TranspositionTable, time_limit_ms: u32) -> Option<Move> {
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

/// Find the best move at a specific depth
pub fn find_best_move_at_depth(board: &mut Board, tt: &mut TranspositionTable, max_depth: u32) -> (Option<Move>, i32) {
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

/// Extract principal variation from the transposition table
pub fn extract_pv(_board: &Board, _tt: &TranspositionTable, _depth: u32) -> Vec<Move> {
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

/// Negamax search with alpha-beta pruning and transposition table
pub fn negamax(
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
