//! Engine controller implementation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::board::search::smp::{smp_search, SmpConfig};
use crate::board::{
    search, Board, SearchClock, SearchConfig, SearchInfoCallback, SearchResult, SearchState,
};

/// Search thread stack size (32 MB)
const SEARCH_STACK_SIZE: usize = 32 * 1024 * 1024;
const HARD_STOP_MARGIN_MS: u64 = 5;

/// Maximum sleep duration when polling time limits (avoids excessive CPU wake-ups)
const MAX_POLL_SLEEP_MS: u64 = 5;

/// Poll interval when waiting for ponder to complete
const PONDER_POLL_MS: u64 = 10;

/// Active search job state
pub struct SearchJob {
    /// Stop flag for the search
    pub stop: Arc<AtomicBool>,
    /// Clock for time management
    pub clock: Arc<SearchClock>,
    /// Whether we're currently pondering
    pub pondering: Arc<AtomicBool>,
    /// Planned soft time limit (for ponderhit)
    pub planned_soft_time_ms: u64,
    /// Planned hard time limit (for ponderhit)
    pub planned_hard_time_ms: u64,
    /// Handle to the search thread
    handle: JoinHandle<()>,
    /// Optional handle to the timer thread enforcing hard stops
    timer_handle: Option<JoinHandle<()>>,
}

impl SearchJob {
    /// Stop the search and wait for the thread to finish
    pub fn stop_and_wait(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.handle.join();
        if let Some(timer) = self.timer_handle {
            let _ = timer.join();
        }
    }

    /// Signal stop without waiting
    pub fn signal_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.pondering.store(false, Ordering::Relaxed);
    }

    /// Handle ponderhit - transition from pondering to real search
    pub fn ponderhit(&self) {
        if self.pondering.load(Ordering::Relaxed) {
            let start = Instant::now();
            let hard_deadline = start + Duration::from_millis(self.planned_hard_time_ms);
            self.clock.reset(
                start,
                Some(start + Duration::from_millis(self.planned_soft_time_ms)),
                Some(hard_deadline),
            );

            // Spawn timer thread to enforce hard deadline
            let stop_timer = Arc::clone(&self.stop);
            thread::spawn(move || {
                let now = Instant::now();
                if hard_deadline > now {
                    thread::sleep(hard_deadline - now);
                }
                stop_timer.store(true, Ordering::Relaxed);
            });

            self.pondering.store(false, Ordering::Relaxed);
        }
    }
}

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

/// Default NNUE file paths to try loading (used when embedded_nnue is disabled)
#[cfg(not(feature = "embedded_nnue"))]
const DEFAULT_NNUE_PATHS: &[&str] = &["trained_new_combined.nnue", "trained.nnue", "default.nnue"];

impl EngineController {
    /// Create a new engine controller
    #[must_use]
    pub fn new(tt_mb: usize) -> Self {
        let mut controller = EngineController {
            board: Board::new(),
            search_state: Arc::new(Mutex::new(SearchState::new(tt_mb))),
            current_job: None,
            info_callback: None,
            num_threads: 1,
        };

        // Try to auto-load a default NNUE file
        controller.try_load_default_nnue();

        controller
    }

    /// Try to load a default NNUE file from common paths or embedded
    fn try_load_default_nnue(&mut self) {
        // First try embedded NNUE (if compiled in)
        #[cfg(feature = "embedded_nnue")]
        {
            use crate::board::nnue::NnueNetwork;
            let network = NnueNetwork::from_embedded();
            let mut state = self.search_state.lock();
            state.tables.nnue = Some(std::sync::Arc::new(network));
            eprintln!("info string Using embedded NNUE");
        }

        // Fall back to loading from file
        #[cfg(not(feature = "embedded_nnue"))]
        for path in DEFAULT_NNUE_PATHS {
            if std::path::Path::new(path).exists() {
                if self.load_nnue(path).is_ok() {
                    eprintln!("info string Loaded NNUE: {}", path);
                    return;
                }
            }
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
        if let Some(job) = &self.current_job {
            job.ponderhit();
        }
    }

    /// Check if there's an active search
    #[must_use]
    pub fn is_searching(&self) -> bool {
        self.current_job.is_some()
    }

    fn build_deadlines(
        params: &SearchParams,
        start: Instant,
    ) -> (Option<Instant>, Option<Instant>) {
        if params.infinite || params.ponder {
            return (None, None);
        }

        let soft_deadline = if params.soft_time_ms > 0 {
            Some(start + Duration::from_millis(params.soft_time_ms))
        } else {
            None
        };

        let hard_deadline = if params.hard_time_ms > 0 {
            Some(
                start
                    + Duration::from_millis(
                        params.hard_time_ms.saturating_sub(HARD_STOP_MARGIN_MS),
                    ),
            )
        } else {
            None
        };

        (soft_deadline, hard_deadline)
    }

    fn build_search_config(&self, params: &SearchParams, node_limit: u64) -> SearchConfig {
        let mut config = if let Some(d) = params.depth {
            SearchConfig::depth(d)
        } else {
            SearchConfig::default()
        };

        if !params.infinite && !params.ponder && params.soft_time_ms > 0 {
            config.time_limit_ms = params.soft_time_ms;
        }
        if node_limit > 0 {
            config = config.with_nodes(node_limit);
        }
        if let Some(cb) = &self.info_callback {
            config = config.with_info_callback(cb.clone());
        }
        if params.multi_pv > 1 {
            config = config.with_multi_pv(params.multi_pv);
        }
        config
    }

    fn spawn_hard_stop_timer(
        hard_deadline: Option<Instant>,
        stop: Arc<AtomicBool>,
    ) -> Option<JoinHandle<()>> {
        hard_deadline.map(|deadline| {
            thread::spawn(move || loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
                let sleep_for = (deadline - now).min(Duration::from_millis(MAX_POLL_SLEEP_MS));
                thread::sleep(sleep_for);
            })
        })
    }

