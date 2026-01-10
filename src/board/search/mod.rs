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
    slots: [[Move; 3]; MAX_PLY],
}

impl Default for KillerTable {
    fn default() -> Self {
        Self::new()
    }
}

impl KillerTable {
    #[must_use]
    pub fn new() -> Self {
        KillerTable {
            slots: [[super::EMPTY_MOVE; 3]; MAX_PLY],
        }
    }

    #[must_use]
    pub fn primary(&self, ply: usize) -> Move {
        self.slots.get(ply).map_or(super::EMPTY_MOVE, |row| row[0])
    }

    #[must_use]
    pub fn secondary(&self, ply: usize) -> Move {
        self.slots.get(ply).map_or(super::EMPTY_MOVE, |row| row[1])
    }

    #[must_use]
    pub fn tertiary(&self, ply: usize) -> Move {
        self.slots.get(ply).map_or(super::EMPTY_MOVE, |row| row[2])
    }

    pub fn update(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.slots[ply][0] != mv {
            self.slots[ply][2] = self.slots[ply][1];
            self.slots[ply][1] = self.slots[ply][0];
            self.slots[ply][0] = mv;
        }
    }

    pub fn reset(&mut self) {
        for killers in &mut self.slots {
            killers[0] = super::EMPTY_MOVE;
            killers[1] = super::EMPTY_MOVE;
            killers[2] = super::EMPTY_MOVE;
        }
    }
}

pub struct HistoryTable {
    entries: [i32; 4096],
}

impl Default for HistoryTable {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryTable {
    #[must_use]
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

    /// Update history score for a move that caused a beta cutoff
    pub fn update(&mut self, mv: &Move, depth: u32, _ply: usize) {
        let from = mv.from().index();
        let to = mv.to().index();
        let idx = from * 64 + to;
        if let Some(entry) = self.entries.get_mut(idx) {
            let bonus = (depth * depth * depth) as i32;
            *entry = entry.saturating_add(bonus);
        }
    }

    /// Penalize a move that failed to cause a cutoff (negative history)
    pub fn penalize(&mut self, mv: &Move, depth: u32, _ply: usize) {
        let from = mv.from().index();
        let to = mv.to().index();
        let idx = from * 64 + to;
        if let Some(entry) = self.entries.get_mut(idx) {
            let penalty = (depth * depth) as i32;
            *entry = entry.saturating_sub(penalty);
        }
    }

    pub fn decay(&mut self) {
        for entry in &mut self.entries {
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

impl Default for CounterMoveTable {
    fn default() -> Self {
        Self::new()
    }
}

impl CounterMoveTable {
    #[must_use]
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
        for counters in &mut self.entries {
            for mv in counters {
                *mv = super::EMPTY_MOVE;
            }
        }
    }
}

/// Continuation history table - tracks what moves work well after previous moves.
///
/// Indexed by `[prev_piece][prev_to][curr_from][curr_to]` simplified to
/// `[prev_piece * 64 + prev_to][curr_from * 64 + curr_to]`.
/// We use 6 piece types * 64 squares = 384 outer slots, each with 4096 inner entries.
pub struct ContinuationHistory {
    /// [piece * 64 + to] -> [from * 64 + to] -> score
    entries: Box<[[i16; 4096]; 384]>,
}

impl Default for ContinuationHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ContinuationHistory {
    #[must_use]
    pub fn new() -> Self {
        ContinuationHistory {
            entries: Box::new([[0i16; 4096]; 384]),
        }
    }

    /// Get continuation history score for a move following a previous move
    #[must_use]
    pub fn score(&self, prev_piece: Piece, prev_to: usize, mv: &Move) -> i32 {
        let outer_idx = prev_piece as usize * 64 + prev_to;
        let inner_idx = mv.from().index() * 64 + mv.to().index();
        if outer_idx < 384 {
            i32::from(self.entries[outer_idx][inner_idx])
        } else {
            0
        }
    }

    /// Update continuation history on beta cutoff
    pub fn update(&mut self, prev_piece: Piece, prev_to: usize, mv: &Move, depth: u32) {
        let outer_idx = prev_piece as usize * 64 + prev_to;
        let inner_idx = mv.from().index() * 64 + mv.to().index();
        if outer_idx < 384 {
            let bonus = (depth * depth) as i16;
            let entry = &mut self.entries[outer_idx][inner_idx];
            // Saturating add with clamping to prevent overflow
            *entry = entry.saturating_add(bonus).min(16000);
        }
    }

    /// Decay all entries
    pub fn decay(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry >>= 2;
            }
        }
    }

