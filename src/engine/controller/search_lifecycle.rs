use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::board::search::smp::{smp_search, SmpConfig};
use crate::board::search::{DEFAULT_MAX_DEPTH, SEARCH_THREAD_STACK_SIZE};
use crate::board::{search, SearchClock, SearchConfig, SearchInfoCallback, SearchResult};
use crate::timer::{search_deadlines, spawn_stop_timer};

use super::{EngineController, SearchParams};
use crate::engine::job::SearchJob;

impl EngineController {
    fn is_timed_search(params: &SearchParams) -> bool {
        !params.infinite && !params.ponder
    }

    fn should_spawn_hard_stop_timer(params: &SearchParams) -> bool {
        Self::is_timed_search(params) && !matches!(params.hard_time_ms, 0 | u64::MAX)
    }

    fn build_deadlines(
        params: &SearchParams,
        start: Instant,
    ) -> (Option<Instant>, Option<Instant>) {
        if !Self::is_timed_search(params) {
            return (None, None);
        }

        search_deadlines(start, params.soft_time_ms, params.hard_time_ms)
    }

    fn build_search_config(params: &SearchParams, node_limit: u64) -> SearchConfig {
        let mut config = if let Some(d) = params.depth {
            SearchConfig::depth(d)
        } else {
            SearchConfig::default()
        };

        if Self::is_timed_search(params) {
            config.time_limit_ms = params.soft_time_ms;
            config.hard_time_limit_ms = params.hard_time_ms;
        }
        if node_limit > 0 {
            config = config.with_nodes(node_limit);
        }
        if params.multi_pv > 1 {
            config = config.with_multi_pv(params.multi_pv);
        }
        config.root_moves.clone_from(&params.root_moves);
        config
    }

    fn wait_for_search_release(infinite: bool, pondering: &AtomicBool, stop: &AtomicBool) {
        while (infinite || pondering.load(Ordering::Relaxed)) && !stop.load(Ordering::Relaxed) {
            // SearchJob wakes this thread after changing either release flag.
            thread::park();
        }
    }

    fn gated_info_callback(&self, publish: &Arc<AtomicBool>) -> Option<SearchInfoCallback> {
        self.info_callback.as_ref().map(|callback| {
            let callback = Arc::clone(callback);
            let publish = Arc::clone(publish);
            Arc::new(move |info: &crate::board::SearchIterationInfo| {
                if publish.load(Ordering::Acquire) {
                    callback(info);
                }
            }) as SearchInfoCallback
        })
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
            guard.prepare_search();
            guard.stats.max_nodes
        };

        let stop = Arc::new(AtomicBool::new(false));
        let publish = Arc::new(AtomicBool::new(true));
        let start = Instant::now();
        let (soft_deadline, hard_deadline) = Self::build_deadlines(&params, start);

        let clock = Arc::new(SearchClock::new(start, soft_deadline, hard_deadline));
        let pondering = Arc::new(AtomicBool::new(params.ponder));

        let timer_handle = if Self::should_spawn_hard_stop_timer(&params) {
            spawn_stop_timer(hard_deadline, Arc::clone(&stop))
        } else {
            None
        };

        let mut search_board = self.board.clone();
        let search_state = Arc::clone(&self.search_state);
        let stop_clone = Arc::clone(&stop);
        let pondering_clone = Arc::clone(&pondering);
        let publish_clone = Arc::clone(&publish);
        let num_threads = self.num_threads;
        let infinite = params.infinite;
        let ponder = params.ponder;

        let mut config = Self::build_search_config(&params, node_limit);
        config.info_callback = self.gated_info_callback(&publish);
        config.clock = Some(Arc::clone(&clock));
        // MultiPV and restricted root lists use the single-threaded search.
        let use_smp = num_threads > 1 && config.multi_pv <= 1 && config.root_moves.is_none();
        let thread_name = if use_smp { "search-main" } else { "search" };
        let handle = thread::Builder::new()
            .name(thread_name.to_owned())
            .stack_size(SEARCH_THREAD_STACK_SIZE)
            .spawn(move || {
                let mut state = search_state.lock();
                let result = if use_smp {
                    let smp_config = SmpConfig {
                        num_threads,
                        max_depth: config.max_depth.unwrap_or(DEFAULT_MAX_DEPTH),
                        time_limit_ms: config.time_limit_ms,
                        hard_time_limit_ms: config.hard_time_limit_ms,
                        clock: config.clock,
                        node_limit: config.node_limit,
                        info_callback: config.info_callback,
                        ponder,
                        infinite,
                    };
                    smp_search(
                        &search_board,
                        &mut state,
                        smp_config,
                        Arc::clone(&stop_clone),
                    )
                } else {
                    search(&mut search_board, &mut state, config, &stop_clone)
                };
                drop(state);

                Self::wait_for_search_release(infinite, &pondering_clone, &stop_clone);
                stop_clone.store(true, Ordering::Relaxed);
                if publish_clone.load(Ordering::Acquire) {
                    on_complete(result);
                }
            })
            .expect("failed to spawn search thread");

