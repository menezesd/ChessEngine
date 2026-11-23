use std::fmt::Write;
use std::time::Instant;

use super::evaluation::accumulator_stacks;
use super::{
    PartialRoot, SearchLine, SimpleSearchContext, MATE_SCORE, MATE_THRESHOLD, SCORE_INFINITE,
};
use crate::board::draw::FIFTY_MOVE_RULE_HALFMOVES;
use crate::board::search::{endgame, SearchConfig, DEFAULT_MAX_DEPTH};
use crate::board::{Board, Move, Piece, SearchIterationInfo, SearchState, EMPTY_MOVE, MAX_PLY};
use std::sync::atomic::AtomicBool;

const SHORT_TIME_FUTILITY_LIMIT_MS: u64 = 300;
const SHORT_TIME_FUTILITY_MARGIN: i32 = 130;

/// Aspiration window constants
const ASPIRATION_DELTA_SHALLOW: i32 = 35; // Initial delta for depth <= 5
const ASPIRATION_DELTA_DEEP: i32 = 20; // Initial delta for depth > 5
const ASPIRATION_MAX_DELTA: i32 = 800; // Fall back to full window above this
const MATE_IN_ONE_SCORE: i32 = MATE_SCORE - 1;

#[inline]
fn clamp_search_depth(max_depth: u32) -> u32 {
    max_depth.min(DEFAULT_MAX_DEPTH)
}

#[inline]
fn effective_futility_margin(time_limit_ms: u64, configured_margin: i32) -> i32 {
    if time_limit_ms > 0 && time_limit_ms <= SHORT_TIME_FUTILITY_LIMIT_MS {
        configured_margin.min(SHORT_TIME_FUTILITY_MARGIN)
    } else {
        configured_margin
    }
}

/// Prove a move's result when one or two reversible plies remain.
fn outcome_near_fifty_moves(board: &mut Board, mv: Move) -> Option<i32> {
    let undo = board.make_move(mv);
    let outcome = if board.is_checkmate() {
        Some(1)
    } else if board.is_theoretical_draw() {
        Some(0)
    } else if board.halfmove_clock() == 0 {
        endgame::score(board, 0, true).map(|score| -score.signum())
    } else {
        debug_assert_eq!(board.halfmove_clock(), FIFTY_MOVE_RULE_HALFMOVES - 1);
        let replies = board.generate_moves();
        let mut best = if replies.is_empty() { 0 } else { -1 };
        let mut unresolved = false;
        for reply in replies {
            match outcome_near_fifty_moves(board, reply) {
                Some(1) => {
                    best = 1;
                    break;
                }
                Some(outcome) => best = best.max(outcome),
                None => unresolved = true,
            }
        }
        (!unresolved || best == 1).then_some(-best)
    };
    board.unmake_move(mv, undo);
    outcome
}

