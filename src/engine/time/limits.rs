use std::time::Duration;

use super::{TimeConfig, TimeControl};

/// Time threshold below which we enter "panic mode" (in ms)
const PANIC_THRESHOLD_MS: u64 = 5000;

/// Minimum moves-to-go estimate to avoid over-thinking
const MIN_MOVES_TO_GO: u64 = 10;

/// Safety margin added to overhead for critical time detection
const CRITICAL_TIME_MARGIN_MS: u64 = 50;
const CRITICAL_TIME_FALLBACK_DIVISOR: u64 = 2;
const PERCENT_DENOMINATOR: u64 = 100;

/// Panic mode target before increment is `safe_ms * safe_ms / PANIC_TIME_DENOMINATOR`.
const PANIC_TIME_DENOMINATOR: u128 = 100_000;

/// Panic mode: minimum fraction divisor for target time
const PANIC_MIN_FRACTION: u64 = 5;

/// Panic mode: hard time fraction divisor
const PANIC_HARD_FRACTION: u64 = 3;

/// Time thresholds for moves-to-go estimation (in ms)
const LONG_TIME_CONTROL_MS: u64 = 300_000;
const MEDIUM_TIME_CONTROL_MS: u64 = 60_000;

/// Estimated moves for different time controls
const LONG_MOVES_ESTIMATE: u64 = 40;
const MEDIUM_MOVES_ESTIMATE: u64 = 30;
const SHORT_MOVES_ESTIMATE: u64 = 25;

fn estimated_moves_to_go(safe_ms: u64) -> u64 {
    if safe_ms > LONG_TIME_CONTROL_MS {
        LONG_MOVES_ESTIMATE
    } else if safe_ms > MEDIUM_TIME_CONTROL_MS {
        MEDIUM_MOVES_ESTIMATE
    } else {
        SHORT_MOVES_ESTIMATE
    }
}

fn is_critical_time(time_left_ms: u64, move_overhead_ms: u64) -> bool {
    time_left_ms <= move_overhead_ms.saturating_add(CRITICAL_TIME_MARGIN_MS)
}

fn critical_time_limits(time_left_ms: u64) -> (u64, u64) {
    let fallback = (time_left_ms / CRITICAL_TIME_FALLBACK_DIVISOR).max(1);
    (fallback, fallback)
}

fn panic_time_limits(safe_ms: u64, inc_ms: u64) -> (u64, u64) {
    let panic_base = (u128::from(safe_ms) * u128::from(safe_ms)) / PANIC_TIME_DENOMINATOR;
    let panic_base = u64::try_from(panic_base).unwrap_or(u64::MAX);
    let target = panic_base.saturating_add(inc_ms);
    let target = target.min(safe_ms / PANIC_MIN_FRACTION).max(1);
    let hard = (safe_ms / PANIC_HARD_FRACTION).max(target).max(1);
    (target, hard)
}

/// Compute soft and hard time limits for incremental time control.
pub(super) fn compute_incremental_limits(
    time_left_ms: u64,
    inc_ms: u64,
    movestogo: Option<u64>,
    config: &TimeConfig,
) -> (u64, u64) {
    let safe_ms = time_left_ms.saturating_sub(config.move_overhead_ms);

    if is_critical_time(time_left_ms, config.move_overhead_ms) {
        // Budget from the time that remains after transmission overhead;
        // budgeting from the raw clock here can schedule more thinking
        // time than actually exists and flag on the spot.
        return critical_time_limits(safe_ms);
    }

    if safe_ms < PANIC_THRESHOLD_MS {
        return panic_time_limits(safe_ms, inc_ms);
    }

    // The floor guards only our own estimate. An explicit `movestogo` from
    // the GUI is authoritative: with `movestogo 1` (last move before a time
    // control) the engine should spend most of the clock, not a tenth.
    let moves_to_go = movestogo.map_or_else(
        || estimated_moves_to_go(safe_ms).max(MIN_MOVES_TO_GO),
        |mtg| mtg.max(1),
    );

    let base_time = (safe_ms / moves_to_go).saturating_add(inc_ms);
    let soft_cap = safe_ms.saturating_mul(config.soft_time_percent) / PERCENT_DENOMINATOR;
    let hard_cap = safe_ms.saturating_mul(config.hard_time_percent) / PERCENT_DENOMINATOR;

    let soft_ms = base_time.min(soft_cap).max(1);
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
    config: &TimeConfig,
) -> (u64, u64) {
    if let Some(mt) = movetime {
        let tc = TimeControl::move_time(mt);
        return tc.compute_limits(config);
    }

    let tc = TimeControl::incremental(time_left, inc, movestogo);
    tc.compute_limits(config)
}

#[cfg(test)]
mod tests;
