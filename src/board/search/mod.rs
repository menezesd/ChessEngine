//! Search module implementing alpha-beta with iterative deepening.
//!
//! Features:
//! - Iterative deepening with aspiration windows
//! - Alpha-beta search with null move pruning and LMR
//! - Quiescence search with stand-pat
//! - Move ordering (TT move, killers, MVV-LVA, history)
//! - Transposition table for move ordering and cutoffs

mod move_order;
mod params;
mod simple;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::Instant;

use crate::tt::TranspositionTable;

use super::{Board, Move, MAX_PLY};
pub use params::SearchParams;

/// Result of a search containing best move and ponder move
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    /// The best move found
    pub best_move: Option<Move>,
    /// The expected opponent reply (for pondering)
    pub ponder_move: Option<Move>,
}

/// Default transposition table size in MB
pub const DEFAULT_TT_MB: usize = 1024;

/// Mate score constant
pub(crate) const MATE_SCORE: i32 = 200000;

/// Statistics tracked during search
pub struct SearchStats {
    pub nodes: u64,
    pub seldepth: u32,
    pub total_nodes: u64,
    pub max_nodes: u64,
}

impl SearchStats {
    pub fn reset_search(&mut self) {
        self.nodes = 0;
        self.seldepth = 0;
        self.total_nodes = 0;
    }

    pub fn reset_iteration(&mut self) {
        self.nodes = 0;
        self.seldepth = 0;
    }
}

/// Tables used during search (TT, killers, history, counter moves)
pub struct SearchTables {
    pub tt: TranspositionTable,
    pub killer_moves: [[Move; 2]; MAX_PLY],
    pub history: [i32; 4096],
    pub counter_moves: [[Move; 64]; 64],
}

impl SearchTables {
    /// MVV-LVA score for a capture move
    #[must_use] 
    pub fn mvv_lva_score(&self, mv: &Move) -> i32 {
        let captured = match mv.captured_piece {
            Some(piece) => move_order::piece_value(piece),
            None => return 0,
        };
        // Simple MVV-LVA: prioritize capturing high-value pieces
        captured * 10
    }

    /// Get history score for a move
    #[must_use] 
    pub fn history_score(&self, mv: &Move) -> i32 {
        let from = mv.from.index().as_usize();
        let to = mv.to.index().as_usize();
        let idx = from * 64 + to;
        if idx < self.history.len() {
            self.history[idx]
        } else {
            0
        }
    }

    /// Update history on beta cutoff
    pub fn update_history(&mut self, mv: &Move, depth: u32) {
        let from = mv.from.index().as_usize();
        let to = mv.to.index().as_usize();
        let idx = from * 64 + to;
        if idx < self.history.len() {
            self.history[idx] = self.history[idx].saturating_add((depth * depth * depth) as i32);
        }
    }

    /// Reset history table
    pub fn reset_history(&mut self) {
        self.history = [0; 4096];
    }
}

/// Search state persisted across searches
pub struct SearchState {
    pub stats: SearchStats,
    pub tables: SearchTables,
    pub generation: u16,
    pub last_move: Move,
    pub hard_stop_at: Option<Instant>,
    pub params: SearchParams,
    pub trace: bool,
}

impl SearchState {
    #[must_use] 
    pub fn new(tt_mb: usize) -> Self {
        SearchState {
            stats: SearchStats {
                nodes: 0,
                seldepth: 0,
                total_nodes: 0,
                max_nodes: 0,
            },
            tables: SearchTables {
                tt: TranspositionTable::new(tt_mb),
                killer_moves: [[super::EMPTY_MOVE; 2]; MAX_PLY],
                history: [0; 4096],
                counter_moves: [[super::EMPTY_MOVE; 64]; 64],
            },
            generation: 0,
            last_move: super::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            trace: false,
        }
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.stats.reset_search();
        self.last_move = super::EMPTY_MOVE;
        self.hard_stop_at = None;
    }

    pub fn set_max_nodes(&mut self, max_nodes: u64) {
        self.stats.max_nodes = max_nodes;
    }

    pub fn set_hard_stop_at(&mut self, stop_at: Option<Instant>) {
        self.hard_stop_at = stop_at;
    }

    pub fn params_mut(&mut self) -> &mut SearchParams {
        &mut self.params
    }

    #[must_use] 
    pub fn params(&self) -> &SearchParams {
        &self.params
    }

