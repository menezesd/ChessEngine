// Advanced chess engine search implementation
// Features: Negamax with alpha-beta, PVS, iterative deepening, aspiration windows,
// null move pruning, LMR, extensions, transposition tables, and advanced move ordering

use crate::{Board, Move, Square, TranspositionTable, MATE_SCORE, Piece};
use crate::board::Color;
use crate::tt::BoundType;
use std::time::Instant;

// Search constants
const PLY_LIMIT: usize = 64;
const SINGULAR_DEPTH: u32 = 7;
const EVAL_LIMIT: i32 = MATE_SCORE - 1000;

// Stack entry for search context
#[derive(Clone, Copy, Debug)]
pub struct Stack {
    pub previous_capture_to: i8,  // target square of previous capture, or -1
    pub previous_eval: i32,       // eval for this ply to detect improving
}

impl Default for Stack {
    fn default() -> Self {
        Stack {
            previous_capture_to: -1,
            previous_eval: 0,
        }
    }
}

// Search context containing stack and excluded move
#[derive(Clone)]
pub struct SearchContext {
    pub stack: [Stack; PLY_LIMIT],
    pub excluded_move: Option<Move>,
}

impl SearchContext {
    pub fn new() -> Self {
        SearchContext {
            stack: [Stack::default(); PLY_LIMIT],
            excluded_move: None,
        }
    }
    
    pub fn clear(&mut self) {
        for i in 0..PLY_LIMIT {
            self.stack[i] = Stack::default();
        }
        self.excluded_move = None;
    }
}

// History data for move ordering
#[derive(Clone)]
pub struct HistoryData {
    killer1: [Option<Move>; PLY_LIMIT],
    killer2: [Option<Move>; PLY_LIMIT],
    cutoff_history: [[[i32; 64]; 64]; 2],  // [color][from][to]
    tries_history: [[[i32; 64]; 64]; 2],   // total attempts for normalization
    last_capture_square: [i8; PLY_LIMIT],
}

impl HistoryData {
    pub fn new() -> Self {
        HistoryData {
            killer1: [None; PLY_LIMIT],
            killer2: [None; PLY_LIMIT],
            cutoff_history: [[[0; 64]; 64]; 2],
            tries_history: [[[0; 64]; 64]; 2],
            last_capture_square: [-1; PLY_LIMIT],
        }
    }
    
    pub fn clear(&mut self) {
        self.killer1 = [None; PLY_LIMIT];
        self.killer2 = [None; PLY_LIMIT];
        self.cutoff_history = [[[0; 64]; 64]; 2];
        self.tries_history = [[[0; 64]; 64]; 2];
        self.last_capture_square = [-1; PLY_LIMIT];
    }
    
    pub fn trim(&mut self) {
        // Halve history values when they grow too high
        for color in 0..2 {
            for from in 0..64 {
                for to in 0..64 {
                    self.cutoff_history[color][from][to] /= 2;
                    self.tries_history[color][from][to] /= 2;
                }
            }
        }
    }
    
    pub fn update(&mut self, board: &Board, mv: Move, depth: u32, ply: usize) {
        let color_idx = if board.current_color() == Color::White { 0 } else { 1 };
        let from_idx = self.square_to_index(mv.from);
        let to_idx = self.square_to_index(mv.to);
        
        let bonus = self.inc(depth);
        self.cutoff_history[color_idx][from_idx][to_idx] += bonus;
        
        // Update killers
        if ply < PLY_LIMIT {
            if self.killer1[ply] != Some(mv) {
                self.killer2[ply] = self.killer1[ply];
                self.killer1[ply] = Some(mv);
            }
        }
    }
    
    pub fn update_tries(&mut self, board: &Board, mv: Move, depth: u32) {
        let color_idx = if board.current_color() == Color::White { 0 } else { 1 };
        let from_idx = self.square_to_index(mv.from);
        let to_idx = self.square_to_index(mv.to);
        
        let inc = self.inc(depth);
        self.tries_history[color_idx][from_idx][to_idx] += inc;
    }
    
