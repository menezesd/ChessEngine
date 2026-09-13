use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::Mutex;

use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::board::{Board, Color, Move, SearchResult, SearchState, DEFAULT_TT_MB};

/// Ponder state for background thinking.
pub(super) struct PonderState {
    /// Stop flag for the ponder search.
    pub(super) stop: Arc<AtomicBool>,
    /// Handle to the ponder thread.
    pub(super) handle: JoinHandle<SearchResult>,
}

/// `XBoard` protocol handler state.
#[allow(clippy::struct_excessive_bools)]
pub struct XBoardHandler {
    pub(super) board: Board,
    pub(super) state: Arc<Mutex<SearchState>>,
    pub(super) force_mode: bool,
    pub(super) engine_color: Option<Color>,
    pub(super) post_thinking: Arc<AtomicBool>,
    pub(super) pondering_enabled: bool,
    pub(super) max_depth: u32,
    pub(super) time_per_move_sec: Option<u32>,
    /// None means no clock has been supplied; Some(0) is an expired clock.
    pub(super) engine_time_cs: Option<u64>,
    pub(super) opponent_time_cs: u64,
    pub(super) moves_per_session: u32,
    pub(super) level_start_fullmove: u32,
    pub(super) base_time_sec: u32,
    pub(super) increment_sec: u32,
    pub(super) stop_flag: Arc<AtomicBool>,
    /// Foreground move search, joined by the protocol thread on completion.
    pub(super) thinking: Option<JoinHandle<SearchResult>>,
    pub(super) move_history: Vec<(Move, crate::board::UnmakeInfo)>,
    pub(super) opponent_name: Option<String>,
    pub(super) ponder: Option<PonderState>,
    pub(super) edit_mode: bool,
    pub(super) edit_white_pieces: bool,
    pub(super) analyze_mode: bool,
    pub(super) analyze_handle: Option<(Arc<AtomicBool>, JoinHandle<SearchResult>)>,
    pub(super) paused: bool,
}

impl Default for XBoardHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl XBoardHandler {
    /// Create a new `XBoard` handler.
    #[must_use]
    pub fn new() -> Self {
        XBoardHandler {
            board: Board::new(),
            state: Arc::new(Mutex::new(SearchState::new(DEFAULT_TT_MB))),
            force_mode: false,
            engine_color: None,
            post_thinking: Arc::new(AtomicBool::new(false)),
            pondering_enabled: false,
            max_depth: DEFAULT_MAX_DEPTH,
            time_per_move_sec: None,
            engine_time_cs: None,
            opponent_time_cs: 0,
            moves_per_session: 40,
            level_start_fullmove: 0,
            base_time_sec: 300,
            increment_sec: 0,
            stop_flag: Arc::new(AtomicBool::new(false)),
            thinking: None,
            move_history: Vec::new(),
            opponent_name: None,
            ponder: None,
            edit_mode: false,
            edit_white_pieces: true,
            analyze_mode: false,
            analyze_handle: None,
            paused: false,
        }
    }
}

impl Drop for XBoardHandler {
    fn drop(&mut self) {
        self.stop_thinking();
        self.stop_ponder();
        self.stop_analyze();
    }
}
