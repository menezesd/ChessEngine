use crate::{Board, Move, TranspositionTable, MATE_SCORE};
use crate::search::SearchHeuristics;
use std::time::Instant;

pub struct AspirationSearch;

impl AspirationSearch {
    /// Search with progressive aspiration windows
    pub fn search_with_aspiration(
        board: &mut Board,
        tt: &mut TranspositionTable,
        depth: u32,
        last_score: i32,
        heur: &mut SearchHeuristics,
        time_limit: Option<Instant>
    ) -> i32 {
        // Use aspiration windows only for deeper searches
        if depth <= 6 || last_score.abs() > MATE_SCORE - 1000 {
            return board.negamax(tt, depth, -MATE_SCORE, MATE_SCORE, heur, 0);
        }
        
        // Start with a small window around the previous score
        let mut window_size = 25;
        let max_window = 400;
        
        loop {
            let alpha = last_score - window_size;
            let beta = last_score + window_size;
            
            let score = board.negamax(tt, depth, alpha, beta, heur, 0);
            
            // Check for timeout
            if let Some(deadline) = time_limit {
                if heur.check_abort() || Instant::now() >= deadline {
                    return score;
                }
            }
            
            // Score is within the window - we're done
            if score > alpha && score < beta {
                return score;
            }
            
            // If we found a mate score, verify with full window
            if score.abs() > MATE_SCORE - 1000 {
                return board.negamax(tt, depth, -MATE_SCORE, MATE_SCORE, heur, 0);
            }
            
            // Widen the window and try again
            window_size = (window_size * 2).min(max_window);
            
            // If window gets too large, just do a full search
            if window_size >= max_window {
                return board.negamax(tt, depth, -MATE_SCORE, MATE_SCORE, heur, 0);
            }
        }
    }
    
    /// Iterative deepening with aspiration windows
    pub fn iterative_deepening(
        board: &mut Board,
        tt: &mut TranspositionTable,
        max_depth: u32,
        time_limit: Option<Instant>,
        mut heur: &mut SearchHeuristics
    ) -> (Move, i32, u32) {
        let mut best_move = Move {
            from: crate::Square(0, 0),
            to: crate::Square(0, 0),
            is_castling: false,
            is_en_passant: false,
            promotion: None,
            captured_piece: None,
        };
        
        let mut last_score = 0;
        let mut completed_depth = 0;
        
        for depth in 1..=max_depth {
            // Check if we should start this iteration
            if let Some(deadline) = time_limit {
                if Self::should_stop_iteration(deadline, depth) {
                    break;
                }
            }
            
            let score = Self::search_with_aspiration(
                board, tt, depth, last_score, &mut heur, time_limit
            );
            
            // If search was aborted, don't update results
            if heur.abort {
                break;
            }
            
            // Extract best move from PV
            let pv = board.extract_pv(tt, 1);
            if !pv.is_empty() {
                best_move = pv[0];
            }
            
            last_score = score;
            completed_depth = depth;
            
            // Print search info (similar to UCI format)
            if depth > 1 {
                let time_ms = time_limit.map_or(0, |deadline| {
                    deadline.duration_since(Instant::now()).as_millis() as u64
                });
                
                println!("info depth {} score cp {} nodes {} time {} pv {}",
                    depth,
                    score,
                    heur.node_count,
                    time_ms,
                    Self::format_pv(&pv)
                );
            }
            
            // Stop if we found a mate
            if score.abs() > MATE_SCORE - 1000 {
                let mate_in = (MATE_SCORE - score.abs()) / 2 + 1;
                if depth >= mate_in as u32 * 3 / 2 {
                    break;
                }
            }
        }
        
        (best_move, last_score, completed_depth)
    }
    
    fn should_stop_iteration(deadline: Instant, depth: u32) -> bool {
        let now = Instant::now();
        if now >= deadline {
            return true;
        }
        
        // Don't start new iteration if we have less than 1/3 of time remaining
        // and we've completed at least depth 4
        if depth >= 4 {
            let remaining = deadline.duration_since(now).as_millis();
            remaining < 100 // Less than 100ms remaining
        } else {
            false
        }
    }
    
    fn format_pv(pv: &[Move]) -> String {
        pv.iter()
            .take(10) // Limit PV length
            .map(|m| format!("{}{}", 
                Self::square_to_string(m.from), 
                Self::square_to_string(m.to)
            ))
            .collect::<Vec<_>>()
            .join(" ")
    }
    
    fn square_to_string(sq: crate::Square) -> String {
        let file = (b'a' + sq.1 as u8) as char;
        let rank = (b'1' + sq.0 as u8) as char;
        format!("{}{}", file, rank)
    }
}

/// Principal Variation collector for search analysis
pub struct PrincipalVariation {
    pub line: [[Move; 64]; 64],
    pub length: [usize; 64],
}

impl PrincipalVariation {
    pub fn new() -> Self {
        PrincipalVariation {
            line: [[Move {
                from: crate::Square(0, 0),
                to: crate::Square(0, 0),
                is_castling: false,
                is_en_passant: false,
                promotion: None,
                captured_piece: None,
            }; 64]; 64],
            length: [0; 64],
        }
    }
    
    pub fn update(&mut self, ply: usize, mv: Move) {
        if ply >= 64 { return; }
        
        self.line[ply][ply] = mv;
        
        // Copy the rest of the line from the next ply
        if ply + 1 < 64 {
            for i in (ply + 1)..self.length[ply + 1] {
                if i < 64 {
                    self.line[ply][i] = self.line[ply + 1][i];
                }
            }
            self.length[ply] = self.length[ply + 1];
        } else {
            self.length[ply] = ply + 1;
        }
    }
    
    pub fn get_best_move(&self) -> Option<Move> {
        if self.length[0] > 0 {
            Some(self.line[0][0])
        } else {
            None
        }
    }
    
    pub fn get_pv(&self, max_length: usize) -> Vec<Move> {
        let len = self.length[0].min(max_length);
        self.line[0][..len].to_vec()
    }
    
    pub fn clear(&mut self) {
        for i in 0..64 {
            self.length[i] = 0;
        }
    }
}