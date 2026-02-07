use std::time::Instant;

use super::{SimpleSearchContext, MATE_SCORE, MATE_THRESHOLD};
use crate::board::search::SearchInfoCallback;
use crate::board::{Move, SearchIterationInfo, SearchState, EMPTY_MOVE, MAX_PLY};
use std::sync::atomic::AtomicBool;

impl SimpleSearchContext<'_> {
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

        let elapsed = self.start_time.elapsed().as_millis() as u64;

        // Base soft time, adjusted for stability and score changes
        let mut adjusted_soft_time = soft_time_ms;
        if stability_count < 3 {
            adjusted_soft_time = adjusted_soft_time.saturating_mul(130) / 100;
        } else if stability_count >= 5 {
            adjusted_soft_time = adjusted_soft_time.saturating_mul(80) / 100;
        }
        if score < previous_score - 30 {
            adjusted_soft_time = adjusted_soft_time.saturating_mul(140) / 100;
        }

        // Node-based time check: estimate if we can complete the next depth
        if elapsed > 0 && prev_iter_nodes > 5000 && depth > 5 {
            let nps = self.nodes * 1000 / elapsed;
            if nps > 0 {
                let estimated_nodes = prev_iter_nodes.saturating_mul(25) / 10;
                let estimated_time = estimated_nodes * 1000 / nps;
                let remaining = self.time_limit_ms.saturating_sub(elapsed);
                if estimated_time > remaining * 2 {
                    return true;
                }
            }
        }

        elapsed >= adjusted_soft_time
    }

    /// Iterative deepening with aspiration windows and time management.
    /// Uses `self.root_moves` for the moves to consider at root.
    /// `multipv_index`: which PV line this is (1 = best, 2 = second best, etc.)
    #[allow(clippy::too_many_lines)]
    pub fn iterative_deepening_multipv(
        &mut self,
        max_depth: u32,
        multipv_index: u32,
    ) -> Option<Move> {
        let mut best_move: Option<Move> = None;
        let mut score = self.evaluate();

        // Time management state
        let mut previous_best_move: Option<Move> = None;
        let mut previous_score = score;
        let mut stability_count = 0u32;
        let mut prev_iter_nodes = 0u64;

        // Soft time limit is ~40% of hard limit (can be exceeded for good reasons)
        let soft_time_ms = self.time_limit_ms * 40 / 100;

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
                stability_count,
                score,
                previous_score,
                prev_iter_nodes,
            ) {
                break;
            }

            self.initial_depth = depth;

            // Aspiration window - fixed delta, stability adjustments removed
            let mut delta = if depth <= 5 {
                35
            } else {
                20
            };

            let mut alpha = score.saturating_sub(delta);
            let mut beta = score.saturating_add(delta);

            loop {
                let new_score =
                    self.alphabeta(depth, alpha, beta, true, 0, crate::board::EMPTY_MOVE);

                if self.should_stop() {
                    break;
                }

                // If we found a mate score, accept it immediately
                if new_score.abs() >= MATE_THRESHOLD {
                    score = new_score;
                    break;
                }

                if new_score >= beta {
                    // Fail high - widen beta
                    beta = beta.saturating_add(delta);
                    delta = delta.saturating_mul(3) / 2; // Grow by 1.5x instead of 2x
                } else if new_score <= alpha {
                    // Fail low - widen alpha more aggressively
                    alpha = alpha.saturating_sub(delta);
                    delta = delta.saturating_mul(2); // Fail low is more critical, widen faster
                } else {
                    score = new_score;
                    break;
                }

                // Prevent infinite loop - fall back to full window
                if delta > 800 {
                    alpha = -30000;
                    beta = 30000;
                }
            }

            // Get best move from TT
            if let Some(entry) = self.state.tables.tt.probe(self.board.hash) {
                if let Some(mv) = entry.best_move() {
                    if mv != EMPTY_MOVE {
                        // Verify move is in our root_moves (already filtered for MultiPV)
                        if self.root_moves.contains(&mv) {
                            best_move = Some(mv);
                        }
                    }
                }
            }

            // Update stability tracking for time management
            if best_move == previous_best_move && best_move.is_some() {
                stability_count = stability_count.saturating_add(1);
            } else {
                stability_count = 0;
            }
            previous_best_move = best_move;
            previous_score = score;

            // Track nodes for this iteration (for node-based time scaling)
            prev_iter_nodes = self.nodes.saturating_sub(iter_start_nodes);

            // Extract PV from TT, ensuring first move is our best_move
            let pv = if let Some(bm) = best_move {
                self.extract_pv_with_first_move(bm, depth as usize)
            } else {
                self.extract_pv(depth as usize)
            };
            let pv_str = Self::format_pv(&pv);

            if let Some(cb) = &self.info_callback {
                let elapsed = self.start_time.elapsed().as_millis() as u64;
                let nps = if elapsed > 0 {
                    self.nodes * 1000 / elapsed
                } else {
                    0
                };
                let mate_in = if score.abs() < MATE_THRESHOLD {
                    None
                } else if score > 0 {
                    Some((MATE_SCORE - score + 1) / 2)
                } else {
                    Some(-(MATE_SCORE + score + 1) / 2)
                };
                let info = SearchIterationInfo {
                    depth,
                    nodes: self.nodes,
                    nps,
                    time_ms: elapsed,
                    score,
                    mate_in,
                    pv: pv_str,
                    seldepth: self.state.stats.seldepth,
                    tt_hits: self.state.stats.tt_hits,
                    multipv: multipv_index,
                };
                cb(&info);
            }
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
        max_depth,
        time_limit_ms,
        node_limit,
        stop,
        info_callback,
        &[],
        1,
    )
}

/// Run the main search algorithm with `MultiPV` support
#[allow(clippy::too_many_arguments)]
pub fn simple_search_multipv(
    board: &mut crate::board::Board,
    state: &mut SearchState,
    max_depth: u32,
    time_limit_ms: u64,
    node_limit: u64,
    stop: &AtomicBool,
    info_callback: Option<SearchInfoCallback>,
    excluded_moves: &[Move],
    multipv_index: u32,
) -> Option<Move> {
    // Increment generation for TT aging (only on first PV line)
    if multipv_index == 1 {
        state.generation = state.generation.wrapping_add(1);
    }

    // Check for single legal move
    let moves = board.generate_moves();

    // Filter out excluded moves (for MultiPV)
    let available_moves: Vec<Move> = moves
        .iter()
        .filter(|m| !excluded_moves.contains(m))
        .copied()
        .collect();

    if available_moves.is_empty() {
        return None;
    }
    if available_moves.len() == 1 {
        return Some(available_moves[0]);
    }

    let mut ctx = SimpleSearchContext {
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
        root_moves: available_moves,
    };

    let result = ctx.iterative_deepening_multipv(max_depth, multipv_index);

    ctx.state.stats.nodes = ctx.nodes;
    ctx.state.stats.total_nodes = ctx.state.stats.total_nodes.saturating_add(ctx.nodes);

    result
}
