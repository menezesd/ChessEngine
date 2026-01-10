//! Unified time management for UCI and `XBoard` protocols.
//!
//! This module provides a protocol-agnostic time control abstraction that both
//! UCI and `XBoard` handlers can use to compute search time limits.

use std::time::Duration;

/// Default moves to go estimate when not specified
pub const DEFAULT_MOVES_TO_GO: u64 = 30;

/// Time threshold below which we enter "panic mode" (in ms)
const PANIC_THRESHOLD_MS: u64 = 5000;

/// Minimum moves-to-go estimate to avoid over-thinking
const MIN_MOVES_TO_GO: u64 = 10;

/// Time control settings for a search.
///
/// This enum unifies different time control modes used by UCI and `XBoard` protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeControl {
    /// Infinite search - no time limit
    #[default]
    Infinite,
    /// Fixed depth search - no time limit, depth controlled externally
    Depth,
    /// Fixed time per move
    MoveTime { time_ms: u64 },
    /// Standard time control with remaining time and increment
    Incremental {
        time_left_ms: u64,
        inc_ms: u64,
        movestogo: Option<u64>,
    },
}

impl TimeControl {
    /// Create a new incremental time control from Duration values.
    #[must_use]
    pub fn incremental(time_left: Duration, inc: Duration, movestogo: Option<u64>) -> Self {
        TimeControl::Incremental {
            time_left_ms: time_left.as_millis() as u64,
            inc_ms: inc.as_millis() as u64,
            movestogo,
        }
    }

    /// Create a fixed move time control from Duration.
    #[must_use]
    pub fn move_time(time: Duration) -> Self {
        TimeControl::MoveTime {
            time_ms: time.as_millis() as u64,
        }
    }

    /// Create a fixed move time control from milliseconds.
    #[must_use]
    pub fn move_time_ms(time_ms: u64) -> Self {
        TimeControl::MoveTime { time_ms }
    }

    // =========================================================================
    // XBoard-specific constructors (XBoard uses centiseconds)
    // =========================================================================

    /// Create time control from `XBoard`'s "st" command (seconds per move).
    #[must_use]
    pub fn from_xboard_st(seconds: u32) -> Self {
        TimeControl::MoveTime {
            time_ms: u64::from(seconds) * 1000,
        }
    }

    /// Create time control from `XBoard`'s "time" command (centiseconds remaining).
    ///
    /// `XBoard` sends time in centiseconds. This converts to an incremental time control.
    #[must_use]
    pub fn from_xboard_time(
        engine_time_cs: u64,
        increment_sec: u32,
        moves_per_session: Option<u32>,
    ) -> Self {
        TimeControl::Incremental {
            time_left_ms: engine_time_cs * 10,
            inc_ms: u64::from(increment_sec) * 1000,
            movestogo: moves_per_session.map(u64::from),
        }
    }

    /// Check if this is an unlimited time control (infinite or depth-based).
    #[must_use]
    pub fn is_unlimited(&self) -> bool {
        matches!(self, TimeControl::Infinite | TimeControl::Depth)
    }

    /// Compute soft and hard time limits for this time control.
    ///
    /// Returns `(soft_time_ms, hard_time_ms)` or `(u64::MAX, u64::MAX)` for unlimited.
    #[must_use]
    pub fn compute_limits(
        &self,
        move_overhead_ms: u64,
        soft_time_percent: u64,
        hard_time_percent: u64,
    ) -> (u64, u64) {
        match self {
            TimeControl::Infinite | TimeControl::Depth => (u64::MAX, u64::MAX),
            TimeControl::MoveTime { time_ms } => {
                // For explicit movetime, use it directly (don't subtract overhead)
                // The user specified exactly how long to search
                let capped = (*time_ms).max(1);
                (capped, capped)
            }
            TimeControl::Incremental {
                time_left_ms,
                inc_ms,
                movestogo,
            } => compute_incremental_limits(
                *time_left_ms,
                *inc_ms,
                *movestogo,
                move_overhead_ms,
                soft_time_percent,
                hard_time_percent,
            ),
        }
    }
}