    pub fn get_score(&self, board: &Board, mv: Move) -> i32 {
        let color_idx = if board.current_color() == Color::White { 0 } else { 1 };
        let from_idx = self.square_to_index(mv.from);
        let to_idx = self.square_to_index(mv.to);
        
        let tries = self.tries_history[color_idx][from_idx][to_idx];
        if tries > 0 {
            (self.cutoff_history[color_idx][from_idx][to_idx] * 10000) / tries
        } else {
            0
        }
    }
    
    pub fn get_killer1(&self, ply: usize) -> Option<Move> {
        if ply < PLY_LIMIT {
            self.killer1[ply]
        } else {
            None
        }
    }
    
    pub fn get_killer2(&self, ply: usize) -> Option<Move> {
        if ply < PLY_LIMIT {
            self.killer2[ply]
        } else {
            None
        }
    }
    
    pub fn is_killer(&self, mv: Move, ply: usize) -> bool {
        if ply < PLY_LIMIT {
            self.killer1[ply] == Some(mv) || self.killer2[ply] == Some(mv)
        } else {
            false
        }
    }
    
    fn inc(&self, depth: u32) -> i32 {
        depth as i32 * depth as i32
    }
    
    fn square_to_index(&self, sq: Square) -> usize {
        (sq.0 * 8 + sq.1) as usize
    }
}

// Principal Variation collector
#[derive(Clone)]
pub struct PvCollector {
    pub line: [[Move; PLY_LIMIT + 2]; PLY_LIMIT + 2],
    pub size: [usize; PLY_LIMIT + 2],
}

impl PvCollector {
    pub fn new() -> Self {
        let dummy_move = Move {
            from: Square(0, 0),
            to: Square(0, 0),
            is_castling: false,
            is_en_passant: false,
            promotion: None,
            captured_piece: None,
        };
        
        PvCollector {
            line: [[dummy_move; PLY_LIMIT + 2]; PLY_LIMIT + 2],
            size: [0; PLY_LIMIT + 2],
        }
    }
    
    pub fn clear(&mut self) {
        for i in 0..PLY_LIMIT + 2 {
            self.size[i] = 0;
        }
    }
    
    pub fn update(&mut self, ply: usize, mv: Move) {
        if ply >= PLY_LIMIT {
            return;
        }
        
        self.line[ply][ply] = mv;
        
        // Copy the continuation from the next ply
        if ply + 1 < PLY_LIMIT {
            let next_size = self.size[ply + 1];
            for i in (ply + 1)..next_size.min(PLY_LIMIT) {
                self.line[ply][i] = self.line[ply + 1][i];
            }
            self.size[ply] = next_size;
        } else {
            self.size[ply] = ply + 1;
        }
    }
    
    pub fn get_best_move(&self) -> Option<Move> {
        if self.size[0] > 0 {
            Some(self.line[0][0])
        } else {
            None
        }
    }
    
    pub fn display(&self, score: i32) {
        if self.size[0] == 0 {
            return;
        }
        
        print!("PV: ");
        for i in 0..self.size[0].min(10) {
            print!("{} ", self.move_to_string(self.line[0][i]));
        }
        println!(" (score: {})", score);
    }
    
    fn move_to_string(&self, mv: Move) -> String {
        let from_file = (b'a' + mv.from.1 as u8) as char;
        let from_rank = (b'1' + mv.from.0 as u8) as char;
        let to_file = (b'a' + mv.to.1 as u8) as char;
        let to_rank = (b'1' + mv.to.0 as u8) as char;
        
        let mut result = format!("{}{}{}{}", from_file, from_rank, to_file, to_rank);
        
        if let Some(piece) = mv.promotion {
            let promo_char = match piece {
                Piece::Queen => 'q',
                Piece::Rook => 'r',
                Piece::Bishop => 'b',
                Piece::Knight => 'n',
                _ => '?',
            };
            result.push(promo_char);
        }
        
        result
    }
}

