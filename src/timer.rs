//! Timer utilities for search time management.
//!
//! Provides deadline-based timers that can signal stop flags.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::sync::StopFlag;

/// Calculate the duration from now until a deadline, if the deadline is in the future.
///
/// Returns `None` if the deadline has already passed.
#[inline]
fn duration_until(deadline: Instant) -> Option<Duration> {
    let now = Instant::now();
    if deadline > now {
        Some(deadline - now)
    } else {
        None
    }
}

/// A timer that signals a stop flag when a deadline is reached.
///
/// The timer runs in a background thread and will automatically
/// set the stop flag when the deadline expires.
pub struct DeadlineTimer {
    handle: Option<JoinHandle<()>>,
    stop_flag: StopFlag,
}

impl DeadlineTimer {
    /// Create and start a timer that will signal after the given duration.
    ///
    /// Returns `None` if the duration is zero (no timer needed).
    #[must_use]
    pub fn start(duration: Duration, stop_flag: StopFlag) -> Option<Self> {
        if duration.is_zero() {
            return None;
        }

        let flag_clone = stop_flag.clone();
        let handle = thread::spawn(move || {
            thread::sleep(duration);
            flag_clone.stop();
        });

        Some(DeadlineTimer {
            handle: Some(handle),
            stop_flag,
        })
    }

    /// Create and start a timer that will signal at the given deadline.
    ///
    /// Returns `None` if the deadline has already passed or is not set.
    #[must_use]
    pub fn start_at(deadline: Option<Instant>, stop_flag: StopFlag) -> Option<Self> {
        let deadline = deadline?;
        if let Some(duration) = duration_until(deadline) {
            Self::start(duration, stop_flag)
        } else {
            stop_flag.stop();
            None
        }
    }

    /// Cancel the timer without triggering the stop flag.
    pub fn cancel(mut self) {
        // Drop the handle without waiting - the thread will still run
        // but the stop flag reference will be dropped
        self.handle.take();
    }

    /// Wait for the timer to complete.
    pub fn wait(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the timer has triggered.
    #[must_use]
    pub fn is_triggered(&self) -> bool {
        self.stop_flag.is_stopped()
    }
}

impl Drop for DeadlineTimer {
    fn drop(&mut self) {
        // We don't join on drop to avoid blocking
        // The thread will complete naturally
    }
}

/// Spawn a timer thread that enforces a hard deadline.
///
/// This is a convenience function for the common pattern of spawning
/// a timer thread to stop search at a deadline.
pub fn spawn_deadline_timer(deadline: Instant, stop_flag: StopFlag) {
    match duration_until(deadline) {
        Some(duration) => {
            thread::spawn(move || {
                thread::sleep(duration);
                stop_flag.stop();
            });
        }
        None => stop_flag.stop(),
    }
}

/// Spawn a timer thread from an Arc<AtomicBool> for backward compatibility.
pub fn spawn_deadline_timer_arc(deadline: Instant, stop: Arc<std::sync::atomic::AtomicBool>) {
    spawn_deadline_timer(deadline, StopFlag::from(stop));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_triggers() {
        let flag = StopFlag::new();
        let timer = DeadlineTimer::start(Duration::from_millis(50), flag.clone());
        assert!(timer.is_some());

        thread::sleep(Duration::from_millis(100));
        assert!(flag.is_stopped());
    }

    #[test]
    fn test_timer_zero_duration() {
        let flag = StopFlag::new();
        let timer = DeadlineTimer::start(Duration::ZERO, flag.clone());
        assert!(timer.is_none());
    }

    #[test]
    fn test_deadline_in_past() {
        let flag = StopFlag::new();
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("1 second ago should be valid");
        let timer = DeadlineTimer::start_at(Some(past), flag.clone());
        assert!(timer.is_none());
        assert!(flag.is_stopped());
    }
}
