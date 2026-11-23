use crate::engine::time::TimeControl;
use crate::uci::command::GoParams;

const FALLBACK_TIME_MS: u64 = 5_000;

pub(super) fn go_time_control(params: &GoParams, is_white: bool) -> TimeControl {
    if params.infinite {
        return TimeControl::Infinite;
    }
    if let Some(ms) = params.movetime {
        return TimeControl::move_time_ms(ms);
    }

    let (remaining, increment) = if is_white {
        (params.wtime, params.winc)
    } else {
        (params.btime, params.binc)
    };
    let fixed_search = params.depth.is_some()
        || params.nodes.is_some()
        || params.mate.is_some_and(|mate| mate > 0);
    // A depth, node, or mate limit can apply together with the game clock.
    if fixed_search && remaining.is_none() {
        return TimeControl::Depth;
    }
    // Bare go keeps searching until the GUI sends stop.
    if params.wtime.is_none()
        && params.btime.is_none()
        && params.winc.is_none()
        && params.binc.is_none()
        && params.movestogo.is_none()
    {
        return TimeControl::Infinite;
    }

    TimeControl::Incremental {
        time_left_ms: remaining.unwrap_or(FALLBACK_TIME_MS),
        inc_ms: increment.unwrap_or(0),
        movestogo: params.movestogo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn go_time_control_uses_white_clock() {
        let params = GoParams {
            wtime: Some(12_000),
            btime: Some(34_000),
            winc: Some(500),
            binc: Some(900),
            ..GoParams::default()
        };

        let time_control = go_time_control(&params, true);
        assert_eq!(
            time_control,
            TimeControl::incremental(Duration::from_secs(12), Duration::from_millis(500), None)
        );
    }

    #[test]
    fn go_time_control_uses_black_clock() {
        let params = GoParams {
            wtime: Some(12_000),
            btime: Some(34_000),
            winc: Some(500),
            binc: Some(900),
            ..GoParams::default()
        };

        let time_control = go_time_control(&params, false);
        assert_eq!(
            time_control,
            TimeControl::incremental(Duration::from_secs(34), Duration::from_millis(900), None)
        );
    }

    #[test]
    fn go_time_control_prefers_movetime() {
        let params = GoParams {
            movetime: Some(1_250),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(
            go_time_control(&params, true),
            TimeControl::move_time(Duration::from_millis(1_250))
        );
    }

    #[test]
    fn go_time_control_keeps_clock_for_ponder_search() {
        let params = GoParams {
            ponder: true,
            wtime: Some(12_000),
            winc: Some(500),
            ..GoParams::default()
        };

        assert_eq!(
            go_time_control(&params, true),
            TimeControl::incremental(Duration::from_secs(12), Duration::from_millis(500), None)
        );
    }

    #[test]
    fn go_time_control_uses_infinite_for_infinite_search() {
        let params = GoParams {
            infinite: true,
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(go_time_control(&params, true), TimeControl::Infinite);
    }

    #[test]
    fn go_time_control_uses_infinite_when_no_limit_is_given() {
        assert_eq!(
            go_time_control(&GoParams::default(), true),
            TimeControl::Infinite
        );

        let params = GoParams {
            searchmoves: Some(vec!["e2e4".to_string()]),
            ..GoParams::default()
        };
        assert_eq!(go_time_control(&params, true), TimeControl::Infinite);
    }

    #[test]
    fn go_time_control_keeps_clock_for_fixed_depth_search() {
        let params = GoParams {
            depth: Some(4),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(
            go_time_control(&params, true),
            TimeControl::incremental(Duration::from_secs(12), Duration::ZERO, None)
        );
    }

    #[test]
    fn go_time_control_keeps_clock_for_node_limited_search() {
        let params = GoParams {
            nodes: Some(10_000),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(
            go_time_control(&params, true),
            TimeControl::incremental(Duration::from_secs(12), Duration::ZERO, None)
        );
    }

    #[test]
    fn go_time_control_keeps_clock_for_mate_limited_search() {
        let params = GoParams {
            mate: Some(2),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(
            go_time_control(&params, true),
            TimeControl::incremental(Duration::from_secs(12), Duration::ZERO, None)
        );
    }

    #[test]
    fn go_time_control_uses_depth_for_fixed_search_without_clock() {
        let params = GoParams {
            depth: Some(4),
            ..GoParams::default()
        };

        assert_eq!(go_time_control(&params, true), TimeControl::Depth);
    }
}