// Timer and node counting
#[derive(Clone)]
pub struct Timer {
    pub start_time: Instant,
    pub node_count: u64,
    pub root_depth: u32,
    pub is_stopping: bool,
    pub deadline: Option<Instant>,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            start_time: Instant::now(),
            node_count: 0,
            root_depth: 0,
            is_stopping: false,
            deadline: None,
        }
    }
    
    pub fn start(&mut self) {
        self.start_time = Instant::now();
        self.node_count = 0;
        self.is_stopping = false;
    }
    
    pub fn should_not_start_iteration(&self) -> bool {
        if let Some(deadline) = self.deadline {
            let now = Instant::now();
            if now >= deadline {
                return true;
            }
            
            // Don't start if we have less than 1/4 of allocated time remaining
            if self.root_depth >= 4 {
                let elapsed = now.duration_since(self.start_time);
                let total_time = deadline.duration_since(self.start_time);
                elapsed.as_millis() * 4 > total_time.as_millis() * 3
            } else {
                false
            }
        } else {
            false
        }
    }
    
    pub fn refresh_stats(&mut self) {
        // Update any timing statistics if needed
    }
    
    pub fn try_interrupting(&mut self) {
        if let Some(deadline) = self.deadline {
            if Instant::now() >= deadline {
                self.is_stopping = true;
            }
        }
    }
    
    pub fn get_nps(&self) -> u64 {
        let elapsed = self.start_time.elapsed().as_millis().max(1) as u64;
        (self.node_count * 1000) / elapsed
    }
}

// TT score conversion helpers  
#[inline]
fn score_to_tt(score: i32, ply: i32) -> i32 {
    if score >= EVAL_LIMIT {
        (score + ply).min(MATE_SCORE)
    } else if score <= -EVAL_LIMIT {
        (score - ply).max(-MATE_SCORE)  
    } else {
        score
    }
}

#[inline]
fn score_from_tt(score: i32, ply: i32) -> i32 {
    if score >= EVAL_LIMIT {
        (score - ply).clamp(-MATE_SCORE, MATE_SCORE)
    } else if score <= -EVAL_LIMIT {
        (score + ply).clamp(-MATE_SCORE, MATE_SCORE)
    } else {
        score
    }
}

// Set improving flag
fn set_improving(ppst: &Stack, eval: i32, ply: usize) -> bool {
    !(ply > 1 && ppst.previous_eval > eval)
}

// Global instances (to be moved to a proper context later)
pub struct SearchEngine {
    pub history: HistoryData,
    pub tt: TranspositionTable,
    pub pv: PvCollector,
    pub timer: Timer,
}

impl SearchEngine {
    pub fn new() -> Self {
        SearchEngine {
            history: HistoryData::new(),
            tt: TranspositionTable::new(16), // 16MB default
            pv: PvCollector::new(),
            timer: Timer::new(),
        }
    }
    
    pub fn on_new_game(&mut self) {
        self.history.clear();
        self.tt.clear();
    }
    
    pub fn think(&mut self, board: &mut Board, time_limit: Option<Instant>) -> Move {
        let mut sc = SearchContext::new();
        sc.clear();
        
        self.pv.clear();
        self.history.clear();
        self.tt.age();
        self.timer.start();
        self.timer.deadline = time_limit;
        
        let best_move = self.iterate(board, &mut sc);
        
        // Emergency fallback if no move found
        if let None = best_move {
            let legal_moves = board.generate_moves();
            if !legal_moves.is_empty() {
                return legal_moves[0];
            }
        }
        
        best_move.unwrap_or_else(|| {
            // Last resort - return a dummy move
            Move {
                from: Square(0, 0),
                to: Square(0, 0),
                is_castling: false,
                is_en_passant: false,
                promotion: None,
                captured_piece: None,
            }
        })
    }
    
    pub fn iterate(&mut self, board: &mut Board, sc: &mut SearchContext) -> Option<Move> {
        let mut val = 0;
        let mut cur_val = 0;
        let max_depth = 50; // Default max depth
        
        for depth in 1..=max_depth {
            self.timer.root_depth = depth;
            self.timer.refresh_stats();
            
            // Print root info
            self.print_root_info();
            
            // Stop searching because of soft time limit
            if self.timer.should_not_start_iteration() || self.timer.is_stopping {
                break;
            }
            
            cur_val = self.widen(board, sc, depth, cur_val);
            
            // Stop searching when we are sure of a checkmate score  
            if cur_val > EVAL_LIMIT || cur_val < -EVAL_LIMIT {
                let expected_mate_depth = (MATE_SCORE - cur_val.abs() + 1) + 1;
                if depth >= (expected_mate_depth * 3 / 2) as u32 {
                    break;
                }
            }
            
            val = cur_val;
            
            // Check for timeout
            if self.timer.is_stopping {
                break;
            }
        }
        
        self.pv.get_best_move()
    }
    
