use crate::{Board, Move};
use crate::search::SearchHeuristics;
use crate::board::{color_to_zobrist_index, square_to_zobrist_index, mvv_lva_score};

// Enhanced move ordering scores
pub const TT_MOVE_SCORE: i32 = 2_000_000;
pub const WINNING_CAPTURE_BASE: i32 = 1_000_000;
pub const EQUAL_CAPTURE_BASE: i32 = 900_000;
pub const KILLER_1_SCORE: i32 = 800_000;
pub const KILLER_2_SCORE: i32 = 700_000;
pub const COUNTER_MOVE_SCORE: i32 = 600_000;
pub const LOSING_CAPTURE_BASE: i32 = -100_000;


pub fn order_moves(
    moves: &mut Vec<Move>,
    board: &Board,
    heur: &SearchHeuristics,
    tt_move: Option<&Move>,
    ply: usize
) {
    let color_idx = color_to_zobrist_index(board.current_color());
    let killers = if ply < heur.killers.len() { 
        &heur.killers[ply] 
    } else { 
        &[None, None] 
    };
    
    let last_move_opt = if ply > 0 && ply <= heur.last_moves.len() { 
        heur.last_moves.get(ply - 1).cloned().flatten() 
    } else { 
        None 
    };

    // Calculate scores for all moves
    let mut move_scores: Vec<(Move, i32)> = moves.iter().map(|m| {
        let mut score = 0i32;
        
        // 1. TT move gets highest priority
        if let Some(hash_move) = tt_move {
            if m == hash_move {
                return (*m, TT_MOVE_SCORE);
            }
        }
        
        // 2. Captures and promotions
        if m.captured_piece.is_some() || m.is_en_passant || m.promotion.is_some() {
            let see_value = board.see(m);
            let mvv_lva = mvv_lva_score(m, board);
            
            if see_value > 0 {
                score = WINNING_CAPTURE_BASE + mvv_lva + see_value;
            } else if see_value == 0 {
                score = EQUAL_CAPTURE_BASE + mvv_lva;
            } else {
                score = LOSING_CAPTURE_BASE + mvv_lva + see_value;
            }
        } else {
            // 3. Quiet moves
            
            // Killer moves
            if let Some(k1) = &killers[0] {
                if m == k1 {
                    score = KILLER_1_SCORE;
                    return (*m, score);
                }
            }
            if let Some(k2) = &killers[1] {
                if m == k2 {
                    score = KILLER_2_SCORE;
                    return (*m, score);
                }
            }
            
            // Counter moves
            if let Some(last_move) = last_move_opt {
                let last_from = square_to_zobrist_index(last_move.from) as usize;
                let last_to = square_to_zobrist_index(last_move.to) as usize;
                if let Some(counter) = heur.countermove[color_idx][last_from][last_to] {
                    if *m == counter {
                        score = COUNTER_MOVE_SCORE;
                        return (*m, score);
                    }
                }
            }
            
            // History heuristic
            let from_idx = square_to_zobrist_index(m.from) as usize;
            let to_idx = square_to_zobrist_index(m.to) as usize;
            let history_score = heur.get_history_score(color_idx, from_idx, to_idx);
            
            score = history_score;
            
            // Additional bonuses for potentially good moves
            if let Some((_, piece)) = board.squares[m.from.0][m.from.1] {
                match piece {
                    crate::Piece::Knight => {
                        // Knights to center squares
                        if is_center_square(m.to) {
                            score += 100;
                        }
                    },
                    crate::Piece::Bishop => {
                        // Long diagonal moves
                        let from_diag = diagonal_index(m.from);
                        let to_diag = diagonal_index(m.to);
                        if from_diag == to_diag && square_distance(m.from, m.to) > 2 {
                            score += 50;
                        }
                    },
                    crate::Piece::Queen => {
                        // Queen moves that increase mobility
                        score += 25;
                    },
                    _ => {}
                }
            }
        }
        
        (*m, score)
    }).collect();
    
    // Sort by score (descending)
    move_scores.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Extract sorted moves
    *moves = move_scores.into_iter().map(|(m, _)| m).collect();
}

pub fn order_tactical_moves(moves: &mut Vec<Move>, board: &Board) {
    // Score and sort tactical moves (captures, promotions, EP) for quiescence
    let mut scored: Vec<(Move, i32)> = moves
        .iter()
        .map(|m| (*m, tactical_move_score(m, board)))
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    *moves = scored.into_iter().map(|(m, _)| m).collect();
}

fn is_center_square(sq: crate::Square) -> bool {
    matches!((sq.0, sq.1), (3, 3) | (3, 4) | (4, 3) | (4, 4))
}

fn diagonal_index(sq: crate::Square) -> i32 {
    (sq.0 as i32) - (sq.1 as i32)
}

fn square_distance(sq1: crate::Square, sq2: crate::Square) -> usize {
    let rank_diff = (sq1.0 as i32 - sq2.0 as i32).abs() as usize;
    let file_diff = (sq1.1 as i32 - sq2.1 as i32).abs() as usize;
    rank_diff.max(file_diff)
}


fn tactical_move_score(m: &Move, board: &Board) -> i32 {
    let mut score = 0;
    
    // SEE value
    let see_value = board.see(m);
    score += see_value * 10;
    
    // MVV-LVA
    score += mvv_lva_score(m, board);
    
    // Promotion bonus
    if let Some(promo) = m.promotion {
        score += match promo {
            crate::Piece::Queen => 900,
            crate::Piece::Rook => 500,
            crate::Piece::Bishop | crate::Piece::Knight => 300,
            _ => 0,
        };
    }
    
    // Checks (if we can detect them cheaply)
    // TODO: Add check detection
    
    score
}