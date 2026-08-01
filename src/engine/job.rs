use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::board::SearchClock;
use crate::timer::deadline_after_ms;

const PONDERHIT_TIMER_POLL_MS: u64 = 5;

fn deadline_for_limit(start: Instant, limit_ms: u64) -> Option<Instant> {
    (limit_ms > 0)
        .then(|| deadline_after_ms(start, limit_ms))
        .flatten()
}

/// Active search job state.
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
    /// Optional handle to the ponderhit timer thread
    ponderhit_timer_handle: Option<JoinHandle<()>>,
}

impl SearchJob {
    pub(crate) fn new(
        stop: Arc<AtomicBool>,
        clock: Arc<SearchClock>,
        pondering: Arc<AtomicBool>,
        planned_soft_time_ms: u64,
        planned_hard_time_ms: u64,
        handle: JoinHandle<()>,
        timer_handle: Option<JoinHandle<()>>,
    ) -> Self {
        SearchJob {
            stop,
            clock,
            pondering,
            planned_soft_time_ms,
            planned_hard_time_ms,
            handle,
            timer_handle,
            ponderhit_timer_handle: None,
        }
    }

    /// Stop the search and wait for the thread to finish.
    pub fn stop_and_wait(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.handle.join();
        Self::join_timer(self.timer_handle.take());
        Self::join_timer(self.ponderhit_timer_handle.take());
    }

    /// Signal stop without waiting.
    pub fn signal_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.pondering.store(false, Ordering::Relaxed);
    }

    /// Return whether the search thread has completed.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Handle ponderhit - transition from pondering to real search.
    pub fn ponderhit(&mut self) {
        if !self.pondering.load(Ordering::Relaxed) {
            return;
        }

        let start = Instant::now();
        let soft_deadline = deadline_for_limit(start, self.planned_soft_time_ms);
        let hard_deadline = deadline_for_limit(start, self.planned_hard_time_ms);
        self.clock.reset(start, soft_deadline, hard_deadline);

        // A ponder search starts without a local time limit.  Once it becomes
        // a normal search, enforce the same soft deadline used by a regular
        // search; fall back to the hard deadline if no soft deadline exists.
        if let Some(stop_deadline) = soft_deadline.or(hard_deadline) {
            let stop_timer = Arc::clone(&self.stop);
            let handle = thread::spawn(move || loop {
                if stop_timer.load(Ordering::Relaxed) {
                    break;
                }
                let now = Instant::now();
                if now >= stop_deadline {
                    stop_timer.store(true, Ordering::Relaxed);
                    break;
                }
                let sleep_for =
                    (stop_deadline - now).min(Duration::from_millis(PONDERHIT_TIMER_POLL_MS));
                thread::sleep(sleep_for);
            });
            self.ponderhit_timer_handle = Some(handle);
        }

        self.pondering.store(false, Ordering::Relaxed);
    }

    fn join_timer(timer: Option<JoinHandle<()>>) {
        if let Some(timer) = timer {
            let _ = timer.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished_job(planned_soft_time_ms: u64, planned_hard_time_ms: u64) -> SearchJob {
        let start = Instant::now();
        SearchJob::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(SearchClock::new(start, None, None)),
            Arc::new(AtomicBool::new(true)),
            planned_soft_time_ms,
            planned_hard_time_ms,
            thread::spawn(|| {}),
            None,
        )
    }

    #[test]
    fn deadline_after_treats_u64_max_as_unlimited() {
        assert!(deadline_after_ms(Instant::now(), u64::MAX).is_none());
    }

    #[test]
    fn ponderhit_with_unlimited_planned_limits_does_not_set_deadlines() {
        let mut job = finished_job(u64::MAX, u64::MAX);

        job.ponderhit();

        let (_, soft, hard) = job.clock.snapshot();
        assert!(soft.is_none());
        assert!(hard.is_none());
        assert!(!job.pondering.load(Ordering::Relaxed));

        job.stop_and_wait();
    }

    #[test]
    fn ponderhit_with_zero_planned_limits_does_not_set_deadlines() {
        let mut job = finished_job(0, 0);

        job.ponderhit();

        let (_, soft, hard) = job.clock.snapshot();
        assert!(soft.is_none());
        assert!(hard.is_none());
        assert!(!job.pondering.load(Ordering::Relaxed));

        job.stop_and_wait();
    }

    #[test]
    fn stop_and_wait_interrupts_ponderhit_timer() {
        let mut job = finished_job(1_000, 10_000);
        job.ponderhit();

        let start = Instant::now();
        job.stop_and_wait();

        assert!(start.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn ponderhit_enforces_the_soft_deadline() {
        let mut job = finished_job(20, 1_000);
        job.ponderhit();

        thread::sleep(Duration::from_millis(75));

        assert!(job.stop.load(Ordering::Relaxed));
        job.stop_and_wait();
    }

    #[test]
    fn finished_job_reports_finished() {
        let job = finished_job(0, 0);
        while !job.is_finished() {
            thread::yield_now();
        }

        assert!(job.is_finished());
        job.stop_and_wait();
    }
}
