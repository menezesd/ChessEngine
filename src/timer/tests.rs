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
fn test_cancel_does_not_trigger_stop_flag() {
    let flag = StopFlag::new();
    let timer =
        DeadlineTimer::start(Duration::from_millis(25), flag.clone()).expect("timer should start");

    timer.cancel();
    thread::sleep(Duration::from_millis(50));

    assert!(!flag.is_stopped());
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

#[test]
fn deadline_after_ms_adds_milliseconds_to_start() {
    let start = Instant::now();

    assert_eq!(
        deadline_after_ms(start, 250).unwrap().duration_since(start),
        Duration::from_millis(250)
    );
}

#[test]
fn deadline_after_ms_treats_u64_max_as_unlimited() {
    assert!(deadline_after_ms(Instant::now(), u64::MAX).is_none());
}
