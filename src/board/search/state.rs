use std::sync::Arc;
use std::time::Instant;

use crate::board::nnue::NnueNetwork;
use crate::tt::TranspositionTable;

use super::{SearchParams, SearchTables, DEFAULT_TT_MB};
use crate::board::Move;

/// Statistics tracked during search
#[derive(Default)]
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

/// Configuration for the full position evaluation used by the main search.
#[derive(Clone, Copy)]
pub struct HceOptions {
    pub use_full: bool,
    pub use_tuned: bool,
}

impl Default for HceOptions {
    fn default() -> Self {
        Self {
            use_full: true,
            use_tuned: true,
        }
    }
}

/// Configuration for the static evaluation used by pruning and quiescence.
#[derive(Clone, Copy, Default)]
pub struct StaticEvalOptions {
    pub nnue_pure: bool,
    pub use_full_hce: bool,
}

/// Search state persisted across searches
pub struct SearchState {
    pub stats: SearchStats,
    pub tables: SearchTables,
    pub generation: u16,
    pub last_move: Move,
    pub hard_stop_at: Option<Instant>,
    pub params: SearchParams,
    pub nnue_eval_scale: i32,
    pub nnue_hce_blend: i32,
    pub hce_options: HceOptions,
    pub static_eval_options: StaticEvalOptions,
    pub nnue_static_eval_scale: i32,
    pub nnue_static_blend: i32,
    pub trace: bool,
}

impl SearchState {
    #[must_use]
    pub fn new(tt_mb: usize) -> Self {
        SearchState {
            stats: SearchStats::default(),
            tables: SearchTables::new(tt_mb),
            generation: 0,
            last_move: crate::board::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            nnue_eval_scale: 100,
            nnue_hce_blend: 100,
            hce_options: HceOptions::default(),
            static_eval_options: StaticEvalOptions::default(),
            nnue_static_eval_scale: 100,
            nnue_static_blend: 100,
            trace: false,
        }
    }

    /// Create a new `SearchState` with shared tables.
    /// Used for SMP workers that share TT, pawn hash, and NNUE but have separate local tables.
    #[must_use]
    pub fn with_shared_tables(
        tt: Arc<TranspositionTable>,
        pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
        nnue: Option<Arc<NnueNetwork>>,
        static_nnue: Option<Arc<NnueNetwork>>,
        generation: u16,
    ) -> Self {
        SearchState {
            stats: SearchStats::default(),
            tables: SearchTables::with_shared(tt, pawn_hash, nnue, static_nnue),
            generation,
            last_move: crate::board::EMPTY_MOVE,
            hard_stop_at: None,
            params: SearchParams::default(),
            nnue_eval_scale: 100,
            nnue_hce_blend: 100,
            hce_options: HceOptions::default(),
            static_eval_options: StaticEvalOptions::default(),
            nnue_static_eval_scale: 100,
            nnue_static_blend: 100,
            trace: false,
        }
    }

    /// Load NNUE network from file
    pub fn load_nnue<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let network = NnueNetwork::load(path)?;
        self.tables.nnue = Some(Arc::new(network));
        Ok(())
    }

    /// Load static-eval NNUE network from file
    pub fn load_static_nnue<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let network = NnueNetwork::load(path)?;
        self.tables.static_nnue = Some(Arc::new(network));
        Ok(())
    }

    /// Get a clone of the shared NNUE network Arc for use by SMP workers
    #[must_use]
    pub fn shared_nnue(&self) -> Option<Arc<NnueNetwork>> {
        self.tables.nnue.clone()
    }

    /// Get a clone of the shared static NNUE network Arc for use by SMP workers
    #[must_use]
    pub fn shared_static_nnue(&self) -> Option<Arc<NnueNetwork>> {
        self.tables.static_nnue.clone()
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
        self.last_move = crate::board::EMPTY_MOVE;
        self.hard_stop_at = None;
        self.tables.history.decay();
        self.tables.continuation_history.decay();
        self.tables.countermove_history.decay();
        self.tables.killer_moves.reset();
        self.tables.counter_moves.reset();
    }

    /// Reset all state that must not leak between independent games.
    ///
    /// Unlike `new_search`, this clears the transposition table and every
    /// learned heuristic table so back-to-back games are reproducible and a
    /// previous game's corrections cannot bias the new game's evaluations.
    pub fn new_game(&mut self) {
        self.new_search();
        self.tables.tt.clear();
        self.tables.history.reset();
        self.tables.continuation_history.reset();
        self.tables.countermove_history.reset();
        self.tables.capture_history.reset();
        self.tables.correction_history.reset();
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
