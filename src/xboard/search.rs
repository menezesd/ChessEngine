use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::board::{
    find_best_move, search, Board, Move, SearchClock, SearchConfig, SearchLimits, SearchResult,
};
use crate::engine::time::{TimeConfig, TimeControl};
use crate::timer::deadline_after_ms;

use super::output::format_search_info;
use super::state::{PonderState, XBoardHandler};

const XBOARD_MOVE_OVERHEAD_MS: u64 = 0;
const XBOARD_SOFT_TIME_PERCENT: u64 = 5;
const XBOARD_HARD_TIME_PERCENT: u64 = 15;
const XBOARD_DEFAULT_MAX_NODES: u64 = 0;
const HINT_DEPTH: u32 = 4;
const SEARCH_STACK_SIZE: usize = 32 * 1024 * 1024;

impl XBoardHandler {
    /// Use the same search, stack size, and live output switch in every mode.
    fn spawn_search(
        &self,
        mut board: Board,
        mut config: SearchConfig,
        stop: Arc<AtomicBool>,
        name: &str,
    ) -> thread::JoinHandle<SearchResult> {
        let root = board.clone();
        let post = Arc::clone(&self.post_thinking);
        config.info_callback = Some(Arc::new(move |info| {
            if post.load(Ordering::Relaxed) {
                let mut stdout = io::stdout().lock();
                writeln!(stdout, "{}", format_search_info(&root, info)).ok();
                stdout.flush().ok();
            }
        }));
        let state = Arc::clone(&self.state);
        thread::Builder::new()
            .name(name.to_string())
            .stack_size(SEARCH_STACK_SIZE)
            .spawn(move || {
                let mut state = state.lock();
                state.new_search();
                search(&mut board, &mut state, config, &stop)
            })
            .expect("failed to spawn XBoard search thread")
    }

    /// Cancel a move search and discard the result before changing position.
    pub(super) fn stop_thinking(&mut self) {
        if let Some(handle) = self.thinking.take() {
            self.stop_flag.store(true, Ordering::Relaxed);
            let _ = handle.join();
        }
    }

    /// Stop any active ponder search.
    pub(super) fn stop_ponder(&mut self) {
        if let Some(ponder) = self.ponder.take() {
            ponder.stop.store(true, Ordering::Relaxed);
            let _ = ponder.handle.join();
        }
    }

    /// Stop any active analyze search.
    pub(super) fn stop_analyze(&mut self) {
        if let Some((stop, handle)) = self.analyze_handle.take() {
            stop.store(true, Ordering::Relaxed);
            let _ = handle.join();
        }
    }

    /// Start analyze mode (continuous search with output).
    pub(super) fn start_analyze(&mut self) {
        self.stop_analyze();

        if self.paused || self.edit_mode {
            return;
        }

        let stop = Arc::new(AtomicBool::new(false));
        let handle = self.spawn_search(
            self.board.clone(),
            SearchConfig::depth(self.max_depth).with_ponder(false),
            Arc::clone(&stop),
            "xboard-analysis",
        );

        self.analyze_handle = Some((stop, handle));
    }

    /// Start pondering on the expected opponent move.
    pub(super) fn start_ponder(&mut self, ponder_move: Move) {
        self.stop_ponder();

        let mut ponder_board = self.board.clone();
        ponder_board.make_move(ponder_move);

        let stop = Arc::new(AtomicBool::new(false));
        let handle = self.spawn_search(
            ponder_board,
            SearchConfig::depth(self.max_depth),
            Arc::clone(&stop),
            "xboard-ponder",
        );

        self.ponder = Some(PonderState { stop, handle });
    }

    /// Start a move search without blocking command processing.
    pub(super) fn start_thinking(&mut self) {
        if self.thinking.is_some() || !self.should_think() {
            return;
        }
        self.stop_ponder();
        self.stop_analyze();
        self.stop_flag = Arc::new(AtomicBool::new(false));

        let time_control = self.time_control();
        let mut config = if time_control.is_unlimited() {
            SearchConfig::depth(self.max_depth)
        } else {
            SearchConfig::from_limits(&self.search_limits(time_control))
        };
        // CECP depth and clock limits apply simultaneously.
        config.max_depth = Some(self.max_depth);
        self.thinking = Some(self.spawn_search(
            self.board.clone(),
            config,
            Arc::clone(&self.stop_flag),
            "xboard-search",
        ));
    }

    pub(super) fn time_control(&self) -> TimeControl {
        if let Some(seconds) = self.time_per_move_sec {
            TimeControl::from_xboard_st(seconds)
        } else if let Some(time_cs) = self.engine_time_cs {
            let moves_to_go = if self.moves_per_session == 0 {
                None
            } else {
                let played = self
                    .board
                    .fullmove_number()
                    .saturating_sub(1)
                    .saturating_sub(self.level_start_fullmove);
                Some(self.moves_per_session - played % self.moves_per_session)
            };
            TimeControl::from_xboard_time(time_cs, self.increment_sec, moves_to_go)
        } else {
            TimeControl::Depth
        }
    }

    fn xboard_time_config() -> TimeConfig {
        TimeConfig {
            move_overhead_ms: XBOARD_MOVE_OVERHEAD_MS,
            soft_time_percent: XBOARD_SOFT_TIME_PERCENT,
            hard_time_percent: XBOARD_HARD_TIME_PERCENT,
            default_max_nodes: XBOARD_DEFAULT_MAX_NODES,
        }
    }

    fn search_limits(&self, time_control: TimeControl) -> SearchLimits {
        let (soft_ms, hard_ms) = time_control.compute_limits(&Self::xboard_time_config());
        let start = Instant::now();
        let soft_deadline = deadline_after_ms(start, soft_ms);
        let hard_deadline = deadline_after_ms(start, hard_ms);
        let clock = Arc::new(SearchClock::new(start, soft_deadline, hard_deadline));
        SearchLimits {
            clock,
            stop: self.stop_flag.clone(),
        }
    }

    /// Get a hint (quick search).
    pub(super) fn get_hint(&mut self) -> Option<Move> {
        // A running move, ponder, or analysis search owns the state lock.
        let mut state = self.state.try_lock()?;
        find_best_move(
            &mut self.board,
            &mut state,
            HINT_DEPTH,
            &AtomicBool::new(false),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::timer::deadline_after_ms;
    use std::time::Instant;

    #[test]
    fn deadline_after_returns_none_when_duration_overflows_instant() {
        assert!(deadline_after_ms(Instant::now(), u64::MAX).is_none());
    }
}
