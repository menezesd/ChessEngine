use std::time::Instant;

mod aspiration;
mod progress;

use aspiration::AspirationWindow;
use progress::SearchProgress;

use super::{SimpleSearchContext, MATE_SCORE, MATE_THRESHOLD, SCORE_INFINITE};
use crate::board::nnue::NnueAccumulator;
use crate::board::search::SearchInfoCallback;
use crate::board::{Board, Move, SearchIterationInfo, SearchState, EMPTY_MOVE, MAX_PLY};
use std::sync::atomic::AtomicBool;

const MILLISECONDS_PER_SECOND: u64 = 1000;
const PERCENT_SCALE: u64 = 100;
const UNSTABLE_BEST_MOVE_TIME_PERCENT: u64 = 130;
const STABLE_BEST_MOVE_TIME_PERCENT: u64 = 80;
const SCORE_DROP_TIME_PERCENT: u64 = 140;
const SOFT_TIME_LIMIT_PERCENT: u64 = 40;
const NEXT_DEPTH_NODE_ESTIMATE_NUMERATOR: u64 = 25;
const NEXT_DEPTH_NODE_ESTIMATE_DENOMINATOR: u64 = 10;
const NEXT_DEPTH_MIN_PREVIOUS_NODES: u64 = 5000;
const NEXT_DEPTH_REMAINING_TIME_MULTIPLIER: u64 = 2;
const ACCUMULATOR_STACK_EXTRA_PLY: usize = 64;

fn search_context<'a>(
    board: &'a mut Board,
    state: &'a mut SearchState,
    stop: &'a AtomicBool,
    time_limit_ms: u64,
    node_limit: u64,
    info_callback: Option<SearchInfoCallback>,
    root_moves: Vec<Move>,
) -> SimpleSearchContext<'a> {
    SimpleSearchContext {
        board,
        state,
        stop,
        start_time: Instant::now(),
        time_limit_ms,
        node_limit,
        nodes: 0,
        initial_depth: 1,
        static_eval: [0; MAX_PLY],
        previous_move: [EMPTY_MOVE; MAX_PLY],
        previous_piece: [None; MAX_PLY],
        info_callback,
        root_moves,
        acc_stack: vec![NnueAccumulator::default(); MAX_PLY + ACCUMULATOR_STACK_EXTRA_PLY]
            .into_boxed_slice(),
        static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + ACCUMULATOR_STACK_EXTRA_PLY]
            .into_boxed_slice(),
    }
}

fn available_root_moves(board: &mut Board, excluded_moves: &[Move]) -> Vec<Move> {
    board
        .generate_moves()
        .iter()
        .filter(|m| !excluded_moves.contains(m))
        .copied()
        .collect()
}

fn record_search_nodes(ctx: &mut SimpleSearchContext<'_>) {
    ctx.state.stats.nodes = ctx.nodes;
    ctx.state.stats.total_nodes = ctx.state.stats.total_nodes.saturating_add(ctx.nodes);
}

fn nodes_per_second(nodes: u64, elapsed_ms: u64) -> u64 {
    nodes
        .saturating_mul(MILLISECONDS_PER_SECOND)
        .checked_div(elapsed_ms)
        .unwrap_or_default()
}

fn scale_time_by_percent(time_ms: u64, percent: u64) -> u64 {
    time_ms.saturating_mul(percent) / PERCENT_SCALE
}

fn mate_distance(score: i32) -> Option<i32> {
    if score.abs() < MATE_THRESHOLD {
        return None;
    }

    let mate_score = i64::from(MATE_SCORE);
    let score = i64::from(score);
    let distance = if score > 0 {
        (mate_score - score + 1) / 2
    } else {
        -((mate_score + score + 1) / 2)
    };

    Some(distance.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

pub struct SimpleSearchRequest<'a> {
    pub max_depth: u32,
    pub time_limit_ms: u64,
    pub node_limit: u64,
    pub info_callback: Option<SearchInfoCallback>,
    pub excluded_moves: &'a [Move],
    pub multipv_index: u32,
}

impl SimpleSearchRequest<'_> {
    fn single(
        max_depth: u32,
        time_limit_ms: u64,
        node_limit: u64,
        info_callback: Option<SearchInfoCallback>,
    ) -> Self {
        Self {
            max_depth,
            time_limit_ms,
            node_limit,
            info_callback,
            excluded_moves: &[],
            multipv_index: 1,
        }
    }
}

