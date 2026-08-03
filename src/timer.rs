//! Deadline arithmetic for search time management.

use std::time::{Duration, Instant};

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
