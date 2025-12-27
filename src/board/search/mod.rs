//! Search module implementing alpha-beta with iterative deepening.
//!
//! Features:
//! - Iterative deepening with aspiration windows
//! - Alpha-beta search with null move pruning and LMR
//! - Quiescence search with stand-pat
//! - Move ordering (TT move, killers, MVV-LVA, history)
//! - Transposition table for move ordering and cutoffs
//! - Lazy SMP parallel search support

mod constants;
mod move_order;
mod params;
mod simple;
pub mod smp;

use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use crate::tt::TranspositionTable;

use super::{Board, Move, Piece, MAX_PLY};
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
pub(crate) const MATE_SCORE: i32 = constants::MATE_THRESHOLD + MAX_PLY as i32;

/// Statistics tracked during search
pub struct SearchStats {
    pub nodes: u64,
    pub seldepth: u32,
    pub total_nodes: u64,
    pub max_nodes: u64,
    pub tt_hits: u64,
}

impl SearchStats {
    pub fn reset_search(&mut self) {
        self.nodes = 0;
        self.seldepth = 0;
        self.total_nodes = 0;
        self.tt_hits = 0;
    }

    pub fn reset_iteration(&mut self) {
        self.nodes = 0;
        self.seldepth = 0;
    }
}

pub struct KillerTable {
    slots: [[Move; 2]; MAX_PLY],
}

impl KillerTable {
    pub fn new() -> Self {
        KillerTable {
            slots: [[super::EMPTY_MOVE; 2]; MAX_PLY],
        }
    }

    #[must_use]
    pub fn primary(&self, ply: usize) -> Move {
        self.slots
            .get(ply)
            .map(|row| row[0])
            .unwrap_or(super::EMPTY_MOVE)
    }

    #[must_use]
    pub fn secondary(&self, ply: usize) -> Move {
        self.slots
            .get(ply)
            .map(|row| row[1])
            .unwrap_or(super::EMPTY_MOVE)
    }

    pub fn update(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.slots[ply][0] != mv {
            self.slots[ply][1] = self.slots[ply][0];
            self.slots[ply][0] = mv;
        }
    }

    pub fn reset(&mut self) {
        for killers in self.slots.iter_mut() {
            killers[0] = super::EMPTY_MOVE;
            killers[1] = super::EMPTY_MOVE;
        }
    }
}

pub struct HistoryTable {
    entries: [i32; 4096],
}

impl HistoryTable {
    pub fn new() -> Self {
        HistoryTable { entries: [0; 4096] }
    }

    #[must_use]
    pub fn score(&self, mv: &Move) -> i32 {
        let from = mv.from().index();
        let to = mv.to().index();
        let idx = from * 64 + to;
        self.entries.get(idx).copied().unwrap_or(0)
    }

    pub fn update(&mut self, mv: &Move, depth: u32) {
        let from = mv.from().index();
        let to = mv.to().index();
        let idx = from * 64 + to;
        if let Some(entry) = self.entries.get_mut(idx) {
            *entry = entry.saturating_add((depth * depth * depth) as i32);
        }
    }

    pub fn decay(&mut self) {
        for entry in self.entries.iter_mut() {
            *entry >>= 2;
        }
    }

    pub fn reset(&mut self) {
        self.entries = [0; 4096];
    }
}

pub struct CounterMoveTable {
    entries: [[Move; 64]; 64],
}

impl CounterMoveTable {
    pub fn new() -> Self {
        CounterMoveTable {
            entries: [[super::EMPTY_MOVE; 64]; 64],
        }
    }

    #[must_use]
    pub fn get(&self, from: usize, to: usize) -> Move {
        if from < 64 && to < 64 {
            self.entries[from][to]
        } else {
            super::EMPTY_MOVE
        }
    }

    pub fn set(&mut self, from: usize, to: usize, mv: Move) {
        if from < 64 && to < 64 {
            self.entries[from][to] = mv;
        }
    }

    pub fn reset(&mut self) {
        for counters in self.entries.iter_mut() {
            for mv in counters.iter_mut() {
                *mv = super::EMPTY_MOVE;
            }
        }
    }
}

/// Tables used during search (TT, killers, history, counter moves)
pub struct SearchTables {
    /// Shared transposition table (thread-safe, can be shared across workers)
    pub tt: Arc<TranspositionTable>,
    /// Per-thread killer move table
    pub killer_moves: KillerTable,
    /// Per-thread history heuristic table
    pub history: HistoryTable,
    /// Per-thread counter move table
    pub counter_moves: CounterMoveTable,
}

