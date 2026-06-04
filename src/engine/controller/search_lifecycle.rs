use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::board::search::smp::{smp_search, SmpConfig};
use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::board::{search, SearchClock, SearchConfig, SearchResult};
use crate::timer::deadline_after_ms;

use super::{EngineController, SearchParams};
use crate::engine::job::SearchJob;

/// Search thread stack size (32 MB)
const SEARCH_STACK_SIZE: usize = 32 * 1024 * 1024;
const HARD_STOP_MARGIN_MS: u64 = 5;

/// Maximum sleep duration when polling time limits (avoids excessive CPU wake-ups)
const MAX_POLL_SLEEP_MS: u64 = 5;

/// Poll interval when waiting for ponder to complete
const PONDER_POLL_MS: u64 = 10;

impl EngineController {
    fn is_timed_search(params: &SearchParams) -> bool {
        !params.infinite && !params.ponder
    }

    fn search_time_limit_ms(params: &SearchParams) -> u64 {
        if Self::is_timed_search(params) {
            params.soft_time_ms
        } else {
            0
        }
    }

    fn should_spawn_hard_stop_timer(params: &SearchParams) -> bool {
        Self::is_timed_search(params) && params.depth.is_none() && params.hard_time_ms > 0
    }

    fn build_deadlines(
        params: &SearchParams,
        start: Instant,
    ) -> (Option<Instant>, Option<Instant>) {
        if !Self::is_timed_search(params) {
            return (None, None);
        }

        let soft_deadline = if params.soft_time_ms > 0 {
            deadline_after_ms(start, params.soft_time_ms)
        } else {
            None
        };

        let hard_deadline = if params.hard_time_ms > 0 {
            deadline_after_ms(
                start,
                params.hard_time_ms.saturating_sub(HARD_STOP_MARGIN_MS),
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

        if Self::is_timed_search(params) && params.soft_time_ms > 0 {
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

    fn wait_for_ponder_completion(pondering: &AtomicBool, stop: &AtomicBool) {
        while pondering.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(PONDER_POLL_MS));
        }
    }

    fn install_current_job(
        &mut self,
        stop: Arc<AtomicBool>,
        clock: Arc<SearchClock>,
        pondering: Arc<AtomicBool>,
        params: &SearchParams,
        handle: JoinHandle<()>,
        timer_handle: Option<JoinHandle<()>>,
    ) {
        self.current_job = Some(SearchJob::new(
            stop,
            clock,
            pondering,
            params.soft_time_ms,
            params.hard_time_ms,
            handle,
            timer_handle,
        ));
    }

    /// Start a search with the given parameters.
    ///
    /// The `on_complete` callback is called when the search finishes with the result.
    #[allow(clippy::needless_pass_by_value)] // Params is small and intentionally consumed
    pub fn start_search<F>(&mut self, params: SearchParams, on_complete: F)
    where
        F: FnOnce(SearchResult) + Send + 'static,
    {
        self.stop_search();

        let node_limit = {
            let mut guard = self.search_state.lock();
            guard.new_search();
            guard.stats.max_nodes
        };

        let stop = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let (soft_deadline, hard_deadline) = Self::build_deadlines(&params, start);

        let clock = Arc::new(SearchClock::new(start, soft_deadline, hard_deadline));
        let pondering = Arc::new(AtomicBool::new(params.ponder));

        let timer_handle = if Self::should_spawn_hard_stop_timer(&params) {
            Self::spawn_hard_stop_timer(hard_deadline, Arc::clone(&stop))
        } else {
            None
        };

        let search_board = self.board.clone();
        let search_state = Arc::clone(&self.search_state);
        let stop_clone = Arc::clone(&stop);
        let pondering_clone = Arc::clone(&pondering);
        let num_threads = self.num_threads;
        let info_callback = self.info_callback.clone();

        let handle = if num_threads > 1 {
            let smp_config = SmpConfig {
                num_threads,
                max_depth: params
                    .depth
                    .unwrap_or(DEFAULT_MAX_DEPTH)
                    .min(DEFAULT_MAX_DEPTH),
                time_limit_ms: Self::search_time_limit_ms(&params),
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

                    EngineController::wait_for_ponder_completion(&pondering_clone, &stop_clone);

                    on_complete(result);
                })
                .expect("failed to spawn search thread");
            handle
        } else {
            let config = self.build_search_config(&params, node_limit);
            let mut search_board = search_board;

            let handle = thread::Builder::new()
                .name("search".to_string())
                .stack_size(SEARCH_STACK_SIZE)
                .spawn(move || {
                    let mut guard = search_state.lock();
                    let result: SearchResult =
                        search(&mut search_board, &mut guard, config, &stop_clone);

                    EngineController::wait_for_ponder_completion(&pondering_clone, &stop_clone);

                    on_complete(result);
                })
                .expect("failed to spawn search thread");
            handle
        };

        self.install_current_job(stop, clock, pondering, &params, handle, timer_handle);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::timer::deadline_after_ms;

    use super::{EngineController, SearchParams, HARD_STOP_MARGIN_MS};

    #[test]
    fn deadline_after_adds_milliseconds_to_start() {
        let start = Instant::now();

        assert_eq!(
            deadline_after_ms(start, 250).unwrap().duration_since(start),
            Duration::from_millis(250)
        );
    }

    #[test]
    fn deadline_after_returns_none_when_duration_overflows_instant() {
        assert!(deadline_after_ms(Instant::now(), u64::MAX).is_none());
    }

    #[test]
    fn build_deadlines_uses_soft_time_and_hard_margin_for_timed_search() {
        let start = Instant::now();
        let params = SearchParams {
            soft_time_ms: 100,
            hard_time_ms: 250,
            ..SearchParams::default()
        };

        let (soft, hard) = EngineController::build_deadlines(&params, start);

        assert_eq!(
            soft.unwrap().duration_since(start),
            Duration::from_millis(100)
        );
        assert_eq!(
            hard.unwrap().duration_since(start),
            Duration::from_millis(250 - HARD_STOP_MARGIN_MS)
        );
    }

    #[test]
    fn build_deadlines_skips_ponder_search_until_ponderhit() {
        let start = Instant::now();
        let params = SearchParams {
            soft_time_ms: 100,
            hard_time_ms: 250,
            ponder: true,
            ..SearchParams::default()
        };

        assert_eq!(
            EngineController::build_deadlines(&params, start),
            (None, None)
        );
    }
}
