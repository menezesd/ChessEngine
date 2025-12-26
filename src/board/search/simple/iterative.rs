use std::time::Instant;

use super::{SimpleSearchContext, MATE_SCORE, MATE_THRESHOLD};
use crate::board::search::SearchInfoCallback;
use crate::board::{Move, SearchIterationInfo, SearchState, EMPTY_MOVE, MAX_PLY};
use std::sync::atomic::AtomicBool;

impl SimpleSearchContext<'_> {
    /// Iterative deepening with aspiration windows
    pub fn iterative_deepening(&mut self, max_depth: u32) -> Option<Move> {
        let mut best_move: Option<Move> = None;
        let mut score = self.evaluate();

        // Reset history at start of search
        self.state.tables.reset_history();
        self.state.stats.seldepth = 0;
        self.state.stats.tt_hits = 0;

        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }

            self.initial_depth = depth;

            // Aspiration window
            let mut delta = 30;
            let mut alpha = score - delta;
            let mut beta = score + delta;

            loop {
                let new_score = self.alphabeta(depth, alpha, beta, true, 0, false);

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