impl SearchTables {
    /// MVV-LVA score for a capture move
    /// Prioritizes capturing high-value pieces with low-value attackers
    #[must_use]
    pub fn mvv_lva_score(&self, board: &Board, mv: &Move) -> i32 {
        if !mv.is_capture() {
            return 0;
        }

        // Get attacker piece value
        let attacker = match board.piece_at(mv.from()) {
            Some((_, piece)) => move_order::piece_value(piece),
            None => return 0,
        };

        // For en passant, captured piece is always a pawn
        if mv.is_en_passant() {
            return move_order::piece_value(Piece::Pawn) * 10 - attacker;
        }

        // Look up what piece is on the target square
        let captured = match board.piece_at(mv.to()) {
            Some((_, piece)) => move_order::piece_value(piece),
            None => return 0,
        };
        // MVV-LVA: prioritize high-value victims captured by low-value attackers
        captured * 10 - attacker
    }

    /// Get history score for a move
    #[must_use]
    pub fn history_score(&self, mv: &Move) -> i32 {
        self.history.score(mv)
    }

    /// Update history on beta cutoff
    pub fn update_history(&mut self, mv: &Move, depth: u32) {
        self.history.update(mv, depth);
    }

    /// Reset history table
    pub fn reset_history(&mut self) {
        self.history.reset();
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
                tt_hits: 0,
            },
            tables: SearchTables {
                tt: Arc::new(TranspositionTable::new(tt_mb)),
                killer_moves: KillerTable::new(),
                history: HistoryTable::new(),
                counter_moves: CounterMoveTable::new(),
            },
            generation: 0,
            last_move: super::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            trace: false,
        }
    }

    /// Create a new SearchState with a shared transposition table.
    /// Used for SMP workers that share a TT but have separate local tables.
    #[must_use]
    pub fn with_shared_tt(tt: Arc<TranspositionTable>, generation: u16) -> Self {
        SearchState {
            stats: SearchStats {
                nodes: 0,
                seldepth: 0,
                total_nodes: 0,
                max_nodes: 0,
                tt_hits: 0,
            },
            tables: SearchTables {
                tt,
                killer_moves: KillerTable::new(),
                history: HistoryTable::new(),
                counter_moves: CounterMoveTable::new(),
            },
            generation,
            last_move: super::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            trace: false,
        }
    }

    /// Get a clone of the shared TT Arc for use by SMP workers
    #[must_use]
    pub fn shared_tt(&self) -> Arc<TranspositionTable> {
        Arc::clone(&self.tables.tt)
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.stats.reset_search();
        self.last_move = super::EMPTY_MOVE;
        self.hard_stop_at = None;
        // Decay history and clear tactical helpers to avoid stale biases.
        self.tables.history.decay();
        self.tables.killer_moves.reset();
        self.tables.counter_moves.reset();
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
        self.tables.tt = Arc::new(TranspositionTable::new(tt_mb));
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
        let mut start = self.start_time.lock();
        *start = start_time;
        let mut soft = self.soft_deadline.lock();
        *soft = soft_deadline;
        let mut hard = self.hard_deadline.lock();
        *hard = hard_deadline;
    }

    pub fn snapshot(&self) -> (Instant, Option<Instant>, Option<Instant>) {
        let start_time = *self.start_time.lock();
        let soft_deadline = *self.soft_deadline.lock();
        let hard_deadline = *self.hard_deadline.lock();
        (start_time, soft_deadline, hard_deadline)
    }
}

// ============================================================================
// UNIFIED SEARCH API
// ============================================================================

/// Configuration for a search operation.
///
/// This struct consolidates all search parameters into a single configuration
/// object, replacing the need for multiple `find_best_move_*` functions.
#[derive(Clone)]
pub struct SearchConfig {
    /// Maximum depth to search (None = unlimited, defaults to 64)
    pub max_depth: Option<u32>,
    /// Time limit in milliseconds (0 = unlimited)
    pub time_limit_ms: u64,
    /// Node limit (0 = unlimited)
    pub node_limit: u64,
    /// Whether to extract ponder move from TT after search
    pub extract_ponder: bool,
    /// Optional callback for iteration info
    pub info_callback: Option<SearchInfoCallback>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            max_depth: None,
            time_limit_ms: 0,
            node_limit: 0,
            extract_ponder: true,
            info_callback: None,
        }
    }
}