        self.current_job = Some(SearchJob::new(
            stop,
            publish,
            clock,
            pondering,
            (params.soft_time_ms, params.hard_time_ms),
            handle,
            timer_handle,
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    use crate::timer::{deadline_after_ms, HARD_STOP_MARGIN_MS};

    use super::{EngineController, SearchParams};

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
    fn unlimited_hard_budget_does_not_become_a_finite_watchdog() {
        let params = SearchParams {
            soft_time_ms: u64::MAX,
            hard_time_ms: u64::MAX,
            ..SearchParams::default()
        };
        assert_eq!(
            EngineController::build_deadlines(&params, Instant::now()),
            (None, None)
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

    #[test]
    fn fixed_depth_timed_search_uses_hard_stop_watchdog() {
        let params = SearchParams {
            depth: Some(8),
            soft_time_ms: 50,
            hard_time_ms: 100,
            ..SearchParams::default()
        };

        assert!(EngineController::should_spawn_hard_stop_timer(&params));
    }

    #[test]
    fn completion_callback_can_read_the_finished_search_state() {
        for threads in [1, 2] {
            let (sender, receiver) = mpsc::channel();
            let mut controller = EngineController::new(1);
            controller.set_threads(threads);
            let state = Arc::clone(controller.search_state());
            let next_generation = state.lock().generation.wrapping_add(1);
            controller.start_search(
                SearchParams {
                    depth: Some(1),
                    ..SearchParams::default()
                },
                move |_| {
                    // A real callback may lock this state to read statistics.
                    // Use try_lock here so a regression fails without hanging.
                    sender
                        .send(state.try_lock().map(|state| state.generation))
                        .unwrap();
                },
            );

            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
                Some(next_generation),
                "expected one generation advance and an unlocked state ({threads} threads)"
            );
            controller.stop_search();
        }
    }

    #[test]
    fn state_setters_stop_the_search_before_waiting_for_its_lock() {
        for load_network in [false, true] {
            let mut controller = EngineController::new(1);
            let (entered_tx, entered_rx) = mpsc::channel();
            let release = Arc::new(std::sync::Barrier::new(2));
            let search_release = Arc::clone(&release);
            controller.set_info_callback(Some(Arc::new(move |_| {
                entered_tx.send(()).unwrap();
                search_release.wait();
            })));
            controller.start_search(
                SearchParams {
                    depth: Some(1),
                    ..SearchParams::default()
                },
                |_| {},
            );
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let stop = Arc::clone(&controller.current_job.as_ref().unwrap().stop);
            let setter = std::thread::spawn(move || {
                if load_network {
                    assert!(controller
                        .load_nnue("/missing/chess-engine-network.nnue")
                        .is_err());
                } else {
                    controller.set_max_nodes(123);
                    assert_eq!(controller.search_state.lock().stats.max_nodes, 123);
                }
                controller.stop_search();
            });

            let deadline = Instant::now() + Duration::from_secs(1);
            while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            let stopped = stop.load(Ordering::Relaxed);
            release.wait();
            setter.join().unwrap();
            assert!(stopped, "setter blocked without stopping the active search");
        }
    }

    #[test]
    fn dropping_a_controller_cancels_and_joins_its_search() {
        for threads in [1, 2] {
            let mut controller = EngineController::new(1);
            controller.set_threads(threads);
            let (sender, receiver) = mpsc::channel();
            controller.start_search(
                SearchParams {
                    depth: Some(1),
                    infinite: true,
                    ..SearchParams::default()
                },
                move |_| {
                    sender.send(()).unwrap();
                },
            );
            let stop = Arc::clone(&controller.current_job.as_ref().unwrap().stop);

            drop(controller);
            assert!(
                stop.load(Ordering::Relaxed),
                "dropping the controller did not stop its worker"
            );
            assert!(
                matches!(
                    receiver.recv_timeout(Duration::from_millis(100)),
                    Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected)
                ),
                "dropping the controller published a cancelled result"
            );
        }
    }

    #[test]
    fn mutating_the_board_cancels_an_active_search_without_publishing() {
        let mut controller = EngineController::new(1);
        let (sender, receiver) = mpsc::channel();
        controller.start_search(
            SearchParams {
                infinite: true,
                ..SearchParams::default()
            },
            move |_| sender.send(()).unwrap(),
        );

        controller.board_mut().make_move_uci("e2e4").unwrap();

        assert!(!controller.is_searching());
        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn ponderhit_re_times_a_running_ponder_search() {
        let (sender, receiver) = mpsc::channel();
        let mut controller = EngineController::new(1);
        controller.start_search(
            SearchParams {
                soft_time_ms: 60,
                hard_time_ms: 120,
                ponder: true,
                ..SearchParams::default()
            },
            move |_| sender.send(()).expect("test receiver should remain alive"),
        );

        std::thread::sleep(Duration::from_millis(50));
        controller.ponderhit();

        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("search should finish shortly after ponderhit installs deadlines");
        controller.stop_search();
    }

    #[test]
    fn completed_search_stops_its_hard_stop_watchdog() {
        let (sender, receiver) = mpsc::channel();
        let mut controller = EngineController::new(1);
        controller.start_search(
            SearchParams {
                depth: Some(1),
                hard_time_ms: 10_000,
                ..SearchParams::default()
            },
            move |_| sender.send(()).expect("test receiver should remain alive"),
        );

        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("depth-one search should complete promptly");

        assert!(controller
            .current_job
            .as_ref()
            .expect("completed job should still be tracked")
            .stop
            .load(std::sync::atomic::Ordering::Relaxed));
        controller.stop_search();
    }

    #[test]
    fn infinite_search_waits_for_stop_after_early_completion() {
        // Cover both the terminal preflight and a completed tree search.
        // The latter also exercises SMP's internal completion signal.
        for threads in [1, 2] {
            for fen in [
                "7k/8/8/8/8/8/8/K7 w - - 0 1",
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            ] {
                let (sender, receiver) = mpsc::channel();
                let mut controller = EngineController::new(1);
                controller.set_threads(threads);
                controller.set_board(crate::board::Board::from_fen(fen));
                controller.start_search(
                    SearchParams {
                        depth: Some(1),
                        infinite: true,
                        ..SearchParams::default()
                    },
                    move |result| sender.send(result).unwrap(),
                );

                // Even a spurious ponderhit must not release an infinite search.
                controller.ponderhit();
                let early_result = receiver.recv_timeout(Duration::from_millis(100));
                controller.signal_stop();
                let result = receiver.recv_timeout(Duration::from_secs(2));
                controller.stop_search();

                assert!(matches!(early_result, Err(mpsc::RecvTimeoutError::Timeout)));
                let best = result
                    .expect("stop should release the result")
                    .best_move
                    .unwrap();
                assert!(controller.board_mut().is_legal_move(best));
                assert!(
                    receiver.try_recv().is_err(),
                    "published more than one result"
                );
            }
        }
    }
}