    pub fn widen(&mut self, board: &mut Board, sc: &mut SearchContext, depth: u32, last_score: i32) -> i32 {
        let mut current_depth_score = last_score;
        
        // Apply aspiration window on sufficient depth and if checkmate is not expected
        if depth > 6 && last_score < EVAL_LIMIT {
            // Progressively widen aspiration window
            for margin in [10, 20, 40, 80, 160, 320].iter() {
                let alpha = last_score - margin;
                let beta = last_score + margin;
                
                current_depth_score = self.search(board, sc, 0, alpha, beta, depth, false, false);
                
                // Timeout
                if self.timer.is_stopping {
                    break;
                }
                
                // We finished within the window, break the loop
                if current_depth_score > alpha && current_depth_score < beta {
                    return current_depth_score;
                }
                
                // Verify a checkmate by searching with infinite bounds
                if current_depth_score > EVAL_LIMIT {
                    break;
                }
            }
        }
        
        // Full window search, unless we broke out due to timeout
        if self.timer.is_stopping {
            last_score
        } else {
            self.search(board, sc, 0, -MATE_SCORE, MATE_SCORE, depth, false, false)
        }
    }
    
    fn print_root_info(&self) {
        println!("info depth {} nodes {} nps {} time {}",
            self.timer.root_depth,
            self.timer.node_count,
            self.get_nps(),
            self.timer.start_time.elapsed().as_millis()
        );
    }
    
    fn get_nps(&self) -> u64 {
        self.timer.get_nps()
    }
    