/// Compute soft and hard time limits for incremental time control.
#[allow(clippy::cast_precision_loss)]
fn compute_incremental_limits(
    time_left_ms: u64,
    inc_ms: u64,
    movestogo: Option<u64>,
    move_overhead_ms: u64,
    soft_time_percent: u64,
    hard_time_percent: u64,
) -> (u64, u64) {
    let safe_ms = time_left_ms.saturating_sub(move_overhead_ms);

    // Critical time: less than overhead + safety margin
    if time_left_ms <= move_overhead_ms.saturating_add(50) {
        let fallback = time_left_ms / 2;
        let fallback = fallback.max(1);
        return (fallback, fallback);
    }

    // Panic mode: use very little time when running low
    if safe_ms < PANIC_THRESHOLD_MS {
        // Use a fraction of remaining time based on how critical it is
        let panic_factor = safe_ms as f64 / PANIC_THRESHOLD_MS as f64;
        let target = (safe_ms as f64 * 0.05 * panic_factor) as u64 + inc_ms;
        let target = target.min(safe_ms / 5).max(1);
        let hard = (safe_ms / 3).max(target).max(1);
        return (target, hard);
    }

    // Estimate moves to go if not provided
    let moves_to_go = movestogo
        .unwrap_or({
            // Assume game is roughly in middle if we have decent time
            // Use more conservative estimate when time is lower
            if safe_ms > 300_000 {
                40 // Long time control - more moves expected
            } else if safe_ms > 60_000 {
                30 // Medium time control
            } else {
                25 // Short time control - be more aggressive
            }
        })
        .max(MIN_MOVES_TO_GO);

    // Base time per move
    let base_time = safe_ms / moves_to_go + inc_ms;

    // Apply percentage caps
    let soft_cap = safe_ms * soft_time_percent / 100;
    let hard_cap = safe_ms * hard_time_percent / 100;

    // Soft time: use more time for first moves, less for later
    let soft_ms = base_time.min(soft_cap).max(1);

    // Hard time: absolute maximum for this move
    let hard_ms = hard_cap.max(soft_ms).max(1);

    (soft_ms, hard_ms)
}

/// Compute soft and hard time limits for a search (legacy API for UCI compatibility).
///
/// Returns `(soft_time_ms, hard_time_ms)` where:
/// - `soft_time_ms`: target time to complete search
/// - `hard_time_ms`: maximum time before hard stop
#[must_use]
pub fn compute_time_limits(
    time_left: Duration,
    inc: Duration,
    movetime: Option<Duration>,
    movestogo: Option<u64>,
    move_overhead_ms: u64,
    soft_time_percent: u64,
    hard_time_percent: u64,
) -> (u64, u64) {
    // Fixed movetime takes priority
    if let Some(mt) = movetime {
        let tc = TimeControl::move_time(mt);
        return tc.compute_limits(move_overhead_ms, soft_time_percent, hard_time_percent);
    }

    let tc = TimeControl::incremental(time_left, inc, movestogo);
    tc.compute_limits(move_overhead_ms, soft_time_percent, hard_time_percent)
}

/// Parameters for executing a search (shared builder for protocol layers).
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub soft_time_ms: u64,
    pub hard_time_ms: u64,
    pub max_nodes: u64,
    pub depth: Option<u32>,
    pub ponder: bool,
    pub infinite: bool,
}