impl SimpleSearchContext<'_> {
    /// Check if we should stop the current iteration based on time management.
    /// Returns true if we should stop iterating.
    ///
    /// Reads the live time budget so deadlines installed by `ponderhit`
    /// govern the iterations that follow it.
    fn should_stop_iteration(
        &self,
        depth: u32,
        stability_count: u32,
        score: i32,
        previous_score: i32,
        prev_iter_nodes: u64,
    ) -> bool {
        let (elapsed, soft_time_ms, hard_time_ms) = self.time_budget_ms();
        // With a single effective deadline (such as UCI movetime), there
        // is no separate soft target from which to save time for later.
        if depth <= 4 || soft_time_ms == 0 || (hard_time_ms > 0 && hard_time_ms <= soft_time_ms) {
            return false;
        }

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
        if prev_iter_nodes > 5000 && depth > 5 {
            // Pondered nodes are included in self.nodes, so measure their
            // throughput with the time spent before the live clock reset.
            let measured_ms = elapsed.max(self.start_time.elapsed().as_millis() as u64);
            let nps = self.nodes.saturating_mul(1000) / measured_ms.max(1);
            if let Some(estimated_time) = {
                let estimated_nodes = prev_iter_nodes.saturating_mul(25) / 10;
                estimated_nodes.saturating_mul(1000).checked_div(nps)
            } {
                let remaining = adjusted_soft_time.saturating_sub(elapsed);
                if estimated_time > remaining.saturating_mul(2) {
                    return true;
                }
            }
        }

        elapsed >= adjusted_soft_time
    }

    /// Search one root domain to completion. A score outside the aspiration
    /// window is a bound, so widen the window before publishing the result.
    fn search_iteration(&mut self, depth: u32, guess: i32) -> Option<i32> {
        self.root_best_move = None;
        let mut delta = if depth <= 5 {
            ASPIRATION_DELTA_SHALLOW
        } else {
            ASPIRATION_DELTA_DEEP
        };
        let mut alpha = guess.saturating_sub(delta);
        let mut beta = guess.saturating_add(delta).min(MATE_IN_ONE_SCORE);

        loop {
            if let Some(progress) = &mut self.partial_root {
                progress.searched_incumbent = false;
            }
            let score = self.alphabeta(depth, alpha, beta, true, 0, EMPTY_MOVE);
            if self.should_stop() {
                return None;
            }
            // A winning bound at the best possible root score is exact.
            if score == MATE_IN_ONE_SCORE {
                return Some(score);
            }
            if score >= beta {
                beta = score.saturating_add(delta).min(MATE_IN_ONE_SCORE);
                delta = delta.saturating_mul(3) / 2;
            } else if score <= alpha {
                alpha = score.saturating_sub(delta);
                delta = delta.saturating_mul(2);
            } else {
                return Some(score);
            }

            if delta > ASPIRATION_MAX_DELTA {
                alpha = -SCORE_INFINITE;
                beta = MATE_IN_ONE_SCORE;
            }
        }
    }

    /// Seed the fallback and allow partial depth one when no cached move exists.
    fn root_fallback(&mut self, track_partial: bool) -> Option<Move> {
        let first = self.root_moves.first().copied();
        if self.board.halfmove_clock() >= FIFTY_MOVE_RULE_HALFMOVES - 1 {
            // The root moves were ranked by their known outcome.
            return first;
        }
        let cached = self
            .state
            .tables
            .tt
            .probe(self.board.hash)
            .and_then(|entry| entry.best_move())
            .filter(|mv| self.root_moves.contains(mv));
        if track_partial && cached.is_none() {
            self.partial_root = Some(PartialRoot {
                incumbent: None,
                searched_incumbent: false,
                challenger: None,
            });
        }
        cached.or(first)
    }

    /// Recover a completed improvement or select a fallback for depth one.
    fn interrupted_root_move(&mut self, fallback: Option<Move>) -> Option<Move> {
        let Some(progress) = &self.partial_root else {
            return fallback;
        };
        if let Some(challenger) = progress.challenger {
            return Some(challenger);
        }
        if progress.incumbent.is_some() {
            return fallback;
        }
        self.unsearched_root_move().or(fallback)
    }

    fn unsearched_root_move(&mut self) -> Option<Move> {
        let pawn_value = crate::board::see::SEE_VALUES[Piece::Pawn.index()];
        let capture = self
            .root_moves
            .iter()
            .copied()
            .filter(|mv| {
                mv.is_capture() && mv.promotion().is_none_or(|piece| piece == Piece::Queen)
            })
            .map(|mv| (mv, self.board.see(mv.from(), mv.to())))
            .filter(|&(_, gain)| gain >= pawn_value)
            .max_by_key(|&(_, gain)| gain)
            .map(|(mv, _)| mv);

        // Near a draw claim, the rule-aware fallback below handles quiet moves.
        let consider_quiet =
            capture.is_none() && self.board.halfmove_clock() < FIFTY_MOVE_RULE_HALFMOVES - 2;
        let mut best_quiet = None;
        for &mv in &self.root_moves {
            let undo = self.board.make_search_move(mv);
            let mate = self.board.is_checkmate();
            if !mate && consider_quiet && !mv.is_tactical() {
                let piece = self.board.piece_at(mv.to()).map(|(_, piece)| piece);
                if piece.is_some_and(|piece| {
                    piece == Piece::King || !self.board.can_win_exchange_on(mv.to(), piece)
                }) {
                    let score = -self.board.evaluate_simple();
                    if best_quiet.is_none_or(|(_, best_score)| score > best_score) {
                        best_quiet = Some((mv, score));
                    }
                }
            }
            self.board.unmake_move(mv, undo);
            if mate {
                return Some(mv);
            }
        }
        capture.or_else(|| best_quiet.map(|(mv, _)| mv))
    }

    fn first_iteration_fallback(
        &mut self,
        fallback: Option<Move>,
        line_count: usize,
    ) -> Option<Move> {
        let mv = self.interrupted_root_move(fallback)?;
        if line_count == 1
            && self.root_moves.len() > 1
            && self.board.halfmove_clock() == FIFTY_MOVE_RULE_HALFMOVES - 2
            && self.board.all_occupied.popcount() <= 5
            && outcome_near_fifty_moves(self.board, mv) == Some(-1)
        {
            // Replace a proven loss only when a legal alternative holds.
            for &alternative in &self.root_moves {
                if alternative != mv
                    && outcome_near_fifty_moves(self.board, alternative)
                        .is_some_and(|outcome| outcome >= 0)
                {
                    return Some(alternative);
                }
            }
        }
        Some(mv)
    }

    /// Complete all requested variations at a depth before publishing them.
    fn iterative_deepening(
        &mut self,
        first_depth: u32,
        max_depth: u32,
        line_count: usize,
    ) -> Option<Move> {
        let mut best_move = self.root_fallback(line_count == 1);
        self.state.stats.seldepth = 0;
        self.state.stats.tt_hits = 0;
        if max_depth == 0 || self.should_stop() || self.time_limit_reached() {
            return self.first_iteration_fallback(best_move, line_count);
        }

        let root_moves = (line_count > 1).then(|| self.root_moves.clone());
        let initially_restricted = self.root_moves_restricted;
        self.init_accumulator(0);
        let mut previous_lines = vec![(self.evaluate_main(0), None); line_count];
        let mut lines = Vec::with_capacity(line_count);

        let mut previous_best_move: Option<Move> = None;
        let mut previous_score = previous_lines[0].0;
        let mut stability_count = 0u32;
        let mut prev_iter_nodes = 0u64;
        let mut completed_iteration = false;

        'iteration: for depth in first_depth..=max_depth {
            if self.should_stop()
                || self.node_budget_exhausted()
                || self.time_limit_reached()
                || self.should_stop_iteration(
                    depth,
                    stability_count,
                    previous_lines[0].0,
                    previous_score,
                    prev_iter_nodes,
                )
            {
                break;
            }

            let iter_start_nodes = self.nodes;
            self.initial_depth = depth;
            self.root_moves_restricted = initially_restricted;
            if let Some(moves) = &root_moves {
                self.root_moves.clone_from(moves);
            }
            lines.clear();

            for &(guess, previous_move) in &previous_lines {
                if self.should_stop() || self.node_budget_exhausted() {
                    break 'iteration;
                }
                self.root_order_move = previous_move
                    .filter(|mv| self.root_moves_restricted && self.root_moves.contains(mv));
                let Some(score) = self.search_iteration(depth, guess) else {
                    return if completed_iteration {
                        self.interrupted_root_move(best_move)
                    } else {
                        self.first_iteration_fallback(best_move, line_count)
                    };
                };
                let Some(mv) = self.root_best_move else {
                    break 'iteration;
                };
                if !completed_iteration && lines.is_empty() {
                    best_move = Some(mv);
                }

                let pv = if self.info_callback.is_some() {
                    self.principal_variation(mv, depth as usize)
                } else {
                    String::new()
                };
                lines.push((mv, score, pv));

                if root_moves.is_some() {
                    self.root_moves.retain(|candidate| *candidate != mv);
                    self.root_moves_restricted = true;
                }
            }

            // Selective search can yield a different score when the root
            // domain changes. Rank the completed lines by the scores reported.
            if lines.len() > 1 {
                lines.sort_by_key(|&(_, score, _)| std::cmp::Reverse(score));
            }
            best_move = Some(lines[0].0);
            self.partial_root =
                (line_count == 1 && lines[0].1.abs() < MATE_THRESHOLD).then(|| PartialRoot {
                    incumbent: Some(lines[0].0),
                    searched_incumbent: false,
                    challenger: None,
                });
            completed_iteration = true;

            if best_move == previous_best_move {
                stability_count = stability_count.saturating_add(1);
            } else {
                stability_count = 0;
            }
            previous_best_move = best_move;
            previous_score = previous_lines[0].0;
            prev_iter_nodes = self.nodes.saturating_sub(iter_start_nodes);

            for (previous, &(mv, score, _)) in previous_lines.iter_mut().zip(&lines) {
                *previous = (score, Some(mv));
            }
            self.publish_iteration(depth, &mut lines);
            if line_count == 1 && previous_lines[0].0 == MATE_IN_ONE_SCORE {
                break;
            }
        }

        if completed_iteration {
            best_move
        } else {
            self.first_iteration_fallback(best_move, line_count)
        }
    }

    fn principal_variation(&mut self, first_move: Move, max_len: usize) -> String {
        let length = max_len.min(MAX_PLY);
        let mut hashes = [0; MAX_PLY];
        let mut played = Vec::with_capacity(length);
        let mut pv = String::new();
        for index in 0..length {
            let hash = self.board.hash;
            if hashes[..index].contains(&hash) {
                break;
            }
            hashes[index] = hash;

            let mv = if index == 0 {
                Some(first_move)
            } else {
                endgame::best_reply(self.board).or_else(|| {
                    self.state
                        .tables
                        .tt
                        .probe(hash)
                        .and_then(|entry| entry.best_move())
                })
            };
            let Some(mv) = mv.filter(|&mv| mv != EMPTY_MOVE && self.board.is_legal_move(mv)) else {
                break;
            };
            if index > 0 {
                pv.push(' ');
            }
            write!(pv, "{mv}").expect("writing to a string is infallible");
            played.push((mv, self.board.make_move(mv)));
        }
        for (mv, undo) in played.into_iter().rev() {
            self.board.unmake_move(mv, undo);
        }
        pv
    }

    fn publish_iteration(&self, depth: u32, lines: &mut Vec<(Move, i32, String)>) {
        if let Some(callback) = &self.info_callback {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            let nps = self
                .nodes
                .saturating_mul(1000)
                .checked_div(elapsed)
                .unwrap_or(0);
            for (index, (_, score, pv)) in lines.drain(..).enumerate() {
                callback(&SearchIterationInfo {
                    depth,
                    nodes: self.nodes,
                    nps,
                    time_ms: elapsed,
                    score,
                    mate_in: mate_distance(score),
                    pv,
                    seldepth: self.state.stats.seldepth,
                    tt_hits: self.state.stats.tt_hits,
                    multipv: index as u32 + 1,
                });
            }
        }
    }
}

