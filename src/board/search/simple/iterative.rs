use std::time::Instant;

use super::{SimpleSearchContext, MATE_SCORE, MATE_THRESHOLD};
use crate::board::search::SearchInfoCallback;
use crate::board::{Move, SearchIterationInfo, SearchState, EMPTY_MOVE, MAX_PLY};
use std::sync::atomic::AtomicBool;

impl SimpleSearchContext<'_> {
    /// Iterative deepening with aspiration windows and time management
    #[allow(clippy::too_many_lines)]
    pub fn iterative_deepening(&mut self, max_depth: u32) -> Option<Move> {
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
            if depth > 4 && self.time_limit_ms > 0 {
                let elapsed = self.start_time.elapsed().as_millis() as u64;

                // Base soft time check
                let mut adjusted_soft_time = soft_time_ms;

                // Extend time if best move changed recently (unstable)
                if stability_count < 3 {
                    adjusted_soft_time = adjusted_soft_time.saturating_mul(130) / 100;
                } else if stability_count >= 5 {
                    // Reduce time if very stable
                    adjusted_soft_time = adjusted_soft_time.saturating_mul(80) / 100;
                }

                // Extend time if score dropped significantly
                if score < previous_score - 30 {
                    adjusted_soft_time = adjusted_soft_time.saturating_mul(140) / 100;
                }

                // Node-based time check: estimate if we can complete the next depth
                // Only apply after we have reliable node counts (depth > 5, prev_iter > 5000)
                if elapsed > 0 && prev_iter_nodes > 5000 && depth > 5 {
                    let nps = self.nodes * 1000 / elapsed;
                    if nps > 0 {
                        // Estimate nodes for this depth (branching factor ~2.5 typically)
                        let estimated_nodes = prev_iter_nodes.saturating_mul(25) / 10;
                        let estimated_time = estimated_nodes * 1000 / nps;
                        let remaining = self.time_limit_ms.saturating_sub(elapsed);

                        // Only abort if we're very confident we can't finish (need 2x remaining time)
                        if estimated_time > remaining * 2 {
                            break;
                        }
                    }
                }

                if elapsed >= adjusted_soft_time {
                    break;
                }
            }

            self.initial_depth = depth;

            // Aspiration window
            let mut delta = 30;
            let mut alpha = score - delta;
            let mut beta = score + delta;

            loop {
                let new_score = self.alphabeta(depth, alpha, beta, true, 0, crate::board::EMPTY_MOVE);

                if self.should_stop() {
                    break;
                }

                // If we found a mate score, accept it immediately
                if new_score.abs() >= MATE_THRESHOLD {
                    score = new_score;
                    break;
                }

                if new_score >= beta {
                    beta = beta.saturating_add(delta);
                    delta = delta.saturating_add(delta);
                } else if new_score <= alpha {
                    alpha = alpha.saturating_sub(delta);
                    delta = delta.saturating_add(delta);
                } else {
                    score = new_score;
                    break;
                }

                // Prevent infinite loop with very wide window
                if delta > 1000 {
                    alpha = -30000;
                    beta = 30000;
                }
            }

            // Get best move from TT
            if let Some(entry) = self.state.tables.tt.probe(self.board.hash) {
                if let Some(mv) = entry.best_move() {
                    if mv != EMPTY_MOVE {
                        // Verify move is legal
                        let moves = self.board.generate_moves();
                        if moves.iter().any(|m| *m == mv) {
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

            // Extract PV from TT
            let pv = self.extract_pv(depth as usize);
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
    // Increment generation for TT aging
    state.generation = state.generation.wrapping_add(1);

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
    };

    // Check for single legal move
    let moves = ctx.board.generate_moves();
    let result = if moves.is_empty() {
        None
    } else if moves.len() == 1 {
        moves.first()
    } else {
        ctx.iterative_deepening(max_depth)
    };

    ctx.state.stats.nodes = ctx.nodes;
    ctx.state.stats.total_nodes = ctx.state.stats.total_nodes.saturating_add(ctx.nodes);

    result
}