impl SimpleSearchContext<'_> {
    fn root_best_move_from_tt(&self) -> Option<Move> {
        self.state
            .tables
            .tt
            .probe(self.board.hash)
            .and_then(|entry| entry.best_move())
            .filter(|mv| *mv != EMPTY_MOVE && self.root_moves.contains(mv))
    }

    fn report_iteration(&self, depth: u32, score: i32, pv: &[Move], multipv_index: u32) {
        let Some(cb) = &self.info_callback else {
            return;
        };

        let elapsed = self.elapsed_ms();
        let nps = nodes_per_second(self.nodes, elapsed);
        let mate_in = mate_distance(score);

        let info = SearchIterationInfo {
            depth,
            nodes: self.nodes,
            nps,
            time_ms: elapsed,
            score,
            mate_in,
            pv: Self::format_pv(pv),
            seldepth: self.state.stats.seldepth,
            tt_hits: self.state.stats.tt_hits,
            multipv: multipv_index,
        };
        cb(&info);
    }

    /// Check if we should stop the current iteration based on time management.
    /// Returns true if we should stop iterating.
    fn should_stop_iteration(
        &self,
        depth: u32,
        soft_time_ms: u64,
        stability_count: u32,
        score: i32,
        previous_score: i32,
        prev_iter_nodes: u64,
    ) -> bool {
        if depth <= 4 || self.time_limit_ms == 0 {
            return false;
        }

        let elapsed = self.elapsed_ms();

        // Base soft time, adjusted for stability and score changes
        let mut adjusted_soft_time = soft_time_ms;
        if stability_count < 3 {
            adjusted_soft_time =
                scale_time_by_percent(adjusted_soft_time, UNSTABLE_BEST_MOVE_TIME_PERCENT);
        } else if stability_count >= 5 {
            adjusted_soft_time =
                scale_time_by_percent(adjusted_soft_time, STABLE_BEST_MOVE_TIME_PERCENT);
        }
        if score < previous_score - 30 {
            adjusted_soft_time = scale_time_by_percent(adjusted_soft_time, SCORE_DROP_TIME_PERCENT);
        }

        // Node-based time check: estimate if we can complete the next depth
        if elapsed > 0 && prev_iter_nodes > NEXT_DEPTH_MIN_PREVIOUS_NODES && depth > 5 {
            let nps = nodes_per_second(self.nodes, elapsed);
            if let Some(estimated_time) = prev_iter_nodes
                .saturating_mul(NEXT_DEPTH_NODE_ESTIMATE_NUMERATOR)
                .checked_div(NEXT_DEPTH_NODE_ESTIMATE_DENOMINATOR)
                .and_then(|nodes| {
                    nodes
                        .saturating_mul(MILLISECONDS_PER_SECOND)
                        .checked_div(nps)
                })
            {
                let remaining = self.time_limit_ms.saturating_sub(elapsed);
                if estimated_time > remaining.saturating_mul(NEXT_DEPTH_REMAINING_TIME_MULTIPLIER) {
                    return true;
                }
            }
        }

        elapsed >= adjusted_soft_time
    }

    fn search_depth_with_aspiration(&mut self, depth: u32, score: i32) -> Option<i32> {
        let mut window = AspirationWindow::new(depth, score);

        loop {
            let new_score = self.alphabeta(
                depth,
                window.alpha(),
                window.beta(),
                true,
                0,
                crate::board::EMPTY_MOVE,
            );

            if self.should_stop() {
                return None;
            }

            if new_score.abs() >= MATE_THRESHOLD {
                return Some(new_score);
            }

            if new_score >= window.beta() {
                window.fail_high();
            } else if new_score <= window.alpha() {
                window.fail_low();
            } else {
                return Some(new_score);
            }

            window.use_full_window_if_needed();
        }
    }

    /// Iterative deepening with aspiration windows and time management.
    /// Uses `self.root_moves` for the moves to consider at root.
    /// `multipv_index`: which PV line this is (1 = best, 2 = second best, etc.)
    pub fn iterative_deepening_multipv(
        &mut self,
        max_depth: u32,
        multipv_index: u32,
    ) -> Option<Move> {
        let mut best_move: Option<Move> = None;
        // Initialize NNUE accumulator for root position
        self.init_accumulator(0);
        let mut score = self.evaluate(0);

        let mut progress = SearchProgress::new(score);

        // Soft time limit is ~40% of hard limit (can be exceeded for good reasons)
        let soft_time_ms = scale_time_by_percent(self.time_limit_ms, SOFT_TIME_LIMIT_PERCENT);

        // Reset history at start of search
        self.state.tables.reset_history();
        self.state.stats.seldepth = 0;
        self.state.stats.tt_hits = 0;

        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }

            let iter_start_nodes = self.nodes;

            // Soft time check: if we've used enough time and have a stable best move, stop
            if self.should_stop_iteration(
                depth,
                soft_time_ms,
                progress.stability_count(),
                score,
                progress.previous_score(),
                progress.prev_iter_nodes(),
            ) {
                break;
            }

            self.initial_depth = depth;
            let Some(new_score) = self.search_depth_with_aspiration(depth, score) else {
                break;
            };
            score = new_score;

            if let Some(mv) = self.root_best_move_from_tt() {
                best_move = Some(mv);
            }

            // Update stability tracking for time management
            // Track nodes for this iteration (for node-based time scaling)
            let iter_nodes = self.nodes.saturating_sub(iter_start_nodes);
            progress.update(best_move, score, iter_nodes);

            // Extract PV from TT, ensuring first move is our best_move
            let pv = if let Some(bm) = best_move {
                self.extract_pv_with_first_move(bm, depth as usize)
            } else {
                self.extract_pv(depth as usize)
            };
            self.report_iteration(depth, score, &pv, multipv_index);
        }

        best_move
    }
}

