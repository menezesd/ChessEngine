//! Timer utilities for search time management.
//!
//! Provides deadline-based timers that can signal stop flags.

use std::sync::atomic::{AtomicBool, Ordering};
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

/// A timer that signals a stop flag when a deadline is reached.
///
/// The timer runs in a background thread and will automatically
/// set the stop flag when the deadline expires.
pub struct DeadlineTimer {
    handle: Option<JoinHandle<()>>,
    stop_flag: StopFlag,
    cancelled: Arc<AtomicBool>,
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
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_clone = Arc::clone(&cancelled);
        let handle = thread::spawn(move || {
            thread::sleep(duration);
            if !cancelled_clone.load(Ordering::Relaxed) {
                flag_clone.stop();
            }
        });

        Some(DeadlineTimer {
            handle: Some(handle),
            stop_flag,
            cancelled,
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
        self.cancelled.store(true, Ordering::Relaxed);
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
mod tests;
