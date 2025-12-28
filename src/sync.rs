//! Synchronization primitives for the chess engine.
//!
//! Provides thread-safe utilities for search control.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A thread-safe stop flag for controlling search termination.
///
/// This wraps `Arc<AtomicBool>` to provide a cleaner API and avoid
/// repeating the same pattern throughout the codebase.
#[derive(Clone, Debug)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    /// Create a new stop flag (initially not stopped).
    #[must_use]
    pub fn new() -> Self {
        StopFlag(Arc::new(AtomicBool::new(false)))
    }

    /// Create a stop flag that is already set.
    #[must_use]
    pub fn stopped() -> Self {
        StopFlag(Arc::new(AtomicBool::new(true)))
    }

    /// Check if the stop flag is set.
    #[inline]
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Set the stop flag.
    #[inline]
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Clear the stop flag.
    #[inline]
    pub fn reset(&self) {
        self.0.store(false, Ordering::Relaxed);
    }

    /// Get a reference to the underlying `AtomicBool`.
    /// Useful for compatibility with existing code.
    #[inline]
    #[must_use]
    pub fn as_atomic(&self) -> &AtomicBool {
        &self.0
    }

    /// Get a clone of the underlying Arc for sharing.
    #[inline]
    #[must_use]
    pub fn as_arc(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }
}

impl Default for StopFlag {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Arc<AtomicBool>> for StopFlag {
    fn from(arc: Arc<AtomicBool>) -> Self {
        StopFlag(arc)
    }
}

impl From<StopFlag> for Arc<AtomicBool> {
    fn from(flag: StopFlag) -> Self {
        flag.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_flag_lifecycle() {
        let flag = StopFlag::new();
        assert!(!flag.is_stopped());

        flag.stop();
        assert!(flag.is_stopped());

        flag.reset();
        assert!(!flag.is_stopped());
    }

    #[test]
    fn test_stop_flag_clone() {
        let flag1 = StopFlag::new();
        let flag2 = flag1.clone();

        flag1.stop();
        assert!(flag2.is_stopped());
    }

    #[test]
    fn test_stop_flag_stopped() {
        let flag = StopFlag::stopped();
        assert!(flag.is_stopped());
    }
}
