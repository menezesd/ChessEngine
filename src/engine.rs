use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use std::fmt;

use crate::core::board::Board;
use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::Move;
use crate::uci::info as uci_info;

/// Options that control a search invocation. Either `max_depth` or `max_time` should
/// be provided; if both are provided `max_time` takes precedence. Additional tuning
/// parameters such as `max_nodes` or `move_ordering` can be provided here.
pub struct SearchOptions {
    pub max_depth: Option<u32>,
    pub max_time: Option<Duration>,
    /// Optional node limit for the search driver. If provided, the engine will set
    /// the node limit via `search_control` before starting the search.
    pub max_nodes: Option<u64>,
    /// If true, include a ponder move string in info messages where applicable
    pub is_ponder: bool,
    /// Optional sink for intermediate best-move updates (shared Arc<Mutex<Option<Move>>>).
    pub sink: Option<Arc<Mutex<Option<Move>>>>,
    /// Optional channel to receive `uci_info::Info` progress messages.
    pub info_sender: Option<Sender<uci_info::Info>>,
    /// Optional move ordering toggle (true = enable history/killers heuristics).
    pub move_ordering: Option<bool>,
}

/// Small, serial search result returned from a search engine.
#[derive(Debug)]
pub struct SearchResult {
    pub best_move: Option<Move>,
}

/// Error type for search operations.
#[derive(Debug)]
pub enum SearchError {
    MissingOptions,
    Internal(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::MissingOptions => write!(f, "either max_time or max_depth must be specified"),
            SearchError::Internal(s) => write!(f, "internal search error: {}", s),
        }
    }
}

impl From<String> for SearchError {
    fn from(s: String) -> Self {
        SearchError::Internal(s)
    }
}

/// Simple trait that abstracts over a search implementation.
pub trait SearchEngine {
    /// Run a search on `board` using `tt` and the provided `opts`.
    fn search(&self, board: &mut Board, tt: &mut TranspositionTable, opts: SearchOptions) -> Result<SearchResult, SearchError>;
}

/// A thin concrete engine that delegates to the existing search drivers.
pub struct SimpleEngine;

impl SimpleEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SimpleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine for SimpleEngine {
    fn search(&self, board: &mut Board, tt: &mut TranspositionTable, opts: SearchOptions) -> Result<SearchResult, SearchError> {
        let start = Instant::now();

        // If a node limit is provided, configure the global search control
        if let Some(n) = opts.max_nodes {
            crate::search::control::set_node_limit(n);
        }

        // Configure move ordering heuristics globally based on option (default = enabled)
        crate::ordering::set_ordering_enabled(opts.move_ordering.unwrap_or(true));

        let best = if let Some(max_time) = opts.max_time {
            // time-limited search
            crate::search::time_limited_search_with_sink(
                board,
                tt,
                max_time,
                start,
                opts.sink.clone(),
                opts.info_sender.clone(),
                opts.is_ponder,
            )
        } else if let Some(depth) = opts.max_depth {
            // depth-limited iterative deepening
            crate::search::iterative_deepening_with_sink(
                board,
                tt,
                depth,
                opts.sink.clone(),
                opts.info_sender.clone(),
                opts.is_ponder,
            )
        } else {
            return Err(SearchError::MissingOptions);
        };

    // Null-move pruning disabled; nothing to clear here.

    // We intentionally do not expose pv/nodes/time in the return struct; they
    // are published via UCI Info messages when an `info_sender` is provided.
    let _ = start.elapsed();

    Ok(SearchResult { best_move: best })
    }
}