fn mate_distance(score: i32) -> Option<i32> {
    if score.abs() < MATE_THRESHOLD {
        None
    } else if score > 0 {
        Some((MATE_SCORE - score + 1) / 2)
    } else {
        Some(-(MATE_SCORE + score + 1) / 2)
    }
}

/// Record a root result that required no tree expansion.
///
/// `total_nodes` is cumulative across direct searches. The
/// per-search counters must still describe this zero-node result rather than
/// leaking values from a previous direct call that reused `SearchState`.
fn record_zero_node_root_result(state: &mut SearchState) {
    state.stats.nodes = 0;
    state.stats.seldepth = 0;
    state.stats.tt_hits = 0;
}

fn report_resolved_root(config: &SearchConfig, multipv: u32, best_move: Option<Move>, score: i32) {
    report_resolved_line(config, multipv, score, || {
        best_move.map_or_else(String::new, |mv| mv.to_string())
    });
}

fn report_resolved_line(
    config: &SearchConfig,
    multipv: u32,
    score: i32,
    pv: impl FnOnce() -> String,
) {
    if let Some(callback) = &config.info_callback {
        callback(&SearchIterationInfo {
            depth: 0,
            nodes: 0,
            nps: 0,
            time_ms: 0,
            score,
            mate_in: mate_distance(score),
            pv: pv(),
            seldepth: 0,
            tt_hits: 0,
            multipv,
        });
    }
}

