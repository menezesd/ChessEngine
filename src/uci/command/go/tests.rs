use super::*;

#[test]
fn searchmoves_preserves_the_list_and_parses_following_limits() {
    let params = parse_go_params(&[
        "go",
        "wtime",
        "1000",
        "searchmoves",
        "e2e4",
        "d2d4",
        "depth",
        "3",
        "ponder",
    ]);

    assert_eq!(params.searchmoves, Some(vec!["e2e4".into(), "d2d4".into()]));
    assert_eq!(params.wtime, Some(1000));
    assert_eq!(params.depth, Some(3));
    assert!(params.ponder);
}

#[test]
fn empty_searchmoves_is_distinct_from_no_restriction() {
    let params = parse_go_params(&["go", "searchmoves", "depth", "3"]);
    assert_eq!(params.searchmoves, Some(Vec::new()));
    assert_eq!(params.depth, Some(3));
    assert_eq!(parse_go_params(&["go", "depth", "3"]).searchmoves, None);
}

#[test]
fn parse_go_params_empty() {
    let parts: Vec<&str> = vec!["go"];
    let params = parse_go_params(&parts);

    assert!(params.wtime.is_none());
    assert!(params.btime.is_none());
    assert!(params.depth.is_none());
    assert!(!params.infinite);
    assert!(!params.ponder);
}

#[test]
fn parse_go_params_depth() {
    let parts: Vec<&str> = vec!["go", "depth", "10"];
    let params = parse_go_params(&parts);

    assert_eq!(params.depth, Some(10));
}

#[test]
fn parse_go_params_movetime() {
    let parts: Vec<&str> = vec!["go", "movetime", "5000"];
    let params = parse_go_params(&parts);

    assert_eq!(params.movetime, Some(5000));
}

#[test]
fn parse_go_params_infinite() {
    let parts: Vec<&str> = vec!["go", "infinite"];
    let params = parse_go_params(&parts);

    assert!(params.infinite);
}

#[test]
fn parse_go_params_ponder() {
    let parts: Vec<&str> = vec!["go", "ponder"];
    let params = parse_go_params(&parts);

    assert!(params.ponder);
}

#[test]
fn parse_go_params_wtime_btime() {
    let parts: Vec<&str> = vec!["go", "wtime", "300000", "btime", "300000"];
    let params = parse_go_params(&parts);

    assert_eq!(params.wtime, Some(300000));
    assert_eq!(params.btime, Some(300000));
}

#[test]
fn parse_go_params_with_increment() {
    let parts: Vec<&str> = vec![
        "go", "wtime", "300000", "btime", "300000", "winc", "3000", "binc", "3000",
    ];
    let params = parse_go_params(&parts);

    assert_eq!(params.wtime, Some(300000));
    assert_eq!(params.btime, Some(300000));
    assert_eq!(params.winc, Some(3000));
    assert_eq!(params.binc, Some(3000));
}

#[test]
fn parse_go_params_movestogo() {
    let parts: Vec<&str> = vec!["go", "wtime", "60000", "btime", "60000", "movestogo", "40"];
    let params = parse_go_params(&parts);

    assert_eq!(params.movestogo, Some(40));
}

#[test]
fn parse_go_params_nodes() {
    let parts: Vec<&str> = vec!["go", "nodes", "1000000"];
    let params = parse_go_params(&parts);

    assert_eq!(params.nodes, Some(1000000));
}

#[test]
fn parse_go_params_mate() {
    let parts: Vec<&str> = vec!["go", "mate", "3"];
    let params = parse_go_params(&parts);

    assert_eq!(params.mate, Some(3));
}

#[test]
fn parse_go_params_complex() {
    let parts: Vec<&str> = vec![
        "go", "wtime", "300000", "btime", "300000", "winc", "3000", "binc", "3000", "depth", "20",
    ];
    let params = parse_go_params(&parts);

    assert_eq!(params.wtime, Some(300000));
    assert_eq!(params.btime, Some(300000));
    assert_eq!(params.winc, Some(3000));
    assert_eq!(params.binc, Some(3000));
    assert_eq!(params.depth, Some(20));
}

#[test]
fn parse_go_params_invalid_value() {
    let parts: Vec<&str> = vec!["go", "depth", "invalid"];
    let params = parse_go_params(&parts);

    assert!(params.depth.is_none());
}

#[test]
fn parse_go_params_recovers_keyword_after_invalid_numeric_value() {
    let parts: Vec<&str> = vec!["go", "depth", "infinite"];
    let params = parse_go_params(&parts);

    assert!(params.depth.is_none());
    assert!(params.infinite);
}

#[test]
fn parse_go_params_missing_value() {
    let parts: Vec<&str> = vec!["go", "depth"];
    let params = parse_go_params(&parts);

    assert!(params.depth.is_none());
}

#[test]
fn parse_go_params_recovers_keyword_after_missing_numeric_value() {
    let parts: Vec<&str> = vec!["go", "depth", "ponder"];
    let params = parse_go_params(&parts);

    assert!(params.depth.is_none());
    assert!(params.ponder);
}

#[test]
fn parse_go_params_unknown_skipped() {
    let parts: Vec<&str> = vec!["go", "unknownparam", "depth", "10"];
    let params = parse_go_params(&parts);

    assert_eq!(params.depth, Some(10));
}