/// Build a search request from a time control and constraints.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_search_request(
    time_control: TimeControl,
    depth: Option<u32>,
    nodes: Option<u64>,
    ponder: bool,
    infinite: bool,
    default_max_nodes: u64,
    move_overhead_ms: u64,
    soft_time_percent: u64,
    hard_time_percent: u64,
) -> (SearchRequest, (u64, u64)) {
    let (soft_ms, hard_ms) = if infinite || ponder {
        (u64::MAX, u64::MAX)
    } else {
        time_control.compute_limits(move_overhead_ms, soft_time_percent, hard_time_percent)
    };

    let max_nodes = nodes.unwrap_or(default_max_nodes);

    (
        SearchRequest {
            soft_time_ms: if infinite || ponder { 0 } else { soft_ms },
            hard_time_ms: if infinite || ponder { 0 } else { hard_ms },
            max_nodes,
            depth,
            ponder,
            infinite,
        },
        (soft_ms, hard_ms),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ========================================================================
    // TimeControl enum tests
    // ========================================================================

    #[test]
    fn time_control_infinite_is_unlimited() {
        let tc = TimeControl::Infinite;
        assert!(tc.is_unlimited());
    }

    #[test]
    fn time_control_depth_is_unlimited() {
        let tc = TimeControl::Depth;
        assert!(tc.is_unlimited());
    }

    #[test]
    fn time_control_movetime_is_not_unlimited() {
        let tc = TimeControl::MoveTime { time_ms: 5000 };
        assert!(!tc.is_unlimited());
    }

    #[test]
    fn time_control_incremental_is_not_unlimited() {
        let tc = TimeControl::Incremental {
            time_left_ms: 300000,
            inc_ms: 3000,
            movestogo: None,
        };
        assert!(!tc.is_unlimited());
    }

    // ========================================================================
    // TimeControl constructors
    // ========================================================================

    #[test]
    fn time_control_incremental_from_duration() {
        let tc =
            TimeControl::incremental(Duration::from_secs(300), Duration::from_secs(3), Some(40));

        match tc {
            TimeControl::Incremental {
                time_left_ms,
                inc_ms,
                movestogo,
            } => {
                assert_eq!(time_left_ms, 300000);
                assert_eq!(inc_ms, 3000);
                assert_eq!(movestogo, Some(40));
            }
            _ => panic!("Expected Incremental"),
        }
    }

    #[test]
    fn time_control_move_time_from_duration() {
        let tc = TimeControl::move_time(Duration::from_secs(5));

        match tc {
            TimeControl::MoveTime { time_ms } => {
                assert_eq!(time_ms, 5000);
            }
            _ => panic!("Expected MoveTime"),
        }
    }

    #[test]
    fn time_control_move_time_ms() {
        let tc = TimeControl::move_time_ms(7500);

        match tc {
            TimeControl::MoveTime { time_ms } => {
                assert_eq!(time_ms, 7500);
            }
            _ => panic!("Expected MoveTime"),
        }
    }

    #[test]
    fn time_control_from_xboard_st() {
        let tc = TimeControl::from_xboard_st(10);

        match tc {
            TimeControl::MoveTime { time_ms } => {
                assert_eq!(time_ms, 10000);
            }
            _ => panic!("Expected MoveTime"),
        }
    }

    #[test]
    fn time_control_from_xboard_time() {
        // XBoard sends time in centiseconds
        let tc = TimeControl::from_xboard_time(30000, 3, Some(40)); // 300 seconds, 3 sec inc

        match tc {
            TimeControl::Incremental {
                time_left_ms,
                inc_ms,
                movestogo,
            } => {
                assert_eq!(time_left_ms, 300000); // 30000 cs * 10 = 300000 ms
                assert_eq!(inc_ms, 3000); // 3 sec * 1000 = 3000 ms
                assert_eq!(movestogo, Some(40));
            }
            _ => panic!("Expected Incremental"),
        }
    }

    // ========================================================================
    // compute_limits tests
    // ========================================================================

    #[test]
    fn compute_limits_infinite() {
        let tc = TimeControl::Infinite;
        let (soft, hard) = tc.compute_limits(50, 5, 20);

        assert_eq!(soft, u64::MAX);
        assert_eq!(hard, u64::MAX);
    }

    #[test]
    fn compute_limits_depth() {
        let tc = TimeControl::Depth;
        let (soft, hard) = tc.compute_limits(50, 5, 20);

        assert_eq!(soft, u64::MAX);
        assert_eq!(hard, u64::MAX);
    }

    #[test]
    fn compute_limits_movetime() {
        let tc = TimeControl::MoveTime { time_ms: 5000 };
        let (soft, hard) = tc.compute_limits(50, 5, 20);

        // For movetime, both should equal the specified time
        assert_eq!(soft, 5000);
        assert_eq!(hard, 5000);
    }

    #[test]
    fn compute_limits_movetime_zero() {
        let tc = TimeControl::MoveTime { time_ms: 0 };
        let (soft, hard) = tc.compute_limits(50, 5, 20);

        // Should be capped at minimum 1ms
        assert_eq!(soft, 1);
        assert_eq!(hard, 1);
    }

    #[test]
    fn compute_limits_incremental_normal() {
        let tc = TimeControl::Incremental {
            time_left_ms: 300000, // 5 minutes
            inc_ms: 3000,         // 3 second increment
            movestogo: None,
        };

        let (soft, hard) = tc.compute_limits(50, 5, 20);

        // Should produce reasonable values
        assert!(soft > 0);
        assert!(hard >= soft);
        assert!(soft < 300000);
        assert!(hard < 300000);
    }

    #[test]
    fn compute_limits_incremental_with_movestogo() {
        let tc = TimeControl::Incremental {
            time_left_ms: 60000, // 1 minute
            inc_ms: 0,
            movestogo: Some(20), // 20 moves to go
        };

        let (soft, _hard) = tc.compute_limits(50, 5, 20);

        // Should allocate roughly 60000/20 = 3000ms per move
        assert!(soft > 0);
        assert!(soft <= 60000);
    }

    #[test]
    fn compute_limits_critical_time() {
        // Very low time - should trigger critical time handling
        let tc = TimeControl::Incremental {
            time_left_ms: 100, // Only 100ms left
            inc_ms: 0,
            movestogo: None,
        };

        let (soft, _hard) = tc.compute_limits(50, 5, 20);

        // Should be very small but non-zero
        assert!(soft > 0);
        assert!(soft <= 100);
    }

    #[test]
    fn compute_limits_panic_mode() {
        // Low time - should trigger panic mode
        let tc = TimeControl::Incremental {
            time_left_ms: 3000, // Only 3 seconds
            inc_ms: 0,
            movestogo: None,
        };

        let (soft, _hard) = tc.compute_limits(50, 5, 20);

        // Should be conservative
        assert!(soft > 0);
        assert!(soft < 3000);
    }

    #[test]
    fn compute_limits_long_time_control() {
        let tc = TimeControl::Incremental {
            time_left_ms: 600000, // 10 minutes
            inc_ms: 5000,         // 5 second increment
            movestogo: None,
        };

        let (soft, hard) = tc.compute_limits(50, 5, 20);

        // With lots of time, should use more time per move
        assert!(soft > 5000); // Should use more than just the increment
        assert!(hard > soft);
    }

    // ========================================================================
    // compute_time_limits legacy API tests
    // ========================================================================

    #[test]
    fn compute_time_limits_with_movetime() {
        let (soft, hard) = compute_time_limits(
            Duration::from_secs(300),
            Duration::from_secs(0),
            Some(Duration::from_secs(5)), // movetime takes priority
            None,
            50,
            5,
            20,
        );

        assert_eq!(soft, 5000);
        assert_eq!(hard, 5000);
    }

    #[test]
    fn compute_time_limits_without_movetime() {
        let (soft, hard) = compute_time_limits(
            Duration::from_secs(300),
            Duration::from_secs(3),
            None,
            None,
            50,
            5,
            20,
        );

        assert!(soft > 0);
        assert!(hard >= soft);
    }

    #[test]
    fn compute_time_limits_with_movestogo() {
        let (soft, _hard) = compute_time_limits(
            Duration::from_secs(60),
            Duration::from_secs(0),
            None,
            Some(10), // 10 moves to go
            50,
            5,
            20,
        );

        // Should allocate roughly 60s / 10 = 6s per move
        assert!(soft > 0);
        assert!(soft <= 60000);
    }

    // ========================================================================
    // build_search_request tests
    // ========================================================================

    #[test]
    fn build_search_request_infinite() {
        let tc = TimeControl::Incremental {
            time_left_ms: 300000,
            inc_ms: 3000,
            movestogo: None,
        };

        let (req, _) = build_search_request(tc, None, None, false, true, 0, 50, 5, 20);

        assert!(req.infinite);
        assert_eq!(req.soft_time_ms, 0);
        assert_eq!(req.hard_time_ms, 0);
    }

    #[test]
    fn build_search_request_ponder() {
        let tc = TimeControl::Incremental {
            time_left_ms: 300000,
            inc_ms: 3000,
            movestogo: None,
        };

        let (req, _) = build_search_request(tc, None, None, true, false, 0, 50, 5, 20);

        assert!(req.ponder);
        assert_eq!(req.soft_time_ms, 0);
        assert_eq!(req.hard_time_ms, 0);
    }

    #[test]
    fn build_search_request_with_depth() {
        let tc = TimeControl::Infinite;
        let (req, _) = build_search_request(tc, Some(10), None, false, false, 0, 50, 5, 20);

        assert_eq!(req.depth, Some(10));
    }

    #[test]
    fn build_search_request_with_nodes() {
        let tc = TimeControl::Infinite;
        let (req, _) = build_search_request(tc, None, Some(1000000), false, false, 0, 50, 5, 20);

        assert_eq!(req.max_nodes, 1000000);
    }

    #[test]
    fn build_search_request_default_nodes() {
        let tc = TimeControl::Infinite;
        let (req, _) = build_search_request(tc, None, None, false, false, 500000, 50, 5, 20);

        assert_eq!(req.max_nodes, 500000);
    }

    #[test]
    fn build_search_request_normal() {
        let tc = TimeControl::Incremental {
            time_left_ms: 300000,
            inc_ms: 3000,
            movestogo: None,
        };

        let (req, (soft, hard)) = build_search_request(tc, None, None, false, false, 0, 50, 5, 20);

        assert!(!req.infinite);
        assert!(!req.ponder);
        assert!(req.soft_time_ms > 0);
        assert!(req.hard_time_ms > 0);
        assert_eq!(req.soft_time_ms, soft);
        assert_eq!(req.hard_time_ms, hard);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn time_control_default_is_infinite() {
        let tc = TimeControl::default();
        assert!(matches!(tc, TimeControl::Infinite));
    }

    #[test]
    fn incremental_zero_time_left() {
        let tc = TimeControl::Incremental {
            time_left_ms: 0,
            inc_ms: 0,
            movestogo: None,
        };

        let (soft, hard) = tc.compute_limits(0, 5, 20);

        // Should handle gracefully
        assert!(soft >= 1);
        assert!(hard >= 1);
    }

    #[test]
    fn incremental_only_increment() {
        let tc = TimeControl::Incremental {
            time_left_ms: 100, // Very little base time
            inc_ms: 10000,     // But large increment
            movestogo: None,
        };

        let (soft, _hard) = tc.compute_limits(50, 5, 20);

        // Should use the increment
        assert!(soft > 0);
    }

    #[test]
    fn incremental_movestogo_one() {
        let tc = TimeControl::Incremental {
            time_left_ms: 60000,
            inc_ms: 0,
            movestogo: Some(1), // Last move before time control
        };

        let (soft, hard) = tc.compute_limits(50, 5, 20);

        // Should use most of the remaining time
        assert!(soft > 0);
        assert!(hard > 0);
    }

    #[test]
    fn search_request_fields() {
        let req = SearchRequest {
            soft_time_ms: 5000,
            hard_time_ms: 10000,
            max_nodes: 1000000,
            depth: Some(20),
            ponder: false,
            infinite: false,
        };

        assert_eq!(req.soft_time_ms, 5000);
        assert_eq!(req.hard_time_ms, 10000);
        assert_eq!(req.max_nodes, 1000000);
        assert_eq!(req.depth, Some(20));
        assert!(!req.ponder);
        assert!(!req.infinite);
    }
}