/// Rank the moves from a position where a draw can be claimed.
fn rank_drawn_root_moves(
    board: &mut Board,
    state: &SearchState,
    moves: &[Move],
) -> (Vec<(Move, Option<i32>)>, bool) {
    if !board.can_play_on_after_draw_claim() {
        return (moves.iter().map(|&mv| (mv, Some(0))).collect(), false);
    }

    let can_reset_clock = board.halfmove_clock() >= FIFTY_MOVE_RULE_HALFMOVES - 1;
    let mut ranked: Vec<_> = moves
        .iter()
        .map(|&mv| {
            let undo = board.make_move(mv);
            let score = if board.is_checkmate() {
                Some(MATE_IN_ONE_SCORE)
            } else if board.halfmove_clock() == 0 && !board.is_theoretical_draw() {
                score_after_zeroing_move(board, state)
            } else {
                Some(0)
            };
            board.unmake_move(mv, undo);
            (mv, score)
        })
        .collect();
    let unresolved = can_reset_clock && ranked.iter().any(|(_, score)| score.is_none());
    ranked.sort_by_key(|&(_, score)| std::cmp::Reverse((score.unwrap_or(0), score.is_some())));
    (ranked, unresolved)
}

fn score_after_zeroing_move(board: &mut Board, state: &SearchState) -> Option<i32> {
    let score = endgame::score(board, 1, true).map(|value| -value);
    #[cfg(feature = "syzygy")]
    let score = score.or_else(|| state.syzygy.as_ref()?.zeroing_move_score(board));
    #[cfg(not(feature = "syzygy"))]
    let _ = state;
    score
}

