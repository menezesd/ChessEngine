use std::time::Duration;

use super::limits;

/// Default moves to go estimate when not specified.
pub const DEFAULT_MOVES_TO_GO: u64 = 30;
const MILLIS_PER_SECOND: u64 = 1000;
const MILLIS_PER_CENTISECOND: u64 = 10;
const MIN_MOVE_TIME_MS: u64 = 1;

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn seconds_to_ms(seconds: u32) -> u64 {
    u64::from(seconds) * MILLIS_PER_SECOND
}

fn centiseconds_to_ms(centiseconds: u64) -> u64 {
    centiseconds.saturating_mul(MILLIS_PER_CENTISECOND)
}

/// Configuration for time management calculations.
///
/// Groups together the various percentages and overheads used in time limit calculations.
#[derive(Debug, Clone, Copy)]
pub struct TimeConfig {
    /// Time to reserve for move overhead (communication latency, etc.)
    pub move_overhead_ms: u64,
    /// Percentage of remaining time to use as soft limit
    pub soft_time_percent: u64,
    /// Percentage of remaining time to use as hard limit
    pub hard_time_percent: u64,
    /// Default maximum nodes (0 = unlimited)
    pub default_max_nodes: u64,
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            move_overhead_ms: 50,
            soft_time_percent: 70,
            hard_time_percent: 90,
            default_max_nodes: 0,
        }
    }
}

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
            time_left_ms: duration_ms(time_left),
            inc_ms: duration_ms(inc),
            movestogo,
        }
    }

    /// Create a fixed move time control from Duration.
    #[must_use]
    pub fn move_time(time: Duration) -> Self {
        TimeControl::MoveTime {
            time_ms: duration_ms(time),
        }
    }

    /// Create a fixed move time control from milliseconds.
    #[must_use]
    pub fn move_time_ms(time_ms: u64) -> Self {
        TimeControl::MoveTime { time_ms }
    }

    /// Create time control from `XBoard`'s "st" command (seconds per move).
    #[must_use]
    pub fn from_xboard_st(seconds: u32) -> Self {
        TimeControl::MoveTime {
            time_ms: seconds_to_ms(seconds),
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
            time_left_ms: centiseconds_to_ms(engine_time_cs),
            inc_ms: seconds_to_ms(increment_sec),
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
    pub fn compute_limits(&self, config: &TimeConfig) -> (u64, u64) {
        match self {
            TimeControl::Infinite | TimeControl::Depth => (u64::MAX, u64::MAX),
            TimeControl::MoveTime { time_ms } => {
                let capped = (*time_ms).max(MIN_MOVE_TIME_MS);
                (capped, capped)
            }
            TimeControl::Incremental {
                time_left_ms,
                inc_ms,
                movestogo,
            } => limits::compute_incremental_limits(*time_left_ms, *inc_ms, *movestogo, config),
        }
    }
}

#[cfg(test)]
mod tests;
