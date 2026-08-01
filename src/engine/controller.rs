//! Engine controller implementation.

use std::sync::Arc;

use parking_lot::Mutex;

mod search_lifecycle;

use super::job::SearchJob;
use crate::board::{Board, SearchInfoCallback, SearchState};

/// Search parameters for starting a new search
#[derive(Default)]
pub struct SearchParams {
    /// Maximum depth to search (None = unlimited)
    pub depth: Option<u32>,
    /// Soft time limit in milliseconds
    pub soft_time_ms: u64,
    /// Hard time limit in milliseconds
    pub hard_time_ms: u64,
    /// Whether to ponder (think on opponent's time)
    pub ponder: bool,
    /// Whether to search infinitely
    pub infinite: bool,
    /// Number of principal variations to search (1 = normal, >1 = `MultiPV`)
    pub multi_pv: u32,
}

/// Engine controller managing search and game state
pub struct EngineController {
    /// Current board position
    board: Board,
    /// Search state (transposition table, killers, etc.)
    search_state: Arc<Mutex<SearchState>>,
    /// Active search job (if any)
    current_job: Option<SearchJob>,
    /// Optional callback for per-iteration search info
    info_callback: Option<SearchInfoCallback>,
    /// Number of search threads for SMP (1 = single-threaded)
    num_threads: usize,
}

impl EngineController {
    /// Create a new engine controller
    #[must_use]
    pub fn new(tt_mb: usize) -> Self {
        EngineController {
            board: Board::new(),
            search_state: Arc::new(Mutex::new(SearchState::new(tt_mb))),
            current_job: None,
            info_callback: None,
            num_threads: 1,
        }
    }

    /// Load NNUE network from file
    pub fn load_nnue<P: AsRef<std::path::Path>>(&mut self, path: P) -> std::io::Result<()> {
        let mut state = self.search_state.lock();
        state.load_nnue(path)
    }

    /// Set the number of search threads for SMP
    pub fn set_threads(&mut self, num_threads: usize) {
        self.num_threads = num_threads.max(1);
    }

    /// Get current thread count
    #[must_use]
    pub fn num_threads(&self) -> usize {
        self.num_threads
    }

    /// Get a reference to the current board
    #[must_use]
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Get a mutable reference to the current board
    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    /// Set the board position
    pub fn set_board(&mut self, board: Board) {
        self.stop_search();
        self.board = board;
    }

    /// Get a reference to the search state
    #[must_use]
    pub fn search_state(&self) -> &Arc<Mutex<SearchState>> {
        &self.search_state
    }

    /// Reset the board to starting position
    pub fn new_game(&mut self) {
        self.stop_search();
        self.board = Board::new();
        let mut state = self.search_state.lock();
        state.new_search();
    }

    /// Stop any active search
    pub fn stop_search(&mut self) {
        if let Some(job) = self.current_job.take() {
            job.stop_and_wait();
        }
    }

    /// Signal stop to active search (non-blocking)
    pub fn signal_stop(&mut self) {
        if let Some(job) = &self.current_job {
            job.signal_stop();
        }
    }

    /// Handle ponderhit
    pub fn ponderhit(&mut self) {
        if let Some(job) = &mut self.current_job {
            job.ponderhit();
        }
    }

    /// Check if there's an active search
    #[must_use]
    pub fn is_searching(&self) -> bool {
        self.current_job
            .as_ref()
            .is_some_and(|job| !job.is_finished())
    }

    /// Execute a closure with mutable access to the search state.
    ///
    /// Returns `Some(R)` if the lock was acquired, `None` if poisoned.
    pub fn with_search_state<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut SearchState) -> R,
    {
        Some(f(&mut self.search_state.lock()))
    }

    /// Execute a closure with immutable access to the search state.
    pub fn with_search_state_ref<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&SearchState) -> R,
    {
        Some(f(&self.search_state.lock()))
    }

    /// Resize the transposition table
    pub fn resize_hash(&mut self, mb: usize) {
        self.stop_search();
        self.with_search_state(|state| state.reset_tables(mb));
    }

    /// Set trace/debug mode
    pub fn set_trace(&mut self, trace: bool) {
        self.with_search_state(|state| state.set_trace(trace));
    }

    /// Set maximum nodes for search
    pub fn set_max_nodes(&mut self, nodes: u64) {
        self.with_search_state(|state| state.set_max_nodes(nodes));
    }

    /// Set callback for iteration info reporting.
    pub fn set_info_callback(&mut self, cb: Option<SearchInfoCallback>) {
        self.info_callback = cb;
    }
}