fn report_exact_endgame(
    board: &mut Board,
    config: &SearchConfig,
    ranked: &[(Move, i32)],
    line_count: usize,
) {
    for (index, &(mv, score)) in ranked.iter().take(line_count).enumerate() {
        report_resolved_line(config, index as u32 + 1, score, || {
            if score.abs() >= MATE_THRESHOLD {
                let length = (MATE_SCORE - score.abs()) as usize;
                endgame::mate_line(board, mv, length)
            } else {
                mv.to_string()
            }
        });
    }
}

#[cfg(feature = "syzygy")]
pub(in crate::board::search) fn try_syzygy_root(
    board: &mut Board,
    state: &mut SearchState,
    config: &SearchConfig,
    available_moves: &[Move],
    stop: &AtomicBool,
) -> Option<Move> {
    if stop.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let tables = std::sync::Arc::get_mut(state.syzygy.as_mut()?)?;
    if !tables.may_cover(board) {
        return None;
    }
    let count = config.multi_pv.max(1) as usize;
    let ranked = tables
        .rank_moves(board, available_moves, count)
        .or_else(|| tables.rank_wdl(board, available_moves, count))?;
    let best_move = ranked.first()?.0;
    record_zero_node_root_result(state);
    for (index, (mv, score)) in ranked.into_iter().enumerate() {
        report_resolved_root(config, index as u32 + 1, Some(mv), score);
    }
    Some(best_move)
}

fn resolve_endgame_root(
    board: &mut Board,
    state: &mut SearchState,
    config: &SearchConfig,
    available_moves: &[Move],
    line_count: usize,
    stop: &AtomicBool,
) -> Option<Move> {
    if let Some(ranked) = endgame::rank_moves(board, available_moves) {
        record_zero_node_root_result(state);
        report_exact_endgame(board, config, &ranked, line_count);
        return Some(ranked[0].0);
    }

    #[cfg(feature = "syzygy")]
    {
        try_syzygy_root(board, state, config, available_moves, stop)
    }
    #[cfg(not(feature = "syzygy"))]
    {
        let _ = stop;
        None
    }
}

