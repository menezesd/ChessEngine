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
