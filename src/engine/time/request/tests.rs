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
fn build_search_request_infinite() {
    let tc = TimeControl::Incremental {
        time_left_ms: 300000,
        inc_ms: 3000,
        movestogo: None,
    };

    let (req, _) = build_search_request(tc, None, None, false, true, &test_config());

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

    let (req, (soft, hard)) = build_search_request(tc, None, None, true, false, &test_config());

    assert!(req.ponder);
    assert!(!req.infinite);
    assert!(req.soft_time_ms > 0);
    assert!(req.hard_time_ms > 0);
    assert_eq!(req.soft_time_ms, soft);
    assert_eq!(req.hard_time_ms, hard);
}

#[test]
fn build_search_request_infinite_ignores_planned_limits() {
    let tc = TimeControl::Incremental {
        time_left_ms: 300000,
        inc_ms: 3000,
        movestogo: None,
    };

    let (req, (soft, hard)) = build_search_request(tc, None, None, false, true, &test_config());

    assert!(req.infinite);
    assert_eq!(req.soft_time_ms, 0);
    assert_eq!(req.hard_time_ms, 0);
    assert_eq!((soft, hard), (u64::MAX, u64::MAX));
}

#[test]
fn build_search_request_with_depth() {
    let tc = TimeControl::Infinite;
    let (req, (soft, hard)) =
        build_search_request(tc, Some(10), None, false, false, &test_config());

    assert_eq!(req.depth, Some(10));
    assert_eq!(req.soft_time_ms, 0);
    assert_eq!(req.hard_time_ms, 0);
    assert_eq!((soft, hard), (u64::MAX, u64::MAX));
}

#[test]
fn build_search_request_with_nodes() {
    let tc = TimeControl::Infinite;
    let (req, _) = build_search_request(tc, None, Some(1000000), false, false, &test_config());

    assert_eq!(req.max_nodes, 1000000);
    assert_eq!(req.soft_time_ms, 0);
    assert_eq!(req.hard_time_ms, 0);
}

#[test]
fn build_search_request_default_nodes() {
    let tc = TimeControl::Infinite;
    let config = TimeConfig {
        default_max_nodes: 500000,
        ..test_config()
    };
    let (req, _) = build_search_request(tc, None, None, false, false, &config);

    assert_eq!(req.max_nodes, 500000);
}

#[test]
fn build_search_request_normal() {
    let tc = TimeControl::Incremental {
        time_left_ms: 300000,
        inc_ms: 3000,
        movestogo: None,
    };

    let (req, (soft, hard)) = build_search_request(tc, None, None, false, false, &test_config());

    assert!(!req.infinite);
    assert!(!req.ponder);
    assert!(req.soft_time_ms > 0);
    assert!(req.hard_time_ms > 0);
    assert_eq!(req.soft_time_ms, soft);
    assert_eq!(req.hard_time_ms, hard);
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