/// Run the main search algorithm with `MultiPV` support
pub fn simple_search_multipv(
    board: &mut Board,
    state: &mut SearchState,
    config: &SearchConfig,
    stop: &AtomicBool,
    worker_id: usize,
) -> Option<Move> {
    let max_depth = clamp_search_depth(config.max_depth.unwrap_or(DEFAULT_MAX_DEPTH));

    let moves = board.generate_moves();
    let mut available_moves: Vec<Move> = moves
        .iter()
        .copied()
        .filter(|mv| config.allows_root_move(*mv))
        .collect();
    let line_count = (config.multi_pv as usize).max(1).min(available_moves.len());
    let mut root_moves_restricted = config.root_moves.is_some();

    if available_moves.is_empty() {
        record_zero_node_root_result(state);
        // An empty restricted domain is not a terminal chess position.
        if moves.is_empty() {
            let score = if board.is_in_check(board.side_to_move()) {
                -MATE_SCORE
            } else {
                0
            };
            report_resolved_root(config, 1, None, score);
        }
        return None;
    }
    if available_moves.len() == 1 && config.root_moves.is_none() && config.info_callback.is_none() {
        record_zero_node_root_result(state);
        return Some(available_moves[0]);
    }

    let drawn = board.is_theoretical_draw();
    if !drawn {
        if let Some(best_move) =
            resolve_endgame_root(board, state, config, &available_moves, line_count, stop)
        {
            return Some(best_move);
        }
    }

    let mut root_can_draw = false;
    if drawn || board.halfmove_clock() == FIFTY_MOVE_RULE_HALFMOVES - 1 {
        let (ranked, unresolved) = rank_drawn_root_moves(board, state, &available_moves);
        let (fallback, proven) = ranked[0];
        if !unresolved || (line_count == 1 && proven.is_some_and(|score| score > 0)) {
            record_zero_node_root_result(state);
            if config.info_callback.is_some() {
                let scores: Vec<_> = ranked
                    .into_iter()
                    .map(|(mv, score)| (mv, score.unwrap_or(0)))
                    .collect();
                report_exact_endgame(board, config, &scores, line_count);
            }
            return Some(fallback);
        }
        // Once we have a draw available, only unproven moves can improve it.
        // Keep the full requested domain when reporting multiple lines.
        root_can_draw = line_count == 1 && proven == Some(0);
        let total = available_moves.len();
        available_moves.clear();
        available_moves.extend(ranked.into_iter().filter_map(|(mv, score)| {
            (!root_can_draw || mv == fallback || score.is_none()).then_some(mv)
        }));
        root_moves_restricted |= available_moves.len() < total;
    }

    // Give helper workers different starting move orders and depths so
    // they populate the shared TT with less redundant work.
    if worker_id != 0 {
        let offset = worker_id.saturating_mul(5) % available_moves.len();
        available_moves.rotate_left(offset);
    }
    let first_depth = (1 + (worker_id % 3) as u32).min(max_depth);

    let futility_margin =
        effective_futility_margin(config.time_limit_ms, state.params.futility_margin);
    // The main net always serves the root evaluation when loaded. Its
    // child accumulators are needed only if pruning uses that same net.
    let (acc_stack, static_acc_stack) = accumulator_stacks(state);
    let line = SearchLine::new(board);

    let mut ctx = SimpleSearchContext {
        board,
        state,
        stop,
        start_time: Instant::now(),
        time_limit_ms: config.time_limit_ms,
        hard_time_limit_ms: config.hard_time_limit_ms,
        clock: config.clock.clone(),
        node_limit: config.node_limit,
        nodes: 0,
        aborted_on_node_limit: false,
        futility_margin,
        initial_depth: 1,
        static_eval: [0; MAX_PLY],
        previous_move: [EMPTY_MOVE; MAX_PLY],
        previous_piece: [None; MAX_PLY],
        line,
        info_callback: config.info_callback.clone(),
        root_moves: available_moves,
        root_moves_restricted,
        root_order_move: None,
        root_best_move: None,
        partial_root: None,
        root_can_draw,
        acc_stack,
        static_acc_stack,
    };

    let result = ctx.iterative_deepening(first_depth, max_depth, line_count);

    ctx.state.stats.nodes = ctx.nodes;
    ctx.state.stats.total_nodes = ctx.state.stats.total_nodes.saturating_add(ctx.nodes);

    result
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    use super::{clamp_search_depth, DEFAULT_MAX_DEPTH};
    use crate::board::{Board, SearchState};

    #[test]
    fn direct_search_depth_is_clamped_to_engine_maximum() {
        assert_eq!(clamp_search_depth(u32::MAX), DEFAULT_MAX_DEPTH);
        assert_eq!(clamp_search_depth(12), 12);
    }

    #[test]
    fn iteration_cost_estimate_honors_time_added_after_a_score_drop() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        ctx.start_time = Instant::now()
            .checked_sub(Duration::from_millis(1_100))
            .unwrap();
        ctx.time_limit_ms = 1_000;
        ctx.hard_time_limit_ms = 2_000;
        ctx.nodes = 40_000;

        assert!(!ctx.should_stop_iteration(8, 4, 50, 100, 6_000));
        assert!(ctx.should_stop_iteration(8, 4, 100, 100, 6_000));
    }

    #[test]
    fn single_deadline_does_not_end_search_on_a_cost_forecast() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        ctx.start_time = Instant::now()
            .checked_sub(Duration::from_millis(80))
            .unwrap();
        ctx.time_limit_ms = 100;
        ctx.hard_time_limit_ms = 95;
        ctx.nodes = 30_000;

        assert!(!ctx.should_stop_iteration(8, 5, 50, 50, 12_000));
        ctx.hard_time_limit_ms = 150;
        assert!(ctx.should_stop_iteration(8, 5, 50, 50, 12_000));
    }

    #[test]
    fn ponderhit_does_not_overestimate_search_throughput() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        let now = Instant::now();
        ctx.start_time = now.checked_sub(Duration::from_millis(800)).unwrap();
        ctx.nodes = 30_000;
        let clock = std::sync::Arc::new(crate::board::SearchClock::new(ctx.start_time, None, None));
        ctx.clock = Some(std::sync::Arc::clone(&clock));
        assert!(!ctx.should_stop_iteration(8, 4, 50, 50, 10_000));
        let hit = Instant::now()
            .checked_sub(Duration::from_millis(5))
            .unwrap();
        clock.reset(
            hit,
            Some(hit + Duration::from_millis(150)),
            Some(hit + Duration::from_millis(900)),
        );
        assert!(ctx.should_stop_iteration(8, 4, 50, 50, 10_000));
    }
}
