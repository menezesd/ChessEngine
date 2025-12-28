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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
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
