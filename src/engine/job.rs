use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use crate::board::SearchClock;
use crate::timer::{search_deadlines, spawn_stop_timer};

/// Active search job state.
pub struct SearchJob {
    /// Stop flag for the search
    pub stop: Arc<AtomicBool>,
    /// Whether search output may still be published.
    pub publish: Arc<AtomicBool>,
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
    /// Timed searches start a watchdog immediately. Pondering starts it on hit.
    timer_handle: Option<JoinHandle<()>>,
}

impl SearchJob {
    pub(crate) fn new(
        stop: Arc<AtomicBool>,
        publish: Arc<AtomicBool>,
        clock: Arc<SearchClock>,
        pondering: Arc<AtomicBool>,
        planned_time_ms: (u64, u64),
        handle: JoinHandle<()>,
        timer_handle: Option<JoinHandle<()>>,
    ) -> Self {
        let (planned_soft_time_ms, planned_hard_time_ms) = planned_time_ms;
        SearchJob {
            stop,
            publish,
            clock,
            pondering,
            planned_soft_time_ms,
            planned_hard_time_ms,
            handle,
            timer_handle,
        }
    }

    /// Cancel the search, discard its result, and wait for the thread to finish.
    pub fn stop_and_wait(self) {
        self.publish.store(false, Ordering::Release);
        self.signal_stop();
        let _ = self.handle.join();
        if let Some(timer) = self.timer_handle {
            let _ = timer.join();
        }
    }

    /// Signal stop without waiting.
    pub fn signal_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.pondering.store(false, Ordering::Relaxed);
        self.handle.thread().unpark();
        if let Some(timer) = &self.timer_handle {
            timer.thread().unpark();
        }
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
        let (soft_deadline, hard_deadline) =
            search_deadlines(start, self.planned_soft_time_ms, self.planned_hard_time_ms);
        self.clock.reset(start, soft_deadline, hard_deadline);

        // Iteration management can spend beyond the soft target when needed.
        // The watchdog enforces the hard deadline, or a soft-only limit.
        self.timer_handle =
            spawn_stop_timer(hard_deadline.or(soft_deadline), Arc::clone(&self.stop));

        self.pondering.store(false, Ordering::Relaxed);
        self.handle.thread().unpark();
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::timer::deadline_after_ms;

    fn finished_job(planned_soft_time_ms: u64, planned_hard_time_ms: u64) -> SearchJob {
        let start = Instant::now();
        SearchJob::new(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(true)),
            Arc::new(SearchClock::new(start, None, None)),
            Arc::new(AtomicBool::new(true)),
            (planned_soft_time_ms, planned_hard_time_ms),
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
    fn ponderhit_watchdog_allows_soft_time_overrun() {
        let mut job = finished_job(20, 1_000);
        job.ponderhit();

        thread::sleep(Duration::from_millis(75));

        assert!(
            !job.stop.load(Ordering::Relaxed),
            "watchdog stopped at the soft target instead of the hard limit"
        );
        job.stop_and_wait();
    }

    #[test]
    fn ponderhit_keeps_the_normal_search_hard_stop_margin() {
        let mut job = finished_job(20, 100);
        job.ponderhit();
        let (start, soft, hard) = job.clock.snapshot();
        assert_eq!(
            soft.unwrap().duration_since(start),
            Duration::from_millis(20)
        );
        assert_eq!(
            hard.unwrap().duration_since(start),
            Duration::from_millis(95)
        );
        job.stop_and_wait();
    }

    #[test]
    fn ponderhit_watchdog_enforces_a_soft_only_limit() {
        let mut job = finished_job(20, 0);
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
