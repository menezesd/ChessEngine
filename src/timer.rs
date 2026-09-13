//! Deadline arithmetic and interruptible watchdogs for search time management.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const HARD_STOP_MARGIN_MS: u64 = 5;
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
    (deadline(soft_ms, 0), deadline(hard_ms, HARD_STOP_MARGIN_MS))
}

/// Enforce a deadline without keeping a completed search's timer asleep.
pub(crate) fn spawn_stop_timer(
    deadline: Option<Instant>,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    deadline.map(|deadline| {
        thread::spawn(move || loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let now = Instant::now();
            if now >= deadline {
                stop.store(true, Ordering::Relaxed);
                break;
            }
            thread::sleep((deadline - now).min(Duration::from_millis(TIMER_POLL_MS)));
        })
    })
}

#[cfg(test)]
mod tests {
    use super::deadline_after_ms;
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
}