    // Main negamax search with advanced pruning techniques
    pub fn search(
        &mut self,
        board: &mut Board,
        sc: &mut SearchContext,
        ply: usize,
        mut alpha: i32,
        mut beta: i32,
        depth: u32,
        was_null_move: bool,
        is_excluded: bool,
    ) -> i32 {
        // Local variables
        let mut best_score: i32;
        let mut new_depth: u32;
        let mut eval: i32;
        let mut moves_tried = 0;
        let mut quiet_moves_tried = 0;
        let mut hash_flag: BoundType;
        let mut reduction: u32;
        let mut score: i32;
        let mut singular_score = -MATE_SCORE;
        let mut tt_move: Option<Move> = None;
        let mut best_move: Option<Move> = None;
        let mut singular_move: Option<Move> = None;
        let mut list_of_tried_moves: Vec<Move> = Vec::new();
        let mut singular_extension = false;
        
        // Init stack pointers
        let ply_i32 = ply as i32;
        if ply >= PLY_LIMIT { return board.evaluate(); }
        
        // Get values we need before borrowing mutably
        let pst_eval = if ply > 0 { sc.stack[ply - 1].previous_eval } else { 0 };
        let pst_capture_to = if ply > 0 { sc.stack[ply - 1].previous_capture_to } else { -1 };
        let ppst_eval = if ply > 1 { sc.stack[ply - 2].previous_eval } else { 0 };
        
        // Root node is different
        let is_root = ply == 0;
        
        // Distinguish between zero window nodes and PV nodes
        let is_pv = beta > alpha + 1;
        
        // QUIESCENCE SEARCH entry point
        if depth <= 0 {
            return self.quiesce(board, ply, 0, alpha, beta);
        }
        
        // Some bookkeeping
        self.timer.node_count += 1;
        self.pv.size[ply] = ply;
        
        // Periodically check for timeout
        self.timer.try_interrupting();
        
        // Exit to unwind search if timed out
        if self.timer.is_stopping {
            return 0;
        }
        
        // Quick exit on draw, unless at root
        if !is_root && board.is_draw() {
            return 0;
        }
        
        // MATE DISTANCE PRUNING
        if !is_root {
            alpha = alpha.max(-MATE_SCORE + ply_i32);
            beta = beta.min(MATE_SCORE - ply_i32 + 1);
            if alpha >= beta {
                return alpha;
            }
        }
        
        // READ THE TRANSPOSITION TABLE
        let mut found_tt_record = false;
        if let Some(entry) = self.tt.probe(board.hash) {
            tt_move = entry.best_move.clone();
            if entry.depth >= depth {
                let tt_score = score_from_tt(entry.score, ply_i32);
                hash_flag = entry.bound_type;
                
                // Reuse score if not PV or score is exact
                if !is_pv || (tt_score > alpha && tt_score < beta) {
                    if !is_root && !is_excluded {
                        return tt_score;
                    }
                }
            }
            found_tt_record = true;
        }
        
        // Safeguard against ply limit overflow
        if ply >= PLY_LIMIT - 1 {
            return board.evaluate();
        }
        
        // Prepare for singular extension
        if !is_root && depth > SINGULAR_DEPTH && sc.excluded_move.is_none() {
            if let Some(entry) = self.tt.probe(board.hash) {
                if matches!(entry.bound_type, BoundType::LowerBound) && entry.score < EVAL_LIMIT {
                    singular_move = entry.best_move.clone();
                    singular_score = entry.score;
                    singular_extension = true;
                }
            }
        }
        
        // Are we in check?
        let is_in_check_before_moving = board.is_in_check(board.current_color());
        
        // Evaluate position, unless in check
        eval = if is_in_check_before_moving {
            -MATE_SCORE
        } else {
            board.evaluate()
        };
        
        // Adjust node eval using TT score
        if found_tt_record {
            if let Some(entry) = self.tt.probe(board.hash) {
                let tt_score = score_from_tt(entry.score, ply_i32);
                match entry.bound_type {
                    BoundType::LowerBound if tt_score > eval => eval = tt_score,
                    BoundType::UpperBound if tt_score < eval => eval = tt_score,
                    _ => {}
                }
            }
        }
        
        // Save eval for current ply
        sc.stack[ply].previous_eval = eval;
        
        // Check if eval improved from 2 plies ago
        let improving = !(ply > 1 && ppst_eval > eval);
        
        // NODE-LEVEL PRUNING
        if !was_null_move && !is_in_check_before_moving && !is_pv && !is_excluded && self.can_try_null_move(board) {
            
            // STATIC NULL MOVE (Reverse Futility Pruning)
            if depth <= 6 {
                let margin = 135 * depth as i32;
                if eval - margin >= beta {
                    return eval - margin;
                }
            }
            
            // RAZORING
            if depth <= 3 && eval + 200 * (depth as i32) < beta {
                score = self.quiesce(board, ply, 0, alpha, beta);
                if score < beta {
                    return score;
                }
                if self.timer.is_stopping {
                    return 0;
                }
            }
            
            // NULL MOVE PRUNING
            if eval > beta && depth > 1 {
                // Set null move reduction
                reduction = 3 + depth / 6 + if eval - beta > 200 { 1 } else { 0 };
                
                // Do null move search
                let (prev_ep, prev_hash, prev_halfmove) = board.make_null_move();
                score = -self.search(board, sc, ply + 1, -beta, -beta + 1, depth.saturating_sub(1 + reduction), true, false);
                board.unmake_null_move(prev_ep, prev_hash, prev_halfmove);
                
                if self.timer.is_stopping {
                    return 0;
                }
                
                // NULL MOVE VERIFICATION
                if depth - reduction > 5 && score >= beta {
                    score = self.search(board, sc, ply, alpha, beta, depth.saturating_sub(reduction + 4), true, false);
                }
                
                if self.timer.is_stopping {
                    return 0;
                }
                
                if score >= beta {
                    return score;
                }
            }
        }
        
        // SET FUTILITY PRUNING FLAG
        let can_do_futility = depth <= 6 && !is_in_check_before_moving && !is_pv && eval + 75 * (depth as i32) < beta;
        
        // INTERNAL ITERATIVE REDUCTION
        if depth > 5 && !is_pv && tt_move.is_none() && !is_in_check_before_moving {
            new_depth = depth - 1;
        } else {
            new_depth = depth;
        }
        
        // Init moves and variables before main loop
        best_score = -MATE_SCORE;
        
        // Generate and order moves
        let mut legal_moves = board.generate_moves();
        if legal_moves.is_empty() {
            return if is_in_check_before_moving { -MATE_SCORE + ply_i32 } else { 0 };
        }
        
        // Order moves (simplified for now)
        self.order_moves(&mut legal_moves, board, tt_move.as_ref(), ply);
        
        // Main move loop
        for (move_idx, mv) in legal_moves.iter().enumerate() {
            // In singular search we omit the best move
            if let Some(excluded) = sc.excluded_move {
                if *mv == excluded && is_excluded {
                    continue;
                }
            }
            
            // Remember destination square if move is capture
            if self.is_move_noisy(board, mv) {
                sc.stack[ply].previous_capture_to = self.square_to_i8(mv.to);
            } else {
                sc.stack[ply].previous_capture_to = -1;
            }
            
            // Detect if move gives check
            let info = board.make_move(mv);
            let move_gives_check = board.is_in_check(board.current_color());
            board.unmake_move(mv, info);
            
            // Extensions
            let mut do_extension = false;
            
            // Check extension
            if move_gives_check && (is_pv || depth < 4) {
                do_extension = true;
            }
            
            // Recapture extension
            if ply > 0 && !do_extension {
                if pst_capture_to == self.square_to_i8(mv.to) && (is_pv || depth < 7) {
                    do_extension = true;
                }
            }
            
            // Singular extension
            if depth > SINGULAR_DEPTH && !do_extension && singular_extension 
                && Some(*mv) == singular_move && sc.excluded_move.is_none() {
                
                let new_alpha = -singular_score - 50;
                sc.excluded_move = Some(*mv);
                
                let exclusion_search_score = self.search(
                    board, sc, ply + 1, new_alpha, new_alpha + 1, 
                    (depth - 1) / 2, false, true
                );
                sc.excluded_move = None;
                
                if self.timer.is_stopping {
                    return 0;
                }
                
                if exclusion_search_score <= new_alpha {
                    do_extension = true;
                }
            }
            
            // Check basic conditions for pruning
            let is_capture = mv.captured_piece.is_some() || mv.is_en_passant;
            let can_prune_move = !is_pv && !is_in_check_before_moving && !move_gives_check && !is_capture && mv.promotion.is_none();
            
            // FUTILITY PRUNING
            if can_do_futility && moves_tried > 0 && can_prune_move {
                continue;
            }
            
            // LATE MOVE PRUNING
            if depth <= 3 && can_prune_move && quiet_moves_tried > ((3 + if improving { 1 } else { 0 }) * depth as usize) - 1 {
                continue;
            }
            
            // Make move
            let move_info = board.make_move(mv);
            
            // Filter out illegal moves
            if board.is_in_check(board.get_opposite_color(board.current_color())) {
                board.unmake_move(mv, move_info);
                continue;
            }
            
            // Update move statistics
            list_of_tried_moves.push(*mv);
            moves_tried += 1;
            if !is_capture && mv.promotion.is_none() {
                quiet_moves_tried += 1;
            }
            
            // Set new search depth
            new_depth = depth - 1 + if do_extension { 1 } else { 0 };
            
            // LATE MOVE REDUCTION
            if depth > 1 && quiet_moves_tried > 3 && !is_capture && mv.promotion.is_none() 
                && !is_in_check_before_moving && !move_gives_check {
                
                reduction = self.get_lmr_reduction(is_pv, depth, moves_tried, improving);
                reduction = reduction.min(new_depth - 1);
                
                if reduction > 0 {
                    score = -self.search(board, sc, ply + 1, -alpha - 1, -alpha, new_depth - reduction, false, false);
                    
                    if score <= alpha {
                        board.unmake_move(mv, move_info);
                        if self.timer.is_stopping {
                            return 0;
                        }
                        continue;
                    }
                }
            }
            
            // PVS (Principal Variation Search)
            if best_score == -MATE_SCORE {
                score = -self.search(board, sc, ply + 1, -beta, -alpha, new_depth, false, false);
            } else {
                score = -self.search(board, sc, ply + 1, -alpha - 1, -alpha, new_depth, false, false);
                if !self.timer.is_stopping && score > alpha {
                    score = -self.search(board, sc, ply + 1, -beta, -alpha, new_depth, false, false);
                }
            }
            
            // Undo move
            board.unmake_move(mv, move_info);
            
            if self.timer.is_stopping {
                return 0;
            }
            
            // Beta cutoff
            if score >= beta {
                // Update history and killers
                self.history.update(board, *mv, depth, ply);
                for tried_mv in &list_of_tried_moves {
                    self.history.update_tries(board, *tried_mv, depth);
                }
                
                // Store in TT
                if !is_excluded {
                    self.tt.store(
                        board.hash,
                        depth,
                        score_to_tt(score, ply_i32),
                        BoundType::LowerBound,
                        Some(*mv)
                    );
                }
                
                // Display PV if at root
                if is_root {
                    self.pv.update(ply, *mv);
                    self.pv.display(score);
                }
                
                return score;
            }
            
            // Update best score and alpha
            if score > best_score {
                best_score = score;
                
                if score > alpha {
                    alpha = score;
                    best_move = Some(*mv);
                    self.pv.update(ply, *mv);
                    if is_root {
                        self.pv.display(score);
                    }
                }
            }
        }
        
        // Return checkmate/stalemate score if no moves
        if best_score == -MATE_SCORE {
            return if board.is_in_check(board.current_color()) { -MATE_SCORE + ply_i32 } else { 0 };
        }
        
        // Save score in TT
        if !is_excluded {
            let original_alpha = alpha - (best_score - alpha).max(0); // Reconstruct original alpha
            let bound_type = if best_score <= original_alpha {
                BoundType::UpperBound
            } else if best_score >= beta {
                BoundType::LowerBound
            } else {
                BoundType::Exact
            };
            
            self.tt.store(
                board.hash,
                depth,
                score_to_tt(best_score, ply_i32),
                bound_type,
                best_move
            );
        }
        
        best_score
    }
    