/// Run the main search algorithm
pub fn simple_search(
    board: &mut crate::board::Board,
    state: &mut SearchState,
    max_depth: u32,
    time_limit_ms: u64,
    node_limit: u64,
    stop: &AtomicBool,
    info_callback: Option<SearchInfoCallback>,
) -> Option<Move> {
    simple_search_multipv(
        board,
        state,
        stop,
        SimpleSearchRequest::single(max_depth, time_limit_ms, node_limit, info_callback),
    )
}

/// Run the main search algorithm with `MultiPV` support
pub fn simple_search_multipv(
    board: &mut crate::board::Board,
    state: &mut SearchState,
    stop: &AtomicBool,
    request: SimpleSearchRequest<'_>,
) -> Option<Move> {
    // Increment generation for TT aging (only on first PV line)
    if request.multipv_index == 1 {
        state.generation = state.generation.wrapping_add(1);
    }

    let available_moves = available_root_moves(board, request.excluded_moves);

    if available_moves.is_empty() {
        return None;
    }
    if available_moves.len() == 1 {
        return Some(available_moves[0]);
    }

    let mut ctx = search_context(
        board,
        state,
        stop,
        request.time_limit_ms,
        request.node_limit,
        request.info_callback,
        available_moves,
    );

    let result = ctx.iterative_deepening_multipv(request.max_depth, request.multipv_index);
    record_search_nodes(&mut ctx);

    result
}

#[cfg(test)]
mod tests {
    use super::{mate_distance, nodes_per_second, scale_time_by_percent, MATE_SCORE};

    #[test]
    fn nodes_per_second_handles_zero_elapsed() {
        assert_eq!(nodes_per_second(10_000, 0), 0);
    }

    #[test]
    fn nodes_per_second_scales_nodes_by_elapsed_ms() {
        assert_eq!(nodes_per_second(10_000, 250), 40_000);
    }

    #[test]
    fn scale_time_by_percent_scales_time() {
        assert_eq!(scale_time_by_percent(1_000, 130), 1_300);
        assert_eq!(scale_time_by_percent(1_000, 80), 800);
    }

    #[test]
    fn mate_distance_reports_only_mate_scores() {
        assert_eq!(mate_distance(0), None);
        assert_eq!(mate_distance(MATE_SCORE - 1), Some(1));
        assert_eq!(mate_distance(-MATE_SCORE + 1), Some(-1));
    }
}