    /// Reset all entries
    pub fn reset(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry = 0;
            }
        }
    }
}

/// Countermove history table - tracks what responses work well against opponent moves.
///
/// Unlike continuation history (which uses our previous move), this uses the opponent's
/// previous move (`prev_piece`, `prev_to`) and our current move's piece type.
/// Indexed by `[opp_piece * 64 + opp_to][our_piece * 64 + our_to]`.
pub struct CountermoveHistory {
    /// `[prev_piece * 64 + prev_to]` -> `[piece * 64 + to]` -> score
    entries: Box<[[i16; 4096]; 384]>,
}

impl Default for CountermoveHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CountermoveHistory {
    #[must_use]
    pub fn new() -> Self {
        CountermoveHistory {
            entries: Box::new([[0i16; 4096]; 384]),
        }
    }

    /// Get countermove history score for responding to opponent's move
    #[must_use]
    pub fn score(&self, opp_piece: Piece, opp_to: usize, our_piece: Piece, mv: &Move) -> i32 {
        let outer_idx = opp_piece as usize * 64 + opp_to;
        let inner_idx = our_piece as usize * 64 + mv.to().index();
        if outer_idx < 384 {
            i32::from(self.entries[outer_idx][inner_idx])
        } else {
            0
        }
    }

    /// Update countermove history on beta cutoff
    pub fn update(
        &mut self,
        opp_piece: Piece,
        opp_to: usize,
        our_piece: Piece,
        mv: &Move,
        depth: u32,
    ) {
        let outer_idx = opp_piece as usize * 64 + opp_to;
        let inner_idx = our_piece as usize * 64 + mv.to().index();
        if outer_idx < 384 {
            let bonus = (depth * depth) as i16;
            let entry = &mut self.entries[outer_idx][inner_idx];
            *entry = entry.saturating_add(bonus).min(16000);
        }
    }

    /// Decay all entries
    pub fn decay(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry >>= 2;
            }
        }
    }

    /// Reset all entries
    pub fn reset(&mut self) {
        for outer in self.entries.iter_mut() {
            for entry in outer.iter_mut() {
                *entry = 0;
            }
        }
    }
}

/// Capture history table - tracks which captures historically cause cutoffs.
/// Indexed by `[attacker_piece][victim_piece]` for a 6x6 = 36 entry table.
pub struct CaptureHistory {
    entries: [[i32; 6]; 6],
}

impl Default for CaptureHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHistory {
    #[must_use]
    pub fn new() -> Self {
        CaptureHistory {
            entries: [[0; 6]; 6],
        }
    }

    /// Get capture history score for a capture move
    #[must_use]
    pub fn score(&self, attacker: Piece, victim: Piece) -> i32 {
        self.entries[attacker as usize][victim as usize]
    }

    /// Update capture history on beta cutoff
    pub fn update(&mut self, attacker: Piece, victim: Piece, depth: u32) {
        let bonus = (depth * depth * depth) as i32;
        let entry = &mut self.entries[attacker as usize][victim as usize];
        // Saturating add with clamping to prevent overflow
        *entry = entry.saturating_add(bonus).min(50000);
    }

    /// Decay all entries
    pub fn decay(&mut self) {
        for row in &mut self.entries {
            for entry in row {
                *entry >>= 2;
            }
        }
    }

    /// Reset all entries
    pub fn reset(&mut self) {
        self.entries = [[0; 6]; 6];
    }
}

/// Tables used during search (TT, killers, history, counter moves)
pub struct SearchTables {
    /// Shared transposition table (thread-safe, can be shared across workers)
    pub tt: Arc<TranspositionTable>,
    /// Shared pawn hash table (thread-safe, can be shared across workers)
    pub pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
    /// Per-thread killer move table
    pub killer_moves: KillerTable,
    /// Per-thread history heuristic table
    pub history: HistoryTable,
    /// Per-thread counter move table
    pub counter_moves: CounterMoveTable,
    /// Per-thread continuation history table
    pub continuation_history: ContinuationHistory,
    /// Per-thread countermove history table (response to opponent's move)
    pub countermove_history: CountermoveHistory,
    /// Per-thread capture history table
    pub capture_history: CaptureHistory,
}