    // Quiescence search based on Publius
    pub fn quiesce(
        &mut self,
        board: &mut Board,
        ply: usize,
        _qdepth: u32,
        mut alpha: i32,
        beta: i32,
    ) -> i32 {
        let mut best_score: i32;
        let mut hash_flag: BoundType;
        let mut score: i32;
        let mut tt_move: Option<Move> = None;
        let mut best_move: Option<Move> = None;
        let save_in_tt = true;
        
        // Are we in PV node?
        let is_pv = beta > alpha + 1;
        
        // Statistics
        self.timer.node_count += 1;
        
        // Check for timeout
        self.timer.try_interrupting();
        
        if self.timer.is_stopping {
            return 0;
        }
        
        // Retrieve score from TT
        if let Some(entry) = self.tt.probe(board.hash) {
            tt_move = entry.best_move.clone();
            if !is_pv || (entry.score > alpha && entry.score < beta) {
                return entry.score;
            }
        }
        
        self.pv.size[ply] = ply;
        
        // Draw detection
        if board.is_draw() {
            return 0;
        }
        
        // Safeguard against ply limit
        if ply >= PLY_LIMIT - 1 {
            return board.evaluate();
        }
        
        // Are we in check?
        let is_in_check = board.is_in_check(board.current_color());
        
        // Get stand-pat score
        best_score = if is_in_check { -MATE_SCORE } else { board.evaluate() };
        
        // Static score cutoff
        if best_score >= beta {
            return best_score;
        }
        
        // Guaranteed score if we don't find anything better
        if best_score > alpha {
            alpha = best_score;
        }
        
        // Generate captures and checks
        let mut tactical_moves = if is_in_check {
            board.generate_moves() // All moves when in check
        } else {
            board.generate_tactical_moves()
        };
        
        // Order tactical moves
        tactical_moves.sort_by_key(|mv| {
            let mut score = 0;
            
            // Hash move first
            if let Some(hash_mv) = &tt_move {
                if mv == hash_mv {
                    return -10000;
                }
            }
            
            // MVV-LVA for captures
            if let Some(victim) = mv.captured_piece {
                score -= 1000 + crate::piece_value(victim);
                if let Some((_, attacker)) = board.piece_at(mv.from) {
                    score += crate::piece_value(attacker) / 10;
                }
            }
            
            score
        });
        
        // Search tactical moves
        for mv in tactical_moves {
            if self.timer.is_stopping {
                break;
            }
            
            // SEE pruning for bad captures
            if mv.captured_piece.is_some() && board.see(&mv) < 0 {
                continue;
            }
            
            // Make move
            let move_info = board.make_move(&mv);
            
            // Skip illegal moves
            if board.is_in_check(board.get_opposite_color(board.current_color())) {
                board.unmake_move(&mv, move_info);
                continue;
            }
            
            // Recursion
            score = -self.quiesce(board, ply + 1, _qdepth + 1, -beta, -alpha);
            
            // Unmake move
            board.unmake_move(&mv, move_info);
            
            if self.timer.is_stopping {
                return 0;
            }
            
            // Beta cutoff
            if score >= beta {
                if save_in_tt {
                    self.tt.store(
                        board.hash,
                        0,
                        score,
                        BoundType::LowerBound,
                        Some(mv)
                    );
                }
                return score;
            }
            
            // Update best score and alpha
            if score > best_score {
                best_score = score;
                if score > alpha {
                    best_move = Some(mv);
                    alpha = score;
                    self.pv.update(ply, mv);
                }
            }
        }
        
        // Return correct checkmate/stalemate score
        if best_score == -MATE_SCORE {
            return if board.is_in_check(board.current_color()) { -MATE_SCORE + ply as i32 } else { 0 };
        }
        
        // Save result in TT
        if save_in_tt {
            let bound_type = if best_move.is_some() {
                BoundType::Exact
            } else {
                BoundType::UpperBound
            };
            
            self.tt.store(
                board.hash,
                0,
                best_score,
                bound_type,
                best_move
            );
        }
        
        best_score
    }
    
