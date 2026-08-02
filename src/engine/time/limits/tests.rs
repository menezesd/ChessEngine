use super::*;

fn test_config() -> TimeConfig {
    TimeConfig {
        move_overhead_ms: 50,
        soft_time_percent: 5,
        hard_time_percent: 20,
        default_max_nodes: 0,
    }
}

#[test]
fn compute_time_limits_with_movetime() {
    let (soft, hard) = compute_time_limits(
        Duration::from_mins(5),
        Duration::from_secs(0),
        Some(Duration::from_secs(5)),
        None,
        &test_config(),
    );

    assert_eq!(soft, 5000);
    assert_eq!(hard, 5000);
}

#[test]
fn compute_time_limits_without_movetime() {
    let (soft, hard) = compute_time_limits(
        Duration::from_mins(5),
        Duration::from_secs(3),
        None,
        None,
        &test_config(),
    );

    assert!(soft > 0);
    assert!(hard >= soft);
}

#[test]
fn compute_time_limits_with_movestogo() {
    let (soft, _hard) = compute_time_limits(
        Duration::from_mins(1),
        Duration::from_secs(0),
        None,
        Some(10),
        &test_config(),
    );

    assert!(soft > 0);
    assert!(soft <= 60000);
}

#[test]
fn compute_incremental_limits_saturates_extreme_clock_values() {
    let (soft, hard) = compute_incremental_limits(u64::MAX, u64::MAX, Some(1), &test_config());

    assert!(soft > 0);
    assert!(hard >= soft);
}

#[test]
fn compute_time_limits_uses_half_remaining_time_when_critical() {
    // 100 ms on the clock with 50 ms move overhead leaves 50 ms of real
    // thinking time; budgeting from the raw clock would flag on the spot.
    let (soft, hard) = compute_incremental_limits(100, 0, None, &test_config());

    assert_eq!((soft, hard), (25, 25));
}

#[test]
fn compute_time_limits_honors_explicit_movestogo_below_estimate_floor() {
    // `movestogo 1` is the last move before a time control: the engine
    // should budget most of the clock (bounded by the soft/hard caps),
    // not divide it by the internal 10-move estimate floor.
    let config = TimeConfig {
        move_overhead_ms: 50,
        soft_time_percent: 80,
        hard_time_percent: 90,
        default_max_nodes: 0,
    };
    let (soft_one, _) = compute_incremental_limits(60_000, 0, Some(1), &config);
    let (soft_ten, _) = compute_incremental_limits(60_000, 0, Some(10), &config);

    assert!(
        soft_one > soft_ten,
        "movestogo 1 should budget more than movestogo 10: {soft_one} vs {soft_ten}"
    );
}

#[test]
fn panic_time_limits_saturates_extreme_increment() {
    let (soft, hard) = compute_incremental_limits(1_000, u64::MAX, None, &test_config());

    assert!(soft > 0);
    assert!(hard >= soft);
}

#[test]
fn panic_time_limits_use_integer_formula() {
    assert_eq!(panic_time_limits(1_000, 0), (10, 333));
    assert_eq!(panic_time_limits(4_000, 0), (160, 1333));
}

#[test]
fn panic_time_limits_clamp_zero_safe_time() {
    assert_eq!(panic_time_limits(0, 0), (1, 1));
}

#[test]
fn estimates_moves_to_go_from_remaining_time() {
    assert_eq!(
        estimated_moves_to_go(LONG_TIME_CONTROL_MS + 1),
        LONG_MOVES_ESTIMATE
    );
    assert_eq!(
        estimated_moves_to_go(MEDIUM_TIME_CONTROL_MS + 1),
        MEDIUM_MOVES_ESTIMATE
    );
    assert_eq!(
        estimated_moves_to_go(MEDIUM_TIME_CONTROL_MS),
        SHORT_MOVES_ESTIMATE
    );
}
