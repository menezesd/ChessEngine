use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::board::{
    find_best_move, find_best_move_with_ponder, find_best_move_with_time_and_ponder, Move,
    SearchClock, SearchLimits, SearchResult,
};
use crate::engine::time::{TimeConfig, TimeControl};
use crate::timer::deadline_after_ms;

use super::state::{PonderState, XBoardHandler};

const CENTISECOND_MS: u64 = 10;
const XBOARD_MOVE_OVERHEAD_MS: u64 = 0;
const XBOARD_SOFT_TIME_PERCENT: u64 = 5;
const XBOARD_HARD_TIME_PERCENT: u64 = 15;
const XBOARD_DEFAULT_MAX_NODES: u64 = 0;
const HINT_DEPTH: u32 = 4;

fn duration_centiseconds_saturating(duration: Duration) -> u64 {
    let centiseconds = duration.as_millis() / u128::from(CENTISECOND_MS);
    u64::try_from(centiseconds).unwrap_or(u64::MAX)
}

fn elapsed_centiseconds_since(start: Instant) -> u64 {
    duration_centiseconds_saturating(start.elapsed())
}

impl XBoardHandler {
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

        if self.paused {
            return;
        }

        let board = self.board.clone();
        let state = Arc::clone(&self.state);
        let max_depth = self.max_depth;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let post_thinking = self.post_thinking;

        let handle = thread::spawn(move || {
            let mut board = board;
            let mut guard = state.lock();
            guard.new_search();

            for depth in 1..=max_depth {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                let start_time = Instant::now();
                let result = find_best_move(&mut board, &mut guard, depth, &stop_clone);

                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }

                if let Some(mv) = result {
                    let elapsed_cs = elapsed_centiseconds_since(start_time);
                    let nodes = guard.stats.nodes;
                    let score = 0;

                    if post_thinking {
                        let san = board.move_to_san(&mv);
                        println!("{depth} {score} {elapsed_cs} {nodes} {san}");
                    }
                }
            }
        });

        self.analyze_handle = Some((stop, handle));
    }

    /// Start pondering on the expected opponent move.
    pub(super) fn start_ponder(&mut self, ponder_move: Move) {
        self.stop_ponder();

        let mut ponder_board = self.board.clone();
        ponder_board.make_move(ponder_move);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let state_clone = Arc::clone(&self.state);
        let max_depth = self.max_depth;

        let handle = thread::spawn(move || {
            let mut guard = state_clone.lock();
            let result =
                find_best_move_with_ponder(&mut ponder_board, &mut guard, max_depth, &stop_clone);
            Some(result)
        });

        self.ponder = Some(PonderState { stop, handle });
    }

    /// Think and return the search result with best move and ponder move.
    pub(super) fn think(&mut self) -> SearchResult {
        self.stop_ponder();
        self.stop_flag.store(false, Ordering::SeqCst);

        let mut state = self.state.lock();
        let time_control = self.time_control();

        if time_control.is_unlimited() {
            find_best_move_with_ponder(&mut self.board, &mut state, self.max_depth, &self.stop_flag)
        } else {
            let limits = self.search_limits(time_control);
            find_best_move_with_time_and_ponder(&mut self.board, &mut state, &limits)
        }
    }

    pub(super) fn time_control(&self) -> TimeControl {
        if let Some(seconds) = self.time_per_move_sec {
            TimeControl::from_xboard_st(seconds)
        } else if self.engine_time_cs > 0 {
            TimeControl::from_xboard_time(
                self.engine_time_cs,
                self.increment_sec,
                if self.moves_per_session > 0 {
                    Some(self.moves_per_session)
                } else {
                    None
                },
            )
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
        let mut state = self.state.lock();
        find_best_move(&mut self.board, &mut state, HINT_DEPTH, &self.stop_flag)
    }
}

#[cfg(test)]
mod tests {
    use super::duration_centiseconds_saturating;
    use crate::timer::deadline_after_ms;
    use std::time::{Duration, Instant};

    #[test]
    fn duration_centiseconds_saturates_large_duration() {
        assert_eq!(duration_centiseconds_saturating(Duration::MAX), u64::MAX);
    }

    #[test]
    fn duration_centiseconds_converts_duration() {
        assert_eq!(
            duration_centiseconds_saturating(Duration::from_millis(1250)),
            125
        );
    }

    #[test]
    fn deadline_after_returns_none_when_duration_overflows_instant() {
        assert!(deadline_after_ms(Instant::now(), u64::MAX).is_none());
    }
}
