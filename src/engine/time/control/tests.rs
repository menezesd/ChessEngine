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
fn converts_seconds_and_centiseconds_to_millis() {
    assert_eq!(seconds_to_ms(3), 3000);
    assert_eq!(centiseconds_to_ms(42), 420);
}

#[test]
fn centiseconds_to_ms_saturates_large_values() {
    assert_eq!(centiseconds_to_ms(u64::MAX), u64::MAX);
}

#[test]
fn duration_ms_converts_duration() {
    assert_eq!(duration_ms(Duration::from_millis(1250)), 1250);
}

#[test]
fn duration_ms_saturates_large_duration() {
    assert_eq!(duration_ms(Duration::MAX), u64::MAX);
}

#[test]
fn infinite_is_unlimited() {
    assert!(TimeControl::Infinite.is_unlimited());
}

#[test]
fn depth_is_unlimited() {
    assert!(TimeControl::Depth.is_unlimited());
}

#[test]
fn movetime_is_not_unlimited() {
    assert!(!TimeControl::MoveTime { time_ms: 5000 }.is_unlimited());
}

#[test]
fn incremental_is_not_unlimited() {
    let tc = TimeControl::Incremental {
        time_left_ms: 300000,
        inc_ms: 3000,
        movestogo: None,
    };
    assert!(!tc.is_unlimited());
}

#[test]
fn incremental_from_duration() {
    let tc = TimeControl::incremental(Duration::from_mins(5), Duration::from_secs(3), Some(40));

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
fn move_time_from_duration() {
    let tc = TimeControl::move_time(Duration::from_secs(5));

    match tc {
        TimeControl::MoveTime { time_ms } => assert_eq!(time_ms, 5000),
        _ => panic!("Expected MoveTime"),
    }
}

#[test]
fn move_time_ms() {
    let tc = TimeControl::move_time_ms(7500);

    match tc {
        TimeControl::MoveTime { time_ms } => assert_eq!(time_ms, 7500),
        _ => panic!("Expected MoveTime"),
    }
}

#[test]
fn from_xboard_st() {
    let tc = TimeControl::from_xboard_st(10);

    match tc {
        TimeControl::MoveTime { time_ms } => assert_eq!(time_ms, 10000),
        _ => panic!("Expected MoveTime"),
    }
}

#[test]
fn from_xboard_time() {
    let tc = TimeControl::from_xboard_time(30000, 3, Some(40));

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
fn compute_limits_infinite() {
    let (soft, hard) = TimeControl::Infinite.compute_limits(&test_config());

    assert_eq!(soft, u64::MAX);
    assert_eq!(hard, u64::MAX);
}

#[test]
fn compute_limits_depth() {
    let (soft, hard) = TimeControl::Depth.compute_limits(&test_config());

    assert_eq!(soft, u64::MAX);
    assert_eq!(hard, u64::MAX);
}

#[test]
fn compute_limits_movetime() {
    let tc = TimeControl::MoveTime { time_ms: 5000 };
    let (soft, hard) = tc.compute_limits(&test_config());

    assert_eq!(soft, 5000);
    assert_eq!(hard, 5000);
}

#[test]
fn compute_limits_movetime_zero() {
    let tc = TimeControl::MoveTime { time_ms: 0 };
    let (soft, hard) = tc.compute_limits(&test_config());

    assert_eq!(soft, 1);
    assert_eq!(hard, 1);
}

#[test]
fn compute_limits_incremental_normal() {
    let tc = TimeControl::Incremental {
        time_left_ms: 300000,
        inc_ms: 3000,
        movestogo: None,
    };

    let (soft, hard) = tc.compute_limits(&test_config());

    assert!(soft > 0);
    assert!(hard >= soft);
    assert!(soft < 300000);
    assert!(hard < 300000);
}

#[test]
fn compute_limits_incremental_with_movestogo() {
    let tc = TimeControl::Incremental {
        time_left_ms: 60000,
        inc_ms: 0,
        movestogo: Some(20),
    };

    let (soft, _hard) = tc.compute_limits(&test_config());

    assert!(soft > 0);
    assert!(soft <= 60000);
}

#[test]
fn compute_limits_critical_time() {
    let tc = TimeControl::Incremental {
        time_left_ms: 100,
        inc_ms: 0,
        movestogo: None,
    };

    let (soft, _hard) = tc.compute_limits(&test_config());

    assert!(soft > 0);
    assert!(soft <= 100);
}

#[test]
fn compute_limits_panic_mode() {
    let tc = TimeControl::Incremental {
        time_left_ms: 3000,
        inc_ms: 0,
        movestogo: None,
    };

    let (soft, _hard) = tc.compute_limits(&test_config());

    assert!(soft > 0);
    assert!(soft < 3000);
}

#[test]
fn compute_limits_long_time_control() {
    let tc = TimeControl::Incremental {
        time_left_ms: 600000,
        inc_ms: 5000,
        movestogo: None,
    };

    let (soft, hard) = tc.compute_limits(&test_config());

    assert!(soft > 5000);
    assert!(hard > soft);
}

#[test]
fn default_is_infinite() {
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

    let config = TimeConfig {
        move_overhead_ms: 0,
        ..test_config()
    };
    let (soft, hard) = tc.compute_limits(&config);

    assert!(soft >= 1);
    assert!(hard >= 1);
}

#[test]
fn incremental_only_increment() {
    let tc = TimeControl::Incremental {
        time_left_ms: 100,
        inc_ms: 10000,
        movestogo: None,
    };

    let (soft, _hard) = tc.compute_limits(&test_config());

    assert!(soft > 0);
}

#[test]
fn incremental_movestogo_one() {
    let tc = TimeControl::Incremental {
        time_left_ms: 60000,
        inc_ms: 0,
        movestogo: Some(1),
    };

    let (soft, hard) = tc.compute_limits(&test_config());

    assert!(soft > 0);
    assert!(hard > 0);
}
