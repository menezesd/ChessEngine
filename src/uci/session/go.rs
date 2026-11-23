use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::engine::time::{build_search_request, TimeConfig, TimeControl};
use crate::engine::SearchParams as EngineSearchParams;
use crate::uci::command::GoParams;
use crate::uci::print::print_time_info;
use crate::uci::report::print_bestmove_with_ponder;

use super::{time_control::go_time_control, UciSession};

const PLIES_PER_MATE_MOVE: u32 = 2;

pub(super) struct GoSearchPlan {
    search_params: EngineSearchParams,
    soft_time_ms: u64,
    hard_time_ms: u64,
    depth_hint: Option<u32>,
    go_ponder: bool,
    max_nodes: u64,
}

impl UciSession {
    pub(super) fn build_go_plan(&mut self, params: &GoParams, is_white: bool) -> GoSearchPlan {
        let time_control = go_time_control(params, is_white);
        let depth = requested_depth(params).or_else(|| {
            // With a game clock, a forced reply needs only enough search to
            // seed a continuation. Explicit analysis limits still apply.
            (matches!(time_control, TimeControl::Incremental { .. })
                && !params.ponder
                && params.searchmoves.is_none()
                && self.engine.board().has_single_legal_move())
            .then_some(1)
        });
        let go_ponder = params.ponder;
        // An unconstrained bare `go` has the same lifecycle as `go infinite`:
        // even if the finite internal depth cap is reached, wait for `stop`
        // before publishing a result.
        let go_infinite = matches!(time_control, TimeControl::Infinite);
        let nodes = if go_infinite { None } else { params.nodes };

        let time_config = TimeConfig {
            move_overhead_ms: self.options.move_overhead_ms,
            soft_time_percent: self.options.soft_time_percent,
            hard_time_percent: self.options.hard_time_percent,
            default_max_nodes: self.options.default_max_nodes,
        };
        let (request, (soft_time_ms, hard_time_ms)) = build_search_request(
            time_control,
            depth,
            nodes,
            go_ponder,
            go_infinite,
            &time_config,
        );

        let search_params = EngineSearchParams {
            depth: request.depth,
            soft_time_ms: request.soft_time_ms,
            hard_time_ms: request.hard_time_ms,
            ponder: request.ponder,
            infinite: request.infinite,
            multi_pv: self.options.multi_pv,
            root_moves: params.searchmoves.as_ref().map(|moves| {
                let board = self.engine.board_mut();
                moves
                    .iter()
                    .filter_map(|mv| board.parse_move(mv).ok())
                    .collect()
            }),
        };

        GoSearchPlan {
            search_params,
            soft_time_ms,
            hard_time_ms,
            depth_hint: depth,
            go_ponder,
            max_nodes: request.max_nodes,
        }
    }

    /// Handle the "go" command - start a search.
    pub(super) fn handle_go(&mut self, params: &GoParams) {
        // Stop any active search before touching the search state: the
        // search thread holds the state lock while running, so calling
        // `set_max_nodes` first would block the protocol loop forever.
        self.engine.stop_search();
        // Re-apply the debug flag in case `debug` arrived mid-search, when
        // the non-blocking `set_trace` cannot reach the locked state.
        self.engine.set_trace(self.debug);

        let plan = self.build_go_plan(params, self.engine.board().white_to_move());

        self.engine.set_max_nodes(plan.max_nodes);

        print_time_info(
            plan.soft_time_ms,
            plan.hard_time_ms,
            self.options.move_overhead_ms,
            plan.max_nodes,
            plan.go_ponder,
            plan.depth_hint,
        );

        self.engine.start_search(plan.search_params, move |result| {
            print_bestmove_with_ponder(result);
        });
    }
}

fn requested_depth(params: &GoParams) -> Option<u32> {
    if params.infinite {
        return None;
    }

    params
        .depth
        .map(clamp_requested_depth)
        .or_else(|| params.nodes.map(|_| DEFAULT_MAX_DEPTH))
        .or_else(|| {
            params
                .mate
                .filter(|mate_moves| *mate_moves > 0)
                .map(|mate| clamp_requested_depth(mate.saturating_mul(PLIES_PER_MATE_MOVE)))
        })
}