    pub fn set_params(&mut self, params: SearchParams) {
        self.params = params;
    }

    #[must_use] 
    pub fn trace(&self) -> bool {
        self.trace
    }

    pub fn set_trace(&mut self, trace: bool) {
        self.trace = trace;
    }

    pub fn reset_tables(&mut self, tt_mb: usize) {
        self.tables.tt = TranspositionTable::new(tt_mb);
        self.stats.reset_search();
    }

    #[must_use] 
    pub fn hashfull_per_mille(&self) -> u32 {
        self.tables.tt.hashfull_per_mille()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState::new(DEFAULT_TT_MB)
    }
}

/// Time limits for a search
pub struct SearchLimits {
    pub clock: std::sync::Arc<SearchClock>,
    pub stop: std::sync::Arc<AtomicBool>,
}

/// Clock for tracking search time limits
pub struct SearchClock {
    start_time: Mutex<Instant>,
    soft_deadline: Mutex<Option<Instant>>,
    hard_deadline: Mutex<Option<Instant>>,
}

impl SearchClock {
    #[must_use] 
    pub fn new(
        start_time: Instant,
        soft_deadline: Option<Instant>,
        hard_deadline: Option<Instant>,
    ) -> Self {
        SearchClock {
            start_time: Mutex::new(start_time),
            soft_deadline: Mutex::new(soft_deadline),
            hard_deadline: Mutex::new(hard_deadline),
        }
    }

    pub fn reset(
        &self,
        start_time: Instant,
        soft_deadline: Option<Instant>,
        hard_deadline: Option<Instant>,
    ) {
        if let Ok(mut start) = self.start_time.lock() {
            *start = start_time;
        }
        if let Ok(mut soft) = self.soft_deadline.lock() {
            *soft = soft_deadline;
        }
        if let Ok(mut hard) = self.hard_deadline.lock() {
            *hard = hard_deadline;
        }
    }

    pub fn snapshot(&self) -> (Instant, Option<Instant>, Option<Instant>) {
        let start_time = *self.start_time.lock().unwrap();
        let soft_deadline = *self.soft_deadline.lock().unwrap();
        let hard_deadline = *self.hard_deadline.lock().unwrap();
        (start_time, soft_deadline, hard_deadline)
    }
}

/// Extract ponder move by making best move and probing TT
fn extract_ponder_move(board: &mut Board, state: &SearchState, best_move: Move) -> Option<Move> {
    // Make the best move temporarily
    let info = board.make_move(&best_move);

    // Probe TT for opponent's expected reply
    let ponder = state.tables.tt.probe(board.hash).and_then(|entry| {
        entry.best_move().filter(|mv| {
            // Verify move is legal
            let moves = board.generate_moves();
            moves.iter().any(|m| m == mv)
        })
    });

    // Unmake the move
    board.unmake_move(&best_move, info);

    ponder
}

/// Find best move with fixed depth limit
pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> Option<Move> {
    simple::simple_search(board, state, max_depth, 0, 0, stop)
}

/// Find best move with fixed depth limit, returning ponder move too
pub fn find_best_move_with_ponder(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> SearchResult {
    let best_move = simple::simple_search(board, state, max_depth, 0, 0, stop);
    let ponder_move = best_move.and_then(|mv| extract_ponder_move(board, state, mv));
    SearchResult { best_move, ponder_move }
}

/// Find best move with time control
pub fn find_best_move_with_time(
    board: &mut Board,
    state: &mut SearchState,
    limits: &SearchLimits,
) -> Option<Move> {
    // Calculate time limit from clock
    let (_, soft_deadline, _) = limits.clock.snapshot();
    let time_limit_ms = soft_deadline
        .map_or(0, |d| d.saturating_duration_since(Instant::now()).as_millis() as u64);

    // Max depth of 64 for time-based search
    simple::simple_search(board, state, 64, time_limit_ms, 0, &limits.stop)
}

/// Find best move with time control, returning ponder move too
pub fn find_best_move_with_time_and_ponder(
    board: &mut Board,
    state: &mut SearchState,
    limits: &SearchLimits,
) -> SearchResult {
    let best_move = find_best_move_with_time(board, state, limits);
    let ponder_move = best_move.and_then(|mv| extract_ponder_move(board, state, mv));
    SearchResult { best_move, ponder_move }
}
