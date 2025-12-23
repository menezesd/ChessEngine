use std::time::Duration;

pub const DEFAULT_MOVES_TO_GO: u64 = 30;

pub fn compute_time_limits(
    time_left: Duration,
    inc: Duration,
    movetime: Option<Duration>,
    movestogo: Option<u64>,
    move_overhead_ms: u64,
    soft_time_percent: u64,
    hard_time_percent: u64,
) -> (u64, u64) {
    let time_left_ms = time_left.as_millis() as u64;
    let inc_ms = inc.as_millis() as u64;
    let safe_ms = time_left_ms.saturating_sub(move_overhead_ms);

    if let Some(mt) = movetime {
        let mt_ms = mt.as_millis() as u64;
        let capped = if safe_ms > 0 { mt_ms.min(safe_ms) } else { mt_ms };
        let capped = capped.max(1);
        return (capped, capped);
    }

    if time_left_ms <= move_overhead_ms.saturating_add(20) {
        let fallback = time_left_ms / 2;
        let fallback = fallback.max(1);
        return (fallback, fallback);
    }

    let moves_to_go = movestogo.unwrap_or(DEFAULT_MOVES_TO_GO).max(1);
    let soft_ms = safe_ms / moves_to_go + inc_ms;
    let soft_cap = safe_ms * soft_time_percent / 100;
    let hard_cap = safe_ms * hard_time_percent / 100;
    let soft_ms = soft_ms.min(soft_cap).max(1);
    let hard_ms = hard_cap.max(soft_ms).max(1);
    (soft_ms, hard_ms)
}