fn clamp_requested_depth(depth: u32) -> u32 {
    depth.min(DEFAULT_MAX_DEPTH)
}

#[cfg(test)]
mod tests {
    use super::{requested_depth, UciSession};
    use crate::board::search::DEFAULT_MAX_DEPTH;
    use crate::uci::command::GoParams;

    #[test]
    fn only_reply_on_a_game_clock_gets_a_short_search() {
        let mut session = UciSession::new(1);
        session.engine.set_board(crate::board::Board::from_fen(
            "7k/5K2/8/8/8/8/8/R7 b - - 0 1",
        ));
        let mut params = GoParams {
            wtime: Some(10_000),
            btime: Some(10_000),
            ..Default::default()
        };
        assert_eq!(
            session.build_go_plan(&params, false).search_params.depth,
            Some(1)
        );
        params.depth = Some(8);
        assert_eq!(
            session.build_go_plan(&params, false).search_params.depth,
            Some(8)
        );
        params.depth = None;
        params.ponder = true;
        assert_eq!(
            session.build_go_plan(&params, false).search_params.depth,
            None
        );
        params.ponder = false;
        params.movetime = Some(200);
        assert_eq!(
            session.build_go_plan(&params, false).search_params.depth,
            None
        );
    }

    #[test]
    fn requested_depth_prefers_explicit_depth() {
        let params = GoParams {
            depth: Some(7),
            nodes: Some(100),
            mate: Some(2),
            ..Default::default()
        };

        assert_eq!(requested_depth(&params), Some(7));
    }

    #[test]
    fn requested_depth_clamps_extreme_explicit_depth() {
        let params = GoParams {
            depth: Some(u32::MAX),
            ..Default::default()
        };

        assert_eq!(requested_depth(&params), Some(DEFAULT_MAX_DEPTH));
    }

    #[test]
    fn requested_depth_uses_default_depth_for_node_limited_search() {
        let params = GoParams {
            nodes: Some(100),
            mate: Some(2),
            ..Default::default()
        };

        assert_eq!(requested_depth(&params), Some(DEFAULT_MAX_DEPTH));
    }

    #[test]
    fn requested_depth_converts_mate_moves_to_plies() {
        let params = GoParams {
            mate: Some(3),
            ..Default::default()
        };

        assert_eq!(requested_depth(&params), Some(6));
    }

    #[test]
    fn requested_depth_clamps_extreme_mate_depth() {
        let params = GoParams {
            mate: Some(u32::MAX),
            ..Default::default()
        };

        assert_eq!(requested_depth(&params), Some(DEFAULT_MAX_DEPTH));
    }

    #[test]
    fn requested_depth_ignores_zero_mate_depth() {
        let params = GoParams {
            mate: Some(0),
            ..Default::default()
        };

        assert_eq!(requested_depth(&params), None);
    }

    #[test]
    fn requested_depth_ignores_limits_for_infinite_search() {
        let params = GoParams {
            infinite: true,
            depth: Some(7),
            nodes: Some(1_000),
            mate: Some(3),
            ..GoParams::default()
        };

        assert_eq!(requested_depth(&params), None);

        let mut session = UciSession::new(1);
        let plan = session.build_go_plan(&params, true);
        assert!(plan.search_params.infinite);
        assert_eq!(plan.search_params.depth, None);
        assert_eq!(plan.max_nodes, 0);
    }

    #[test]
    fn bare_go_is_unlimited_until_stop() {
        let mut session = UciSession::new(1);
        let plan = session.build_go_plan(&GoParams::default(), true);

        assert!(plan.search_params.infinite);
        assert_eq!(plan.search_params.depth, None);
        assert_eq!(plan.search_params.soft_time_ms, 0);
        assert_eq!(plan.search_params.hard_time_ms, 0);
        assert_eq!(plan.max_nodes, 0);
    }
}
