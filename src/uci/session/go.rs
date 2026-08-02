use crate::board::search::DEFAULT_MAX_DEPTH;
use crate::engine::time::{build_search_request, TimeConfig};
use crate::engine::SearchParams as EngineSearchParams;
use crate::uci::command::GoParams;
use crate::uci::print::print_time_info;
use crate::uci::report::print_bestmove_with_ponder;

use super::UciSession;

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
        let time_control = self.state.update_time_control(params, is_white);
        let depth = requested_depth(params);
        let go_ponder = params.ponder;
        let go_infinite = params.infinite;
        let nodes = (!go_infinite).then_some(params.nodes).flatten();

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
        self.engine.set_trace(self.state.debug);

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

        let is_checkmate = self.engine.board_mut().is_checkmate();
        let is_stalemate = self.engine.board_mut().is_stalemate();
        let is_draw = self.engine.board().is_draw();

        self.engine.start_search(plan.search_params, move |result| {
            if result.best_move.is_none() {
                if is_checkmate {
                    println!("info score mate -1");
                } else if is_stalemate || is_draw {
                    println!("info score cp 0");
                }
            }
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
}