impl SearchConfig {
    /// Create a depth-limited search config
    #[must_use]
    pub fn depth(max_depth: u32) -> Self {
        SearchConfig {
            max_depth: Some(max_depth),
            ..Default::default()
        }
    }

    /// Create a time-limited search config
    #[must_use]
    pub fn time(time_limit_ms: u64) -> Self {
        SearchConfig {
            time_limit_ms,
            ..Default::default()
        }
    }

    /// Create a config from SearchLimits
    #[must_use]
    pub fn from_limits(limits: &SearchLimits) -> Self {
        let (_, soft_deadline, _) = limits.clock.snapshot();
        let time_limit_ms = soft_deadline.map_or(0, |d| {
            d.saturating_duration_since(Instant::now()).as_millis() as u64
        });
        SearchConfig {
            time_limit_ms,
            ..Default::default()
        }
    }

    /// Set whether to extract ponder move
    #[must_use]
    pub fn with_ponder(mut self, extract_ponder: bool) -> Self {
        self.extract_ponder = extract_ponder;
        self
    }

    /// Set node limit
    #[must_use]
    pub fn with_nodes(mut self, node_limit: u64) -> Self {
        self.node_limit = node_limit;
        self
    }

    /// Attach a callback for iteration info reporting.
    #[must_use]
    pub fn with_info_callback(mut self, callback: SearchInfoCallback) -> Self {
        self.info_callback = Some(callback);
        self
    }
}

/// Information about a completed search iteration.
#[derive(Debug, Clone)]
pub struct SearchIterationInfo {
    pub depth: u32,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
    pub score: i32,
    pub mate_in: Option<i32>,
    pub pv: String,
    pub seldepth: u32,
    pub tt_hits: u64,
}

/// Callback type for iteration info.
pub type SearchInfoCallback = Arc<dyn Fn(&SearchIterationInfo) + Send + Sync>;

/// Extract ponder move by making best move and probing TT
fn extract_ponder_move(board: &mut Board, state: &SearchState, best_move: Move) -> Option<Move> {
    // Make the best move temporarily
    let info = board.make_move(best_move);

    // Probe TT for opponent's expected reply
    let ponder = state.tables.tt.probe(board.hash).and_then(|entry| {
        entry.best_move().filter(|mv| {
            // Verify move is legal
            let moves = board.generate_moves();
            moves.iter().any(|m| m == mv)
        })
    });

    // Unmake the move
    board.unmake_move(best_move, info);

    ponder
}

/// Unified search function that accepts a configuration.
///
/// This is the preferred API for running searches. It consolidates
/// all the `find_best_move_*` variants into a single function.
///
/// # Example
/// ```ignore
/// let config = SearchConfig::depth(10).with_ponder(true);
/// let result = search(board, state, config, &stop);
/// ```
pub fn search(
    board: &mut Board,
    state: &mut SearchState,
    config: SearchConfig,
    stop: &AtomicBool,
) -> SearchResult {
    let max_depth = config.max_depth.unwrap_or(64);
    let info_callback = config.info_callback.clone();
    let best_move = simple::simple_search(
        board,
        state,
        max_depth,
        config.time_limit_ms,
        config.node_limit,
        stop,
        info_callback,
    );

    let ponder_move = if config.extract_ponder {
        best_move.and_then(|mv| extract_ponder_move(board, state, mv))
    } else {
        None
    };

    SearchResult {
        best_move,
        ponder_move,
    }
}

// ============================================================================
// LEGACY API (for backward compatibility)
// ============================================================================

/// Find best move with fixed depth limit
pub fn find_best_move(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> Option<Move> {
    simple::simple_search(board, state, max_depth, 0, 0, stop, None)
}

/// Find best move with fixed depth limit, returning ponder move too
pub fn find_best_move_with_ponder(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    stop: &AtomicBool,
) -> SearchResult {
    search(board, state, SearchConfig::depth(max_depth), stop)
}

/// Find best move with time control
pub fn find_best_move_with_time(
    board: &mut Board,
    state: &mut SearchState,
    limits: &SearchLimits,
) -> Option<Move> {
    let config = SearchConfig::from_limits(limits).with_ponder(false);
    search(board, state, config, &limits.stop).best_move
}

/// Find best move with time control, returning ponder move too
pub fn find_best_move_with_time_and_ponder(
    board: &mut Board,
    state: &mut SearchState,
    limits: &SearchLimits,
) -> SearchResult {
    let config = SearchConfig::from_limits(limits);
    search(board, state, config, &limits.stop)
}