    // Helper functions
    fn can_try_null_move(&self, board: &Board) -> bool {
        let current_color = board.current_color();
        let mut piece_count = 0;
        
        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = board.piece_at(Square(rank, file)) {
                    if color == current_color && piece != Piece::Pawn && piece != Piece::King {
                        piece_count += 1;
                    }
                }
            }
        }
        
        piece_count > 0
    }
    
    fn is_move_noisy(&self, board: &Board, mv: &Move) -> bool {
        mv.captured_piece.is_some() || mv.is_en_passant || mv.promotion.is_some()
    }
    
    fn square_to_i8(&self, sq: Square) -> i8 {
        (sq.0 * 8 + sq.1) as i8
    }
    
    fn get_lmr_reduction(&self, is_pv: bool, depth: u32, moves_tried: usize, improving: bool) -> u32 {
        // Simple LMR table approximation
        let base_reduction = if depth >= 3 && moves_tried >= 4 {
            1 + (depth / 4).min(3)
        } else {
            0
        };
        
        let mut reduction = base_reduction;
        
        if !is_pv {
            reduction += 1;
        }
        
        if !improving {
            reduction += 1;
        }
        
        reduction.min(depth - 1)
    }
    
    fn order_moves(&self, moves: &mut Vec<Move>, board: &Board, tt_move: Option<&Move>, ply: usize) {
        // Simple move ordering - hash move first, then captures, then killers, then history
        moves.sort_by_key(|mv| {
            let mut score = 0;
            
            // Hash move gets highest priority
            if let Some(hash_mv) = tt_move {
                if mv == hash_mv {
                    return -10000;
                }
            }
            
            // Captures by MVV-LVA
            if let Some(victim) = mv.captured_piece {
                score -= 1000 + crate::piece_value(victim);
                if let Some((_, attacker)) = board.piece_at(mv.from) {
                    score += crate::piece_value(attacker) / 10;
                }
            }
            
            // Killers
            if self.history.is_killer(*mv, ply) {
                score -= 500;
            }
            
            // History score
            score -= self.history.get_score(board, *mv) / 100;
            
            score
        });
    }
}