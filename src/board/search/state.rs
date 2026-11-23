use std::sync::Arc;
use std::time::Instant;

use crate::tt::TranspositionTable;

use super::constants::SCORE_SAFE_MAX;
use super::{SearchParams, SearchTables, DEFAULT_TT_MB};
use crate::board::nnue::NnueNetwork;
use crate::board::{Board, Move};
use crate::eval_math::{blended_eval, scaled_eval};

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
    #[cfg(feature = "syzygy")]
    pub(super) syzygy: Option<Arc<super::syzygy::Tablebases>>,
}

impl SearchState {
    #[must_use]
    pub fn new(tt_mb: usize) -> Self {
        Self::from_tables(SearchTables::new(tt_mb), 0)
    }

    fn from_tables(tables: SearchTables, generation: u16) -> Self {
        Self {
            stats: SearchStats::default(),
            tables,
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
            #[cfg(feature = "syzygy")]
            syzygy: None,
        }
    }

    /// Evaluation defaults for the standalone engine.
    pub(crate) fn for_play(tt_mb: usize) -> Self {
        let state = Self::new(tt_mb);
        #[cfg(feature = "embedded_nnue")]
        let state = {
            let mut state = state;
            state.tables.nnue = Some(NnueNetwork::shared_embedded());
            state.nnue_hce_blend = 40;
            state.hce_options.use_full = false;
            state.static_eval_options.nnue_pure = true;
            state
        };
        state
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
        Self::from_tables(
            SearchTables::with_shared(tt, pawn_hash, nnue, static_nnue),
            generation,
        )
    }

    /// Enable tablebase probing from a Fathom search path, or disable it
    /// with an empty path. A rejected path leaves the previous setting active.
    #[cfg(feature = "syzygy")]
    pub fn set_syzygy_path(&mut self, path: &str) -> Result<(), &'static str> {
        let result = if path.is_empty() {
            self.syzygy = None;
            Ok(())
        } else if let Some(tables) = &mut self.syzygy {
            Arc::get_mut(tables)
                .ok_or("Syzygy tables are in use")?
                .reload(path)
        } else {
            self.syzygy = Some(Arc::new(super::syzygy::Tablebases::open(path)?));
            Ok(())
        };
        self.clear_evaluation_cache();
        result
    }

    /// Load NNUE network from file
    pub fn load_nnue<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let network = NnueNetwork::load(path)?;
        self.tables.nnue = Some(Arc::new(network));
        self.clear_evaluation_cache();
        Ok(())
    }

    /// Load static-eval NNUE network from file
    pub fn load_static_nnue<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let network = NnueNetwork::load(path)?;
        self.tables.static_nnue = Some(Arc::new(network));
        self.clear_evaluation_cache();
        Ok(())
    }

    /// Discard scores learned with the previous evaluation configuration.
    /// Call this after changing evaluation settings directly.
    pub fn clear_evaluation_cache(&mut self) {
        self.clear_tt();
        self.tables.correction_history.reset();
    }

    fn clear_tt(&mut self) {
        if let Some(tt) = Arc::get_mut(&mut self.tables.tt) {
            tt.clear_exclusive();
        } else {
            self.tables.tt.clear();
        }
    }

    #[inline]
    fn hce_eval(&self, board: &Board, full: bool) -> i32 {
        if !full {
            board.evaluate_simple()
        } else if self.hce_options.use_tuned {
            board.evaluate_tuned_hce_cached(&self.tables.pawn_hash)
        } else {
            board.evaluate_cached(&self.tables.pawn_hash)
        }
    }

    pub(super) fn active_main_nnue(&self) -> Option<&NnueNetwork> {
        self.tables
            .nnue
            .as_deref()
            .filter(|_| self.nnue_hce_blend > 0)
    }

    pub(super) fn active_static_nnue(&self) -> Option<&NnueNetwork> {
        self.tables
            .static_nnue
            .as_deref()
            .filter(|_| self.nnue_static_blend > 0)
    }

    pub(super) fn uses_main_evaluator_for_pruning(&self) -> bool {
        // A loaded static file owns this preference even at a zero blend.
        self.static_eval_options.nnue_pure
            && self.tables.static_nnue.is_none()
            && self.tables.nnue.is_some()
    }

    #[inline]
    fn evaluate_blend(
        &self,
        board: &Board,
        full_hce: bool,
        network: Option<&NnueNetwork>,
        scale: i32,
        blend: i32,
        nnue_score: impl FnOnce(&NnueNetwork) -> i32,
    ) -> i32 {
        let hce = || self.hce_eval(board, full_hce);
        let score = if let Some(network) = network {
            let nnue = scaled_eval(nnue_score(network), scale);
            if blend >= 100 {
                nnue
            } else {
                blended_eval(nnue, hce(), blend)
            }
        } else {
            hce()
        };
        score.clamp(-SCORE_SAFE_MAX, SCORE_SAFE_MAX)
    }

    #[inline]
    pub(super) fn evaluate_main(
        &self,
        board: &Board,
        nnue_score: impl FnOnce(&NnueNetwork) -> i32,
    ) -> i32 {
        self.evaluate_blend(
            board,
            self.hce_options.use_full,
            self.active_main_nnue(),
            self.nnue_eval_scale,
            self.nnue_hce_blend,
            nnue_score,
        )
    }

    #[inline]
    pub(super) fn evaluate_for_pruning(
        &self,
        board: &Board,
        main_nnue_score: impl FnOnce(&NnueNetwork) -> i32,
        static_nnue_score: impl FnOnce(&NnueNetwork) -> i32,
    ) -> i32 {
        if self.uses_main_evaluator_for_pruning() {
            self.evaluate_main(board, main_nnue_score)
        } else {
            self.evaluate_blend(
                board,
                self.static_eval_options.use_full_hce,
                self.active_static_nnue(),
                self.nnue_static_eval_scale,
                self.nnue_static_blend,
                static_nnue_score,
            )
        }
    }

    /// Search's raw static score, recomputing NNUE accumulators from the board.
    pub(crate) fn evaluate_static_position(&self, board: &Board) -> i32 {
        self.evaluate_for_pruning(
            board,
            |network| board.evaluate_nnue(network),
            |network| board.evaluate_nnue(network),
        )
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
        self.prepare_search();
    }

    pub(crate) fn prepare_search(&mut self) {
        self.reset_search_state();
        self.tables.history.decay();
        self.tables.continuation_history.decay();
        self.tables.countermove_history.decay();
    }

    fn reset_search_state(&mut self) {
        self.stats.reset_search();
        self.last_move = crate::board::EMPTY_MOVE;
        self.hard_stop_at = None;
        self.tables.killer_moves.reset();
        self.tables.counter_moves.reset();
    }

    /// Reset all state that must not leak between independent games.
    ///
    /// Unlike `new_search`, this clears the transposition table and every
    /// learned heuristic table so back-to-back games are reproducible and a
    /// previous game's corrections cannot bias the new game's evaluations.
    pub fn new_game(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.reset_search_state();
        self.clear_tt();
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