impl SearchTables {
    /// MVV-LVA score for a capture move, enhanced with capture history
    /// Prioritizes capturing high-value pieces with low-value attackers,
    /// with capture history as a secondary factor
    #[must_use]
    pub fn mvv_lva_score(&self, board: &Board, mv: &Move) -> i32 {
        if !mv.is_capture() {
            return 0;
        }

        // Get attacker piece
        let Some((_, attacker_piece)) = board.piece_at(mv.from()) else {
            return 0;
        };
        let attacker = move_order::piece_value(attacker_piece);

        // For en passant, captured piece is always a pawn
        if mv.is_en_passant() {
            let mvv_lva = move_order::piece_value(Piece::Pawn) * 10 - attacker;
            let see_score = board.see(mv.from(), mv.to()) / 10;
            let cap_hist = self.capture_history.score(attacker_piece, Piece::Pawn) / 100;
            return mvv_lva + see_score + cap_hist;
        }

        // Look up what piece is on the target square
        let Some((_, victim_piece)) = board.piece_at(mv.to()) else {
            return 0;
        };
        let captured = move_order::piece_value(victim_piece);

        // MVV-LVA: prioritize high-value victims captured by low-value attackers
        let mvv_lva = captured * 10 - attacker;

        // Add SEE as a factor for more accurate ordering
        // SEE is scaled down so it doesn't dominate MVV-LVA
        let see_score = board.see(mv.from(), mv.to()) / 10;

        // Add capture history as a tie-breaker
        let cap_hist = self.capture_history.score(attacker_piece, victim_piece) / 100;

        mvv_lva + see_score + cap_hist
    }

    /// Get history score for a move
    #[must_use]
    pub fn history_score(&self, mv: &Move) -> i32 {
        self.history.score(mv)
    }

    /// Update history on beta cutoff with gravity
    pub fn update_history(&mut self, mv: &Move, depth: u32, ply: usize) {
        self.history.update(mv, depth, ply);
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
                pawn_hash: Arc::new(crate::pawn_hash::PawnHashTable::default()),
                killer_moves: KillerTable::new(),
                history: HistoryTable::new(),
                counter_moves: CounterMoveTable::new(),
                continuation_history: ContinuationHistory::new(),
                countermove_history: CountermoveHistory::new(),
                capture_history: CaptureHistory::new(),
            },
            generation: 0,
            last_move: super::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            trace: false,
        }
    }

    /// Create a new `SearchState` with a shared transposition table.
    /// Used for SMP workers that share a TT but have separate local tables.
    #[must_use]
    pub fn with_shared_tt(
        tt: Arc<TranspositionTable>,
        pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
        generation: u16,
    ) -> Self {
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
                pawn_hash,
                killer_moves: KillerTable::new(),
                history: HistoryTable::new(),
                counter_moves: CounterMoveTable::new(),
                continuation_history: ContinuationHistory::new(),
                countermove_history: CountermoveHistory::new(),
                capture_history: CaptureHistory::new(),
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

    /// Get a clone of the shared pawn hash table Arc for use by SMP workers
    #[must_use]
    pub fn shared_pawn_hash(&self) -> Arc<crate::pawn_hash::PawnHashTable> {
        Arc::clone(&self.tables.pawn_hash)
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.stats.reset_search();
        self.last_move = super::EMPTY_MOVE;
        self.hard_stop_at = None;
        // Decay history and clear tactical helpers to avoid stale biases.
        self.tables.history.decay();
        self.tables.continuation_history.decay();
        self.tables.countermove_history.decay();
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

    /// Create a config from `SearchLimits`
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
    /// Which PV line this is (1 = best, 2 = second best, etc.)
    /// Currently always 1 - full `MultiPV` is not yet implemented.
    pub multipv: u32,
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
#[allow(clippy::needless_pass_by_value)] // Config is intentionally consumed
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