    /// Start a search with the given parameters
    ///
    /// The `on_complete` callback is called when the search finishes with the result.
    #[allow(clippy::needless_pass_by_value)] // Params is small and intentionally consumed
    pub fn start_search<F>(&mut self, params: SearchParams, on_complete: F)
    where
        F: FnOnce(SearchResult) + Send + 'static,
    {
        self.stop_search();

        // Prepare search state
        let node_limit = {
            let mut guard = self.search_state.lock();
            guard.new_search();
            guard.stats.max_nodes
        };

        let stop = Arc::new(AtomicBool::new(false));
        let start = Instant::now();

        // Set deadlines based on params
        let (soft_deadline, hard_deadline) = Self::build_deadlines(&params, start);

        let clock = Arc::new(SearchClock::new(start, soft_deadline, hard_deadline));
        let pondering = Arc::new(AtomicBool::new(params.ponder));

        // Spawn timer thread for hard deadline
        let timer_handle = if !params.infinite
            && !params.ponder
            && params.depth.is_none()
            && params.hard_time_ms > 0
        {
            Self::spawn_hard_stop_timer(hard_deadline, Arc::clone(&stop))
        } else {
            None
        };

        // Clone for the search thread
        let search_board = self.board.clone();
        let search_state = Arc::clone(&self.search_state);
        let stop_clone = Arc::clone(&stop);
        let pondering_clone = Arc::clone(&pondering);
        let num_threads = self.num_threads;
        let info_callback = self.info_callback.clone();

        // Build config based on thread count
        if num_threads > 1 {
            // Use SMP search with multiple threads
            let smp_config = SmpConfig {
                num_threads,
                max_depth: params.depth.unwrap_or(64),
                time_limit_ms: if params.infinite || params.ponder {
                    0
                } else {
                    params.soft_time_ms
                },
                node_limit,
                info_callback,
            };

            let handle = thread::Builder::new()
                .name("search-main".to_string())
                .stack_size(SEARCH_STACK_SIZE)
                .spawn(move || {
                    let mut guard = search_state.lock();
                    let result =
                        smp_search(&search_board, &mut guard, smp_config, stop_clone.clone());

                    // Wait while pondering (unless stopped)
                    while pondering_clone.load(Ordering::Relaxed)
                        && !stop_clone.load(Ordering::Relaxed)
                    {
                        thread::sleep(Duration::from_millis(PONDER_POLL_MS));
                    }

                    on_complete(result);
                })
                .expect("failed to spawn search thread");

            self.current_job = Some(SearchJob {
                stop,
                clock,
                pondering,
                planned_soft_time_ms: params.soft_time_ms,
                planned_hard_time_ms: params.hard_time_ms,
                handle,
                timer_handle,
            });
        } else {
            // Single-threaded search
            let config = self.build_search_config(&params, node_limit);
            let mut search_board = search_board;

            let handle = thread::Builder::new()
                .name("search".to_string())
                .stack_size(SEARCH_STACK_SIZE)
                .spawn(move || {
                    let mut guard = search_state.lock();
                    let result: SearchResult =
                        search(&mut search_board, &mut guard, config, &stop_clone);

                    // Wait while pondering (unless stopped)
                    while pondering_clone.load(Ordering::Relaxed)
                        && !stop_clone.load(Ordering::Relaxed)
                    {
                        thread::sleep(Duration::from_millis(PONDER_POLL_MS));
                    }

                    on_complete(result);
                })
                .expect("failed to spawn search thread");

            self.current_job = Some(SearchJob {
                stop,
                clock,
                pondering,
                planned_soft_time_ms: params.soft_time_ms,
                planned_hard_time_ms: params.hard_time_ms,
                handle,
                timer_handle,
            });
        }
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
