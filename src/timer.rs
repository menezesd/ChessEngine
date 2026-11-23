//! Deadline arithmetic and interruptible watchdogs for search time management.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const HARD_STOP_MARGIN_MS: u64 = 5;
const HARD_STOP_RESERVE_DIVISOR: u64 = 4;
const TIMER_POLL_MS: u64 = 5;

/// Return a deadline `millis` after `start`.
///
/// `u64::MAX` represents an unlimited deadline, and arithmetic overflow is
/// treated as no finite deadline.
#[must_use]
pub fn deadline_after_ms(start: Instant, millis: u64) -> Option<Instant> {
    if millis == u64::MAX {
        None
    } else {
        start.checked_add(Duration::from_millis(millis))
    }
}

/// Use the same budget interpretation for a new search and a ponderhit.
pub(crate) fn search_deadlines(
    start: Instant,
    soft_ms: u64,
    hard_ms: u64,
) -> (Option<Instant>, Option<Instant>) {
    let deadline = |limit: u64, margin: u64| {
        if matches!(limit, 0 | u64::MAX) {
            None
        } else {
            deadline_after_ms(start, limit.saturating_sub(margin))
        }
    };
    // A fixed five-millisecond reserve consumes the entire budget for a
    // very short search. Reserve up to a quarter of the hard limit there.
    let hard_margin = HARD_STOP_MARGIN_MS.min(hard_ms / HARD_STOP_RESERVE_DIVISOR);
    (deadline(soft_ms, 0), deadline(hard_ms, hard_margin))
}

/// Enforce a deadline without keeping a completed search's timer asleep.
pub(crate) fn spawn_stop_timer(
    deadline: Option<Instant>,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    // The search also checks its live deadline if a watchdog thread cannot
    // be created.
    deadline.and_then(|deadline| {
        thread::Builder::new()
            .spawn(move || loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
                thread::park_timeout((deadline - now).min(Duration::from_millis(TIMER_POLL_MS)));
            })
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::{deadline_after_ms, search_deadlines};
    use std::time::{Duration, Instant};

    #[test]
    fn deadline_after_ms_adds_milliseconds_to_start() {
        let start = Instant::now();

        assert_eq!(
            deadline_after_ms(start, 250).unwrap().duration_since(start),
            Duration::from_millis(250)
        );
    }

    #[test]
    fn deadline_after_ms_treats_u64_max_as_unlimited() {
        assert!(deadline_after_ms(Instant::now(), u64::MAX).is_none());
    }

    #[test]
    fn short_hard_limits_leave_time_to_search() {
        let start = Instant::now();
        for (budget, remaining) in [(1, 1), (2, 2), (5, 4), (10, 8), (20, 15), (100, 95)] {
            let (soft, hard) = search_deadlines(start, budget, budget);
            assert_eq!(soft.unwrap() - start, Duration::from_millis(budget));
            assert_eq!(hard.unwrap() - start, Duration::from_millis(remaining));
        }
    }
}
