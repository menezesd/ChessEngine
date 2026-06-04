use std::time::Duration;

use crate::engine::time::TimeControl;
use crate::uci::command::GoParams;

pub(super) const FALLBACK_TIME_SECS: u64 = 5;

/// UCI session state (time controls, debug mode).
pub(super) struct UciState {
    pub(super) time_control: TimeControl,
    pub(super) debug: bool,
}

impl Default for UciState {
    fn default() -> Self {
        UciState {
            time_control: TimeControl::move_time(Duration::from_secs(FALLBACK_TIME_SECS)),
            debug: false,
        }
    }
}

impl UciState {
    pub(super) fn update_time_control(&mut self, params: &GoParams, is_white: bool) -> TimeControl {
        self.time_control = go_time_control(params, is_white);
        self.time_control
    }
}

fn go_time_control(params: &GoParams, is_white: bool) -> TimeControl {
    if params.infinite {
        return TimeControl::Infinite;
    }

    if let Some(mt) = params.movetime {
        return TimeControl::move_time(Duration::from_millis(mt));
    }

    if is_fixed_search(params) {
        return TimeControl::Depth;
    }

    let fallback = Duration::from_secs(FALLBACK_TIME_SECS);
    let time_left = duration_from_millis(side_time(params, is_white)).unwrap_or(fallback);
    let inc = duration_from_millis(side_increment(params, is_white)).unwrap_or(Duration::ZERO);

    TimeControl::incremental(time_left, inc, params.movestogo)
}

fn duration_from_millis(value: Option<u64>) -> Option<Duration> {
    value.map(Duration::from_millis)
}

fn is_fixed_search(params: &GoParams) -> bool {
    params.depth.is_some() || params.nodes.is_some() || params.mate.is_some_and(|mate| mate > 0)
}

fn side_time(params: &GoParams, is_white: bool) -> Option<u64> {
    if is_white {
        params.wtime
    } else {
        params.btime
    }
}

fn side_increment(params: &GoParams, is_white: bool) -> Option<u64> {
    if is_white {
        params.winc
    } else {
        params.binc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_time_control_uses_white_clock() {
        let mut state = UciState::default();
        let params = GoParams {
            wtime: Some(12_000),
            btime: Some(34_000),
            winc: Some(500),
            binc: Some(900),
            ..GoParams::default()
        };

        let time_control = state.update_time_control(&params, true);
        assert_eq!(
            time_control,
            TimeControl::incremental(Duration::from_secs(12), Duration::from_millis(500), None)
        );
    }

    #[test]
    fn update_time_control_uses_black_clock() {
        let mut state = UciState::default();
        let params = GoParams {
            wtime: Some(12_000),
            btime: Some(34_000),
            winc: Some(500),
            binc: Some(900),
            ..GoParams::default()
        };

        let time_control = state.update_time_control(&params, false);
        assert_eq!(
            time_control,
            TimeControl::incremental(Duration::from_secs(34), Duration::from_millis(900), None)
        );
    }

    #[test]
    fn update_time_control_prefers_movetime() {
        let mut state = UciState::default();
        let params = GoParams {
            movetime: Some(1_250),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(
            state.update_time_control(&params, true),
            TimeControl::move_time(Duration::from_millis(1_250))
        );
    }

    #[test]
    fn update_time_control_keeps_clock_for_ponder_search() {
        let mut state = UciState::default();
        let params = GoParams {
            ponder: true,
            wtime: Some(12_000),
            winc: Some(500),
            ..GoParams::default()
        };

        assert_eq!(
            state.update_time_control(&params, true),
            TimeControl::incremental(Duration::from_secs(12), Duration::from_millis(500), None)
        );
    }

    #[test]
    fn update_time_control_uses_infinite_for_infinite_search() {
        let mut state = UciState::default();
        let params = GoParams {
            infinite: true,
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(
            state.update_time_control(&params, true),
            TimeControl::Infinite
        );
    }

    #[test]
    fn update_time_control_uses_depth_for_fixed_depth_search() {
        let mut state = UciState::default();
        let params = GoParams {
            depth: Some(4),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(state.update_time_control(&params, true), TimeControl::Depth);
    }

    #[test]
    fn update_time_control_uses_depth_for_node_limited_search() {
        let mut state = UciState::default();
        let params = GoParams {
            nodes: Some(10_000),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(state.update_time_control(&params, true), TimeControl::Depth);
    }

    #[test]
    fn update_time_control_uses_depth_for_mate_limited_search() {
        let mut state = UciState::default();
        let params = GoParams {
            mate: Some(2),
            wtime: Some(12_000),
            ..GoParams::default()
        };

        assert_eq!(state.update_time_control(&params, true), TimeControl::Depth);
    }
}
