//! Core search implementation.
//!
//! This module implements:
//! - Iterative deepening with aspiration windows
//! - Alpha-beta search with PVS
//! - Null move pruning
//! - Late move reductions (LMR)
//! - Late move pruning (LMP)
//! - Static null move / Reverse futility pruning (RFP)
//! - Razoring
//! - Futility pruning
//! - Internal iterative reduction (IIR)
//! - Check, recapture, and singular extensions
//! - Mate distance pruning
//! - Quiescence search with SEE pruning
//! - Move ordering (TT move, killers, MVV-LVA, history)

mod evaluation;
mod iterative;
mod pruning;
mod quiescence;
mod transposition;

pub(super) use iterative::simple_search_multipv;
#[cfg(feature = "syzygy")]
pub(super) use iterative::try_syzygy_root;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::tt::BoundType;

use super::constants::{
    COUNTER_SCORE, KILLER1_SCORE, KILLER2_SCORE, KILLER3_SCORE, LMR_SCORE_THRESHOLD,
    LMR_TABLE_MAX_DEPTH, LMR_TABLE_MAX_IDX, MATE_THRESHOLD, PAWN_EXTENSION_RANK_BLACK,
    PAWN_EXTENSION_RANK_WHITE, SCORE_INFINITE, SCORE_NEAR_MATE, SCORE_SAFE_MAX, TT_MOVE_SCORE,
};
use super::{SearchInfoCallback, SearchState, MATE_SCORE};
use crate::board::draw::{FIFTY_MOVE_RULE_HALFMOVES, THREEFOLD_REPETITION_COUNT};
use crate::board::{Board, Move, MoveList, ScoredMoveList, EMPTY_MOVE, MAX_PLY};
use evaluation::AccumulatorStack;

use super::super::Piece;

const DRAW_CLAIM_PAWN_MARGIN: i32 = crate::board::see::SEE_VALUES[Piece::Pawn.index()] * 3 / 4;
/// How far past the soft time target a running iteration may continue when
/// the hard budget is known. Covers the 1.3x/1.4x iteration-start
/// extensions for unstable positions; the hard budget still caps it.
const SOFT_OVERRUN_FACTOR: u64 = 2;
/// Bound the full-width horizon when checks, forced replies and other
/// extensions stack up on the same line. Quiescence still searches evasions.
const MAX_EXTENSION_PLIES: u32 = 10;
const PV_EXTENSION_PLIES: u32 = 12;
/// Search context for a single search
pub struct SimpleSearchContext<'a> {
    pub board: &'a mut Board,
    pub state: &'a mut SearchState,
    pub stop: &'a AtomicBool,
    pub start_time: Instant,
    /// Soft time target in milliseconds (0 = untimed).
    pub time_limit_ms: u64,
    /// Hard time budget in milliseconds (0 = unknown). When known, a
    /// running iteration may overrun the soft target up to
    /// `SOFT_OVERRUN_FACTOR`x (never past this budget) so the stability
    /// and score-drop time extensions can take effect.
    pub hard_time_limit_ms: u64,
    /// Optional live clock shared with the controller. When present, its
    /// deadlines override the static millisecond limits above, so a
    /// `ponderhit` reset re-times the running search.
    pub clock: Option<std::sync::Arc<crate::board::search::SearchClock>>,
    pub node_limit: u64,
    pub nodes: u64,
    /// Set only when a requested node could not be searched.
    aborted_on_node_limit: bool,
    pub futility_margin: i32,
    pub initial_depth: u32,
    /// Static eval at each ply for improving detection
    pub static_eval: [i32; MAX_PLY],
    /// Previous move at each ply for counter-move heuristic
    pub previous_move: [Move; MAX_PLY],
    /// Previous piece type at each ply for continuation history
    pub previous_piece: [Option<Piece>; MAX_PLY],
    /// Position hashes on the current search line, indexed by ply.
    line: SearchLine,
    /// Optional callback for reporting iteration info
    pub info_callback: Option<SearchInfoCallback>,
    /// Root moves to consider (for `MultiPV` support - empty means all moves)
    pub root_moves: Vec<Move>,
    /// Whether this PV excludes otherwise legal root moves.
    pub root_moves_restricted: bool,
    /// Previous PV for this restricted root domain, if still permitted.
    root_order_move: Option<Move>,
    /// Root result kept independently of the shared transposition table.
    pub root_best_move: Option<Move>,
    partial_root: Option<PartialRoot>,
    /// A verified draw is available among the permitted root moves.
    root_can_draw: bool,
    acc_stack: AccumulatorStack,
    static_acc_stack: AccumulatorStack,
}

struct SearchLine {
    hashes: [u64; MAX_PLY],
    /// First position after a real move following the last synthetic pass.
    repetition_floor: usize,
    game_has_repetition: bool,
}

impl SearchLine {
    fn new(board: &Board) -> Self {
        Self {
            hashes: [0; MAX_PLY],
            repetition_floor: 0,
            game_has_repetition: board.repetition_counts.has_repeated_position(),
        }
    }
}

struct PartialRoot {
    incumbent: Option<Move>,
    searched_incumbent: bool,
    challenger: Option<Move>,
}

#[cfg(test)]
fn test_context<'a>(
    board: &'a mut Board,
    state: &'a mut SearchState,
    stop: &'a AtomicBool,
) -> SimpleSearchContext<'a> {
    let futility_margin = state.params.futility_margin;
    let line = SearchLine::new(board);
    SimpleSearchContext {
        board,
        state,
        stop,
        start_time: Instant::now(),
        time_limit_ms: 0,
        hard_time_limit_ms: 0,
        clock: None,
        node_limit: 0,
        nodes: 0,
        aborted_on_node_limit: false,
        futility_margin,
        initial_depth: 1,
        static_eval: [0; MAX_PLY],
        previous_move: [EMPTY_MOVE; MAX_PLY],
        previous_piece: [None; MAX_PLY],
        line,
        info_callback: None,
        root_moves: Vec::new(),
        root_moves_restricted: false,
        root_order_move: None,
        root_best_move: None,
        partial_root: None,
        root_can_draw: false,
        acc_stack: AccumulatorStack::new(MAX_PLY + 16, None),
        static_acc_stack: AccumulatorStack::new(MAX_PLY + 16, None),
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct NodeContext {
    ply: usize,
    /// Evaluation before correction history, retained across child updates.
    raw_eval: i32,
    is_pv: bool,
    in_check: bool,
    improving: bool,
    excluded_move: Move,
    tt_move: Move,
    /// Extension for the TT move (from singular extension search)
    singular_extension: u32,
}

/// Context for a single move being searched
struct MoveContext {
    m: Move,
    move_score: i32,
    is_quiet: bool,
    gives_check: bool,
    moving_piece: Option<crate::board::Piece>,
}

/// Result of searching one move before generating the full move list.
struct StagedMoveResult {
    mv: Move,
    score: i32,
    raised_alpha: bool,
}

impl SimpleSearchContext<'_> {
    /// Compute extensions for a move
    fn compute_extensions(ctx: &MoveContext, node: &NodeContext) -> u32 {
        let mut extension = 0u32;

        // Check extension
        if ctx.gives_check {
            extension += 1;
        }

        // Singular extension
        if ctx.m == node.tt_move && node.singular_extension > 0 {
            extension += node.singular_extension;
        }

        // Passed pawn extension: pawn on 7th/2nd rank is dangerous
        if extension == 0 && !ctx.m.is_promotion() {
            if let Some(crate::board::Piece::Pawn) = ctx.moving_piece {
                let to_rank = ctx.m.to().rank();
                if to_rank == PAWN_EXTENSION_RANK_WHITE || to_rank == PAWN_EXTENSION_RANK_BLACK {
                    extension += 1;
                }
            }
        }

        extension
    }

    fn child_search_depth(
        &self,
        depth: u32,
        node: &NodeContext,
        move_ctx: &MoveContext,
        forced: bool,
    ) -> u32 {
        let base_depth = depth.saturating_sub(1);
        let extension = Self::compute_extensions(move_ctx, node) + u32::from(forced);
        let nominal_leaf = node.ply as u32 + 1 + base_depth;
        let extension_budget = if node.is_pv {
            PV_EXTENSION_PLIES
        } else {
            MAX_EXTENSION_PLIES
        };
        let extension_room = (self.initial_depth + extension_budget).saturating_sub(nominal_leaf);
        base_depth + extension.min(extension_room)
    }

    /// Check if a quiet move should be pruned (futility pruning or LMP)
    fn should_prune_quiet(
        &self,
        ctx: &MoveContext,
        node: &NodeContext,
        depth: u32,
        moves_tried: usize,
        alpha: i32,
    ) -> bool {
        if !ctx.is_quiet
            || node.ply == 0
            || node.in_check
            || ctx.gives_check
            || node.is_pv
            || alpha.abs() >= SCORE_NEAR_MATE
        {
            return false;
        }

        // Futility pruning
        if depth <= 6 && moves_tried > 1 {
            let static_eval = if node.ply < MAX_PLY {
                self.static_eval[node.ply]
            } else {
                0
            };
            let futility_margin = self.futility_margin * depth as i32;
            // If the search bound says this side can at least draw while the
            // static evaluator says it is losing, the position is often being
            // held by tactics. Do not discard quiet tactical resources merely
            // from that contradictory static estimate.
            if !(alpha >= 0 && static_eval < 0) && static_eval + futility_margin <= alpha {
                return true;
            }
        }

        // Late Move Pruning (LMP)
        if depth <= self.state.params.lmp_min_depth {
            let lmp_threshold = self.state.params.lmp_move_limit + depth as usize * depth as usize;
            if moves_tried > lmp_threshold {
                return true;
            }
        }

        false
    }

    /// Precomputed LMR table - slightly more aggressive than before
    #[allow(clippy::cast_precision_loss)]
    fn lmr_table() -> &'static [[u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH] {
        use std::sync::OnceLock;
        static TABLE: OnceLock<[[u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH]> = OnceLock::new();
        TABLE.get_or_init(|| {
            let mut t = [[0u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH];
            for (depth, row) in t.iter_mut().enumerate().skip(1) {
                for (idx, cell) in row.iter_mut().enumerate().skip(1) {
                    // Stockfish-like LMR: base 0.77, divisor 2.36
                    let val = (0.77 + (depth as f64).ln() * (idx as f64).ln() / 2.36).floor();
                    *cell = val.max(0.0) as u32;
                }
            }
            t
        })
    }

    /// Search the ordered move list and return the best score.
    #[allow(clippy::too_many_lines)]
    fn search_moves(
        &mut self,
        node: &NodeContext,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        moves: &MoveList,
        staged: Option<StagedMoveResult>,
    ) -> i32 {
        let ply = node.ply;
        let in_check = node.in_check;

        // Get previous move for counter-move ordering
        let prev_move = if ply > 0 && ply < MAX_PLY {
            self.previous_move[ply - 1]
        } else {
            EMPTY_MOVE
        };

        // Move ordering: TT move, killers, counter, captures, history
        // Use partial sorting (pick_best) to avoid sorting moves we never try
        let staged_move = staged.as_ref().map(|result| result.mv);
        let ordering_move = if ply == 0 {
            self.root_order_move.unwrap_or(node.tt_move)
        } else {
            staged_move.unwrap_or(node.tt_move)
        };
        let mut scored_moves = ScoredMoveList::new();
        self.order_moves(moves, ordering_move, ply, prev_move, &mut scored_moves);

        let move_count = scored_moves.len();
        let forced = move_count == 1
            && !(ply == 0 && self.root_moves_restricted && self.board.has_multiple_legal_moves());
        let tt_tactical = node.tt_move.is_tactical();
        let draw_in_range =
            self.board.halfmove_clock().saturating_add(depth) >= FIFTY_MOVE_RULE_HALFMOVES;

        let (mut best_score, mut best_move, mut raised_alpha) = if let Some(result) = staged {
            if result.raised_alpha {
                alpha = result.score;
            }
            (result.score, result.mv, result.raised_alpha)
        } else {
            (-SCORE_INFINITE, EMPTY_MOVE, false)
        };
        let mut moves_tried = usize::from(staged_move.is_some());

        // Track quiet moves for negative history on beta cutoff
        let mut quiets_tried: [Move; 64] = [EMPTY_MOVE; 64];
        let mut quiets_count = 0usize;

        // Use partial sorting: pick best remaining move each iteration
        // This avoids sorting moves we never try due to early cutoffs
        let mut i = 0;
        while let Some(scored) = scored_moves.pick_best(i) {
            let m = scored.mv;
            let move_score = scored.score;
            i += 1;
            if self.should_stop() {
                break;
            }

            // Skip excluded move (for singular extension search)
            if m == node.excluded_move {
                continue;
            }

            // The priority move was already searched.
            if staged_move == Some(m) {
                continue;
            }

            // Track if this is a quiet move
            let is_quiet = !m.is_tactical();
            let moving_piece = self.board.piece_at(m.from()).map(|(_, p)| p);
            // A quiet non-pawn move can keep the fifty-move draw reachable.
            let preserves_draw_clock =
                draw_in_range && is_quiet && moving_piece != Some(Piece::Pawn);

            // Screen cheap candidates before making them. Verify the
            // exchange on the resulting board, where pins and defenders
            // reflect the quiet move.
            let suspect_quiet = is_quiet
                && !preserves_draw_clock
                && depth <= 3
                && ply > 0
                && !node.is_pv
                && !in_check
                && moves_tried > 0
                && alpha.abs() < SCORE_NEAR_MATE
                && !self.board.see_quiet_safe(m.from(), m.to());

            // Make move first (we'll check for check after)
            let info = self.board.make_search_move(m);

            // Prefetch TT entry for this position to hide memory latency
            // By the time we call alphabeta, the cache line should be loaded
            self.state.tables.tt.prefetch(self.board.hash);

            // Check if move gives check
            let gives_check = self.board.is_in_check(self.board.side_to_move());

            if suspect_quiet
                && !gives_check
                && moving_piece.is_some_and(|piece| self.board.can_win_exchange_on(m.to(), piece))
            {
                self.board.unmake_move(m, info);
                continue;
            }

            if ply < MAX_PLY {
                self.previous_move[ply] = m;
                self.previous_piece[ply] = moving_piece;
            }

            moves_tried += 1;

            // Create move context for helper methods
            let move_ctx = MoveContext {
                m,
                move_score,
                is_quiet,
                gives_check,
                moving_piece,
            };

            // Futility pruning and LMP
            if !preserves_draw_clock
                && self.should_prune_quiet(&move_ctx, node, depth, moves_tried, alpha)
            {
                self.board.unmake_move(m, info);
                continue;
            }

            // Track quiet moves for negative history on a later beta cutoff.
            // Only moves that are actually searched belong here: penalizing
            // futility/LMP-pruned moves would poison the history tables.
            if is_quiet && quiets_count < 64 {
                quiets_tried[quiets_count] = m;
                quiets_count += 1;
            }

            if let Some(piece) = moving_piece {
                self.update_accumulator_for_move(ply, m, piece, info.captured_piece_info);
            }

            // LMR reduction
            let reduction = if preserves_draw_clock || alpha.abs() >= SCORE_NEAR_MATE {
                // A reduced search can miss a shorter mating line without
                // ever raising a mate-valued alpha enough to be re-searched.
                0
            } else {
                self.compute_lmr_reduction(
                    i - 1,
                    move_count,
                    depth,
                    node,
                    &move_ctx,
                    tt_tactical || gives_check || m.is_capture(),
                )
            };

            let new_depth = self.child_search_depth(depth, node, &move_ctx, forced);

            let mut score: i32;

            if i > 1 {
                // PVS: null window search for non-first moves (i is 1-indexed after pick_best)
                score = -self.alphabeta(
                    new_depth.saturating_sub(reduction),
                    -alpha - 1,
                    -alpha,
                    true,
                    ply + 1,
                    EMPTY_MOVE,
                );

                // Re-search at full depth if reduced search found something
                if reduction > 0 && score > alpha {
                    score =
                        -self.alphabeta(new_depth, -alpha - 1, -alpha, true, ply + 1, EMPTY_MOVE);
                }

                // Re-search with full window if PVS found improvement
                if score > alpha && score < beta {
                    score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);
                }
            } else {
                // First move: full window search
                score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);
            }

            // A quiet pawn advance gives up the draw claim. Check for a
            // proven loss and confirm small gains with a full reply.
            if ply == 0
                && self.root_can_draw
                && new_depth == 0
                && is_quiet
                && moving_piece == Some(Piece::Pawn)
                && score > alpha.max(0)
                && !self.should_stop()
            {
                if super::endgame::refuted_by_quiet_check(self.board) {
                    score = -1;
                } else if score < DRAW_CLAIM_PAWN_MARGIN {
                    score = -self.alphabeta(1, -beta, -alpha, true, ply + 1, EMPTY_MOVE);
                }
            }

            self.board.unmake_move(m, info);

            if self.should_stop() {
                break;
            }

            if ply == 0 {
                if let Some(progress) = &mut self.partial_root {
                    if Some(m) == progress.incumbent {
                        progress.searched_incumbent = true;
                    } else if (progress.incumbent.is_none() || progress.searched_incumbent)
                        && score > alpha
                    {
                        // The incumbent was searched in this window and did not
                        // cut off, or there is no searched move to protect yet.
                        // This move has a completed score or lower bound.
                        progress.challenger = Some(m);
                    }
                }
            }

            if score > best_score {
                best_score = score;
                best_move = m;

                if score > alpha {
                    if score >= beta {
                        // Singular-extension verification excludes a legal
                        // move. Its result must not replace the unrestricted
                        // TT entry or train ordinary move-ordering history.
                        if node.excluded_move == EMPTY_MOVE {
                            for quiet_mv in quiets_tried.iter().take(quiets_count) {
                                if *quiet_mv != m && *quiet_mv != EMPTY_MOVE {
                                    self.state.tables.history.penalize(quiet_mv, depth);
                                }
                            }
                            self.handle_beta_cutoff(m, ply, depth, score);
                        }
                        return score;
                    }
                    alpha = score;
                    raised_alpha = true;
                }
            }
        }

        // A stop can arrive after move generation but before the first move
        // is searched.  Do not let that look like an empty legal-move list:
        // returning a terminal score here would turn an interrupted search
        // into a fictitious mate or stalemate in the parent.
        if self.should_stop() {
            return 0;
        }

        // Check for checkmate/stalemate
        if moves_tried == 0 {
            if node.excluded_move != EMPTY_MOVE {
                // No alternative to the excluded move is a failed
                // verification search, not an actual terminal position.
                return alpha;
            }
            return if in_check {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }

        if node.excluded_move == EMPTY_MOVE {
            let bound = if raised_alpha {
                BoundType::Exact
            } else {
                BoundType::UpperBound
            };
            self.store_tt(depth, best_score, bound, best_move, ply);
        }

        // Update correction history for exact bounds (when we have reliable score vs static eval)
        if raised_alpha
            && ply < MAX_PLY
            && !in_check
            && node.excluded_move == EMPTY_MOVE
            && !(ply == 0 && self.root_moves_restricted)
            && best_score.abs() < SCORE_NEAR_MATE
        {
            let pawn_hash = self.board.pawn_hash();
            self.state.tables.correction_history.update(
                pawn_hash,
                self.board.side_to_move(),
                node.raw_eval,
                best_score,
                depth,
            );
        }

        best_score
    }

    /// Current `(elapsed, soft, hard)` time budget in milliseconds.
    ///
    /// With a live clock, deadlines and the start instant come from it, so
    /// a `ponderhit` reset re-times the search from the hit; otherwise the
    /// static limits captured at search start apply. A zero soft budget
    /// means untimed.
    pub(super) fn time_budget_ms(&self) -> (u64, u64, u64) {
        let to_ms = |d: std::time::Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
        if let Some((start, soft_deadline, hard_deadline)) =
            self.clock.as_ref().map(|clock| clock.snapshot())
        {
            let elapsed = to_ms(Instant::now().saturating_duration_since(start));
            let soft = soft_deadline.map_or(0, |d| to_ms(d.saturating_duration_since(start)));
            let hard = hard_deadline.map_or(0, |d| to_ms(d.saturating_duration_since(start)));
            (elapsed, soft, hard)
        } else {
            let elapsed = to_ms(self.start_time.elapsed());
            (elapsed, self.time_limit_ms, self.hard_time_limit_ms)
        }
    }

    /// Mid-tree time abort budget.
    ///
    /// Without a known hard budget this is exactly the soft target
    /// (legacy behavior for direct callers). With one, the iteration may
    /// run to `min(soft * SOFT_OVERRUN_FACTOR, hard)`: iteration-start
    /// decisions still key on the soft target, so this headroom is what
    /// lets an unstable position actually receive extra time.
    #[inline]
    fn in_tree_time_cap_ms(soft_ms: u64, hard_ms: u64) -> u64 {
        if soft_ms == 0 {
            hard_ms
        } else if hard_ms > 0 {
            soft_ms.saturating_mul(SOFT_OVERRUN_FACTOR).min(hard_ms)
        } else {
            soft_ms
        }
    }

    fn time_limit_reached(&self) -> bool {
        if let Some((start, soft_deadline, hard_deadline)) =
            self.clock.as_ref().map(|clock| clock.snapshot())
        {
            let now = Instant::now();
            if hard_deadline.is_some_and(|deadline| now >= deadline) {
                return true;
            }
            let Some(soft_deadline) = soft_deadline else {
                return false;
            };
            if hard_deadline.is_none() {
                return now >= soft_deadline;
            }

            // Compare absolute deadlines so an expired or sub-millisecond
            // budget cannot round down to the zero/unlimited sentinel.
            let overrun = soft_deadline
                .saturating_duration_since(start)
                .saturating_mul(SOFT_OVERRUN_FACTOR as u32);
            start
                .checked_add(overrun)
                .is_some_and(|deadline| now >= deadline)
        } else {
            let cap = Self::in_tree_time_cap_ms(self.time_limit_ms, self.hard_time_limit_ms);
            cap > 0 && self.start_time.elapsed().as_millis() >= u128::from(cap)
        }
    }

    /// Check if we should stop searching
    #[inline]
    fn should_stop(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        if self
            .state
            .hard_stop_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return true;
        }
        if self.aborted_on_node_limit {
            return true;
        }
        if (self.time_limit_ms > 0 || self.hard_time_limit_ms > 0 || self.clock.is_some())
            && self.nodes.trailing_zeros() >= 10
            && self.time_limit_reached()
        {
            return true;
        }

        false
    }

    /// Account for a searched node without crossing a caller's hard node cap.
    #[inline]
    fn try_visit_node(&mut self, ply: usize) -> bool {
        if self.node_budget_exhausted() {
            self.aborted_on_node_limit = true;
            return false;
        }
        if self.should_stop() {
            return false;
        }
        self.nodes = self.nodes.saturating_add(1);
        self.state.stats.seldepth = self.state.stats.seldepth.max(ply as u32);
        if ply < MAX_PLY {
            self.line.hashes[ply] = self.board.hash;
        }
        true
    }

    #[inline]
    fn node_budget_exhausted(&self) -> bool {
        self.node_limit > 0 && self.nodes >= self.node_limit
    }

    #[inline]
    fn is_search_draw(&self, ply: usize) -> bool {
        // A position cannot recur in fewer than four reversible plies.
        let halfmove = self.board.halfmove_clock() as usize;
        if halfmove >= 4 {
            let hash = self.board.hash;
            let first = self.line.repetition_floor.max(ply.saturating_sub(halfmove));
            // The previous occurrence must have the same side to move.
            if let Some(last) = ply
                .checked_sub(4)
                .filter(|&index| index >= first && index < MAX_PLY)
            {
                if self.line.hashes[first..=last]
                    .iter()
                    .rev()
                    .step_by(2)
                    .any(|&previous| previous == hash)
                {
                    return true;
                }
            }
            if self.line.repetition_floor == 0
                && self.line.game_has_repetition
                && self.board.repetition_counts.get(hash)
                    >= THREEFOLD_REPETITION_COUNT - u32::from(ply > 0)
            {
                return true;
            }
        }
        self.board.is_fifty_move_draw() || self.board.is_insufficient_material()
    }

    /// Resolve terminal positions before evaluating at a search limit.
    fn score_at_search_limit(&mut self, ply: usize) -> i32 {
        if self.board.has_legal_move() {
            self.evaluate_static(ply)
        } else if self.board.is_in_check(self.board.side_to_move()) {
            -MATE_SCORE + ply as i32
        } else {
            0
        }
    }

    #[inline]
    fn exact_endgame_score(&mut self, ply: usize) -> Option<i32> {
        let score = self.built_in_endgame_score(ply);
        #[cfg(feature = "syzygy")]
        let score = score.or_else(|| self.syzygy_after_zeroing(ply));
        score
    }

    #[cfg(feature = "syzygy")]
    fn syzygy_after_zeroing(&mut self, ply: usize) -> Option<i32> {
        let tables = self.state.syzygy.as_ref()?;
        if ply == 0 || self.board.halfmove_clock() != 0 {
            return None;
        }
        let score = -tables.zeroing_move_score(self.board)?;
        Some(if score < 0 && self.board.is_checkmate() {
            -MATE_SCORE + ply as i32
        } else {
            score
        })
    }

    #[inline]
    fn built_in_endgame_score(&mut self, ply: usize) -> Option<i32> {
        if !matches!(self.board.all_occupied.popcount(), 3 | 4) {
            return None;
        }
        let score = super::endgame::score(self.board, ply, false)?;
        if score < 0 && self.line.repetition_floor == 0 && self.board.halfmove_clock() >= 3 {
            // The root is already included in the played game's history.
            let history = &self.line.hashes[1..ply.clamp(1, MAX_PLY)];
            if (self.line.game_has_repetition
                || history
                    .iter()
                    .any(|&hash| self.board.repetition_counts.get(hash) > 0))
                && super::endgame::can_claim_repetition_in_line(self.board, history)
            {
                return Some(0);
            }
        }
        Some(score)
    }

    /// Check if the position is improving (eval better than 2 plies ago)
    #[inline]
    fn is_improving(&self, ply: usize, eval: i32) -> bool {
        if (2..MAX_PLY).contains(&ply) {
            eval > self.static_eval[ply - 2]
        } else {
            true
        }
    }

    /// Order moves for better pruning (TT move > killers > counter > captures > history + continuation)
    fn order_moves(
        &self,
        moves: &MoveList,
        tt_move: Move,
        ply: usize,
        prev_move: Move,
        scored: &mut ScoredMoveList,
    ) {
        // Get counter move for the previous move (if any)
        let counter = if prev_move == EMPTY_MOVE {
            EMPTY_MOVE
        } else {
            let from = prev_move.from().index();
            let to = prev_move.to().index();
            self.state.tables.counter_moves.get(from, to)
        };

        // Get previous piece for continuation history
        let prev_piece = if ply > 0 && ply < MAX_PLY {
            self.previous_piece[ply - 1]
        } else {
            None
        };
        let prev_to = if prev_move == EMPTY_MOVE {
            0
        } else {
            prev_move.to().index()
        };

        let killers = &self.state.tables.killer_moves;
        let primary = killers.primary(ply);
        let secondary = killers.secondary(ply);
        let tertiary = killers.tertiary(ply);
        for m in moves {
            let score = if *m == tt_move {
                TT_MOVE_SCORE
            } else if *m == primary {
                KILLER1_SCORE
            } else if *m == secondary {
                KILLER2_SCORE
            } else if *m == tertiary {
                KILLER3_SCORE
            } else if *m == counter {
                COUNTER_SCORE
            } else if m.is_tactical() {
                self.state.tables.mvv_lva_score(self.board, m)
            } else {
                // Combine history, continuation history, and countermove history for quiet moves
                let hist = self.state.tables.history_score(m);
                let cont_hist = if let Some(piece) = prev_piece {
                    self.state
                        .tables
                        .continuation_history
                        .score(piece, prev_to, *m)
                } else {
                    0
                };
                // Countermove history: use opponent's previous move and our moving piece
                let cm_hist = if let Some(opp_piece) = prev_piece {
                    if let Some((_, our_piece)) = self.board.piece_at(m.from()) {
                        // Weight countermove history at 0.5x of continuation history
                        self.state
                            .tables
                            .countermove_history
                            .score(opp_piece, prev_to, our_piece, *m)
                            / 2
                    } else {
                        0
                    }
                } else {
                    0
                };
                hist + cont_hist + cm_hist
            };
            scored.push(*m, score);
        }
    }

    /// Handle beta cutoff: update killers, history, counter moves, continuation history, and TT
    fn handle_beta_cutoff(&mut self, m: Move, ply: usize, depth: u32, score: i32) {
        // Update killers for quiet moves
        if m.is_quiet() && ply < MAX_PLY {
            self.state.tables.killer_moves.update(ply, m);

            // Update counter move: what move refuted the opponent's previous move?
            if ply > 0 {
                let prev = self.previous_move[ply - 1];
                if prev != EMPTY_MOVE {
                    let from = prev.from().index();
                    let to = prev.to().index();
                    self.state.tables.counter_moves.set(from, to, m);
                }
            }

            // Update continuation history and countermove history
            if ply > 0 {
                if let Some(prev_piece) = self.previous_piece[ply - 1] {
                    let prev_to = self.previous_move[ply - 1].to().index();
                    self.state
                        .tables
                        .continuation_history
                        .update(prev_piece, prev_to, m, depth);

                    // Also update countermove history (opponent's piece-to -> our piece-to)
                    if let Some((_, our_piece)) = self.board.piece_at(m.from()) {
                        self.state
                            .tables
                            .countermove_history
                            .update(prev_piece, prev_to, our_piece, m, depth);
                    }
                }
            }
        } else if m.is_capture() {
            // Update capture history for captures.
            // After unmake_move: m.from() has the attacker, m.to() has the captured piece
            // (restored by unmake). For en passant, the captured pawn is on a different
            // square, so we handle it explicitly.
            if let Some((_, attacker)) = self.board.piece_at(m.from()) {
                let victim = if m.is_en_passant() {
                    Piece::Pawn
                } else {
                    self.board.piece_at(m.to()).map_or(Piece::Pawn, |(_, p)| p)
                };
                self.state
                    .tables
                    .capture_history
                    .update(attacker, victim, depth);
            }
        }

        // Update history with gravity
        self.state.tables.update_history(&m, depth);

        self.store_tt(depth, score, BoundType::LowerBound, m, ply);
    }

    /// Compute LMR reduction for a move.
    ///
    /// Uses `NodeContext` and `MoveContext` to reduce parameter count.
    fn compute_lmr_reduction(
        &self,
        move_idx: usize,
        move_count: usize,
        depth: u32,
        node: &NodeContext,
        move_ctx: &MoveContext,
        tt_tactical: bool,
    ) -> u32 {
        let lmr_ok = move_idx > self.state.params.lmr_min_move + move_count / 4
            && move_ctx.move_score < LMR_SCORE_THRESHOLD
            && depth >= self.state.params.lmr_min_depth
            && !node.in_check
            && !move_ctx.gives_check
            && move_ctx.is_quiet
            && !node.is_pv
            && !tt_tactical;

        if lmr_ok {
            let table = Self::lmr_table();
            let depth_idx = depth.min((LMR_TABLE_MAX_DEPTH - 1) as u32) as usize;
            let move_idx_clamped = move_idx.min(LMR_TABLE_MAX_IDX - 1);
            let mut reduction = table[depth_idx][move_idx_clamped];

            // Reduce less when position is improving (our eval is getting better)
            if node.improving {
                reduction = reduction.saturating_sub(1);
            }

            // Reduce less for moves with good history scores
            if move_ctx.move_score > 1000 {
                reduction = reduction.saturating_sub(1);
            }

            reduction.min(depth.saturating_sub(1))
        } else {
            0
        }
    }

    /// Alpha-beta search with all pruning and extension techniques
    #[allow(clippy::too_many_lines)]
    pub fn alphabeta(
        &mut self,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        allow_null: bool,
        ply: usize,
        excluded_move: Move,
    ) -> i32 {
        // Singular extension constants
        const SINGULAR_MIN_DEPTH: u32 = 6;
        const SINGULAR_MARGIN: i32 = 3; // margin per depth

        // Quiescence counts its own root and handles draws and terminal
        // positions, so entering it does not count as a full-width node.
        if depth == 0 {
            return self.quiesce(alpha, beta, ply, 0);
        }
        if !self.try_visit_node(ply) {
            return 0;
        }

        let is_root = ply == 0;
        let is_pv = beta > alpha + 1;
        let excluded_move_active = excluded_move != EMPTY_MOVE;

        // A repeated position on the current line can be claimed as a draw.
        // The board-level rules also cover the fifty-move rule and positions
        // with insufficient mating material. Do this before probing the TT: a
        // transposition entry does not encode repetition history.
        if !is_root && self.is_search_draw(ply) {
            return 0;
        }

        // Resolve supported endings before static pruning and TT cutoffs.
        if !is_root && !excluded_move_active {
            if let Some(score) = self.exact_endgame_score(ply) {
                return score;
            }
        }

        // Extensions can make the game-tree ply exceed the nominal search
        // depth.  Keep recursion inside the fixed heuristic stacks rather
        // than risking an out-of-bounds continuation/NNUE access in an
        // unusually long checking sequence.
        if ply >= MAX_PLY {
            return self.score_at_search_limit(ply);
        }

        let in_check = self.board.is_in_check(self.board.side_to_move());

        // Mate distance pruning
        if !is_root {
            alpha = alpha.max(-MATE_SCORE + ply as i32);
            beta = beta.min(MATE_SCORE - ply as i32 + 1);
            if alpha >= beta {
                return alpha;
            }
        }

        // Probe TT for best move and potential cutoff
        let tt = self.probe_tt(depth, alpha, beta, ply, is_pv, excluded_move_active);
        let tt_move = tt.best_move;

        // Restricted MultiPV roots have different search domains. Use the
        // shared TT for ordering there, but never for a root score cutoff.
        let use_tt_cutoff = if is_root {
            !self.root_moves_restricted
                && tt_move != EMPTY_MOVE
                && (self.root_moves.is_empty() || self.root_moves.contains(&tt_move))
        } else {
            true
        };

        if use_tt_cutoff {
            if let Some(cutoff_score) = tt.cutoff {
                if is_root {
                    self.root_best_move = Some(tt_move);
                }
                self.state.stats.tt_hits = self.state.stats.tt_hits.saturating_add(1);
                return cutoff_score;
            }
        }

        // Static evaluation for pruning decisions (needed before node-level pruning)
        let raw_eval = if in_check {
            -SCORE_INFINITE // Don't use static eval when in check
        } else {
            self.evaluate_static(ply)
        };

        // Apply correction history to improve static eval accuracy
        let eval = if in_check {
            raw_eval
        } else {
            let pawn_hash = self.board.pawn_hash();
            let correction = self
                .state
                .tables
                .correction_history
                .get(pawn_hash, self.board.side_to_move());
            (raw_eval + correction).clamp(-SCORE_SAFE_MAX, SCORE_SAFE_MAX)
        };

        self.static_eval[ply] = eval;
        let mut node = NodeContext {
            ply,
            raw_eval,
            is_pv,
            in_check,
            improving: self.is_improving(ply, eval),
            excluded_move,
            tt_move,
            singular_extension: 0,
        };

        // ========================================================================
        // NODE-LEVEL PRUNING (before move loop)
        // ========================================================================

        if !is_pv && !in_check && !excluded_move_active {
            if let Some(score) = self.prune_before_move_loop(depth, beta, eval, &node, allow_null) {
                // The pruning routine already established multiple legal moves.
                return score;
            }
        }

        // ========================================================================
        // SINGULAR EXTENSION
        // ========================================================================
        // If we have a reliable TT move, check if it's singular (much better than
        // alternatives). If so, extend its search by 1 ply. This must run before
        // the staged TT-move search below: the staged search is the only place
        // the TT move is searched on that path, so an extension computed later
        // would never apply.
        if !excluded_move_active
            && !is_root
            && depth >= SINGULAR_MIN_DEPTH
            && tt_move != EMPTY_MOVE
            && tt.depth + 3 >= depth
            && tt.score.abs() < MATE_THRESHOLD
            && matches!(tt.bound, BoundType::LowerBound | BoundType::Exact)
            && self.board.is_legal_move(tt_move)
        {
            let singular_beta = tt.score - SINGULAR_MARGIN * depth as i32;
            let singular_depth = (depth - 1) / 2;

            // Search with TT move excluded
            let singular_score = self.alphabeta(
                singular_depth,
                singular_beta - 1,
                singular_beta,
                false,
                ply,
                tt_move,
            );

            if singular_score < singular_beta {
                // TT move is singular - extend it
                node.singular_extension = 1;
            }
        }

        // Try the hash move or a promising capture before generating quiet replies.
        let staged_result = if !excluded_move_active && !is_root && !in_check {
            let first = if tt_move != EMPTY_MOVE && self.board.is_legal_move(tt_move) {
                Some(tt_move)
            } else if tt_move == EMPTY_MOVE
                && !is_pv
                && depth <= 2
                && depth < self.state.params.iir_min_depth
                && beta.abs() < SCORE_NEAR_MATE
                && eval >= beta - 120
            {
                self.first_tactical_move()
            } else {
                None
            };
            first.and_then(|mv| self.try_move_first(mv, &node, depth, alpha, beta))
        } else {
            None
        };

        if let Some(ref result) = staged_result {
            if result.score >= beta {
                return result.score;
            }
        }

        // Generate moves (at root, use root_moves if available for MultiPV)
        let mut moves = MoveList::new();
        if is_root && !self.root_moves.is_empty() {
            for m in &self.root_moves {
                moves.push(*m);
            }
        } else if in_check {
            self.board.generate_evasions_into(&mut moves);
        } else {
            self.board.generate_moves_into(&mut moves);
        }

        // Handle empty move list (checkmate/stalemate)
        // A staged result came from a legal move.
        if moves.is_empty() {
            return if let Some(result) = staged_result {
                result.score
            } else if in_check {
                -MATE_SCORE + ply as i32 // Checkmate
            } else {
                0 // Stalemate
            };
        }

        // Internal Iterative Reduction (IIR)
        // If we have no TT move at high depth, reduce depth to find a move faster
        let search_depth = if tt_move == EMPTY_MOVE
            && depth >= self.state.params.iir_min_depth
            && !excluded_move_active
            && alpha.abs() < SCORE_NEAR_MATE
            && beta.abs() < SCORE_NEAR_MATE
        {
            depth - 1
        } else {
            depth
        };

        self.search_moves(&node, search_depth, alpha, beta, &moves, staged_result)
    }

    fn first_tactical_move(&self) -> Option<Move> {
        let mut moves = MoveList::new();
        self.board.generate_tactical_moves_into(&mut moves);
        let mut first = None;
        for &mv in &moves {
            let score = self.state.tables.mvv_lva_score(self.board, &mv);
            if first.is_none_or(|(_, best)| score > best) {
                first = Some((mv, score));
            }
        }
        let (mv, score) = first?;
        (score > KILLER1_SCORE && (mv.is_promotion() || self.board.see(mv.from(), mv.to()) >= 0))
            .then_some(mv)
    }

    /// Search a legal priority move before generating the full move list.
    fn try_move_first(
        &mut self,
        mv: Move,
        node: &NodeContext,
        depth: u32,
        alpha: i32,
        beta: i32,
    ) -> Option<StagedMoveResult> {
        let forced = !self.board.has_legal_move_except(mv);
        let ply = node.ply;

        // Get the piece that's moving for continuation history
        let moving_piece = self.board.piece_at(mv.from()).map(|(_, p)| p);

        // Make the move
        let info = self.board.make_search_move(mv);

        // Prefetch TT entry for the new position
        self.state.tables.tt.prefetch(self.board.hash);

        // Check if move gives check
        let gives_check = self.board.is_in_check(self.board.side_to_move());

        if ply < MAX_PLY {
            self.previous_move[ply] = mv;
            self.previous_piece[ply] = moving_piece;
        }

        // Compute extensions for the staged move
        let is_quiet = !mv.is_tactical();
        let move_ctx = MoveContext {
            m: mv,
            move_score: TT_MOVE_SCORE,
            is_quiet,
            gives_check,
            moving_piece,
        };
        let new_depth = self.child_search_depth(depth, node, &move_ctx, forced);

        if let Some(piece) = moving_piece {
            self.update_accumulator_for_move(ply, mv, piece, info.captured_piece_info);
        }
        // Full window search for the staged move (it's the first move)
        let score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);

        self.board.unmake_move(mv, info);

        if self.should_stop() {
            return None;
        }

        // Check for beta cutoff
        if score >= beta {
            // Update history heuristics on cutoff
            self.handle_beta_cutoff(mv, ply, depth, score);
            return Some(StagedMoveResult {
                mv,
                score,
                raised_alpha: true,
            });
        }

        // The staged move did not cause cutoff, but may have raised alpha
        Some(StagedMoveResult {
            mv,
            score,
            raised_alpha: score > alpha,
        })
    }
}

#[cfg(test)]
mod evaluation_tests {
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    use crate::board::nnue::NnueAccumulator;
    use crate::board::{Board, SearchState, EMPTY_MOVE, MAX_PLY};

    use super::{NodeContext, MATE_SCORE, SCORE_INFINITE};
    use crate::tt::BoundType;

    #[test]
    fn inherited_twofold_is_not_an_immediate_search_draw() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/4Q3/6K1 w - - 0 1");
        for mv in ["e2e3", "h8g8", "e3e2", "g8h8"] {
            let mv = board.parse_move(mv).unwrap();
            board.make_move(mv);
        }
        let search_root_hash = board.hash();
        let mv = board.parse_move("e2e3").unwrap();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.line.hashes[0] = search_root_hash;
        ctx.board.make_search_move(mv);
        assert_eq!(ctx.board.repetition_counts.get(ctx.board.hash()), 1);

        let score = ctx.alphabeta(1, -SCORE_INFINITE, SCORE_INFINITE, true, 1, EMPTY_MOVE);

        assert!(score < 0, "a historical twofold was scored as a draw");
    }

    #[test]
    fn search_line_twofold_is_still_scored_as_a_draw() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/4Q3/6K1 w - - 0 1");
        let repeated_hash = board.hash();
        for mv in ["e2e3", "h8g8", "e3e2", "g8h8"] {
            let mv = board.parse_move(mv).unwrap();
            board.make_move(mv);
        }
        assert_eq!(board.hash(), repeated_hash);
        assert_eq!(board.repetition_counts.get(repeated_hash), 2);
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.line.hashes[0] = repeated_hash;

        let score = ctx.alphabeta(1, -SCORE_INFINITE, SCORE_INFINITE, true, 4, EMPTY_MOVE);

        assert_eq!(score, 0);
    }

    #[test]
    fn repetition_uses_only_real_moves_after_a_null_move() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/4Q3/6K1 w - - 4 1");
        let hash = board.hash();
        board.repetition_counts.set(hash, 3);
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.init_accumulator(6);
        ctx.line.hashes[0] = hash;
        ctx.line.hashes[1] = hash; // Position reached by the synthetic pass.
        ctx.line.repetition_floor = 2;

        assert!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 6, 0) > 0);
        assert!(ctx.alphabeta(1, -SCORE_INFINITE, SCORE_INFINITE, true, 6, EMPTY_MOVE) > 0);

        ctx.line.hashes[2] = hash; // Earlier position after a real move.
        assert!(ctx.is_search_draw(6));
        ctx.line.repetition_floor = 0;
        assert!(ctx.is_search_draw(6));
    }

    #[test]
    fn quiescence_leaf_counts_towards_nodes_and_selective_depth() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/4Q3/6K1 w - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.init_accumulator(3);
        ctx.node_limit = 1;

        assert!(ctx.alphabeta(0, -SCORE_INFINITE, SCORE_INFINITE, true, 3, EMPTY_MOVE) > 0);
        assert_eq!(ctx.nodes, 1);
        assert_eq!(ctx.state.stats.seldepth, 3);
        assert_eq!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 4, 0), 0);
        assert_eq!(ctx.nodes, 1);
        assert_eq!(ctx.state.stats.seldepth, 3);
    }

    #[test]
    fn cached_forced_moves_keep_the_single_reply_extension() {
        for cached in [false, true] {
            // Black must play Kh7, after which Rh1 is mate. At depth one,
            // the sole-reply extension must expose that quiet mating move.
            let mut board = Board::from_fen("7k/5K2/8/8/8/8/8/R7 b - - 0 1");
            let forced = board.parse_move("h8h7").unwrap();
            assert_eq!(board.generate_moves().len(), 1);
            assert!(!board.is_in_check(board.side_to_move()));
            let mut state = SearchState::new(1);
            if cached {
                state.tables.tt.store_with_halfmove_clock(
                    board.hash(),
                    0,
                    0,
                    BoundType::UpperBound,
                    Some(forced),
                    state.generation,
                    board.halfmove_clock(),
                );
            }
            let stop = AtomicBool::new(false);
            let mut ctx = super::test_context(&mut board, &mut state, &stop);
            let score = ctx.alphabeta(1, -SCORE_INFINITE, SCORE_INFINITE, true, 1, EMPTY_MOVE);

            assert_eq!(score, -MATE_SCORE + 3, "cached={cached}");
        }
    }

    #[test]
    fn check_and_forced_reply_extensions_share_the_same_horizon() {
        let mut board = Board::new();
        let mv = board.parse_move("e2e4").unwrap();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.initial_depth = 6;
        let mut node = NodeContext {
            ply: 14,
            raw_eval: 0,
            is_pv: true,
            in_check: true,
            improving: false,
            excluded_move: EMPTY_MOVE,
            tt_move: mv,
            singular_extension: 1,
        };
        let checking_move = super::MoveContext {
            m: mv,
            move_score: 0,
            is_quiet: true,
            gives_check: true,
            moving_piece: Some(crate::board::Piece::Pawn),
        };

        // A check, singular move and forced reply would request three
        // extensions. Only one fits before the PV's ply-18 horizon.
        assert_eq!(ctx.child_search_depth(3, &node, &checking_move, true), 3);
        node.is_pv = false;
        assert_eq!(ctx.child_search_depth(3, &node, &checking_move, true), 2);
        node.is_pv = true;
        node.ply += 1;
        assert_eq!(ctx.child_search_depth(3, &node, &checking_move, true), 2);
    }

    #[test]
    fn incremental_neural_accumulators_match_refresh_for_special_moves() {
        use crate::board::nnue::network::BASE_INPUT_SIZE;
        use crate::board::nnue::{NnueNetwork, HIDDEN_SIZE};
        use std::sync::Arc;

        let network = Arc::new(NnueNetwork {
            input_size: BASE_INPUT_SIZE,
            feature_weights: (0..BASE_INPUT_SIZE)
                .map(|feature| {
                    std::array::from_fn(|lane| ((feature * 131 + lane * 179) % 65536) as i16)
                })
                .collect(),
            feature_bias: [32000; HIDDEN_SIZE],
            output_weights_white: [0; HIDDEN_SIZE],
            output_weights_black: [0; HIDDEN_SIZE],
            output_bias: 0,
        });
        for fen in [
            "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
            "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1",
            "7k/8/8/3pP3/8/8/8/K7 w - d6 0 1",
            "7k/8/8/8/3Pp3/8/8/K7 b - d3 0 1",
            "r6k/1P6/8/8/8/8/8/7K w - - 0 1",
            "7k/8/8/8/8/8/1p6/R6K b - - 0 1",
        ] {
            let mut board = Board::from_fen(fen);
            let moves = board.generate_moves();
            let mut state = SearchState::new(1);
            state.tables.nnue = Some(Arc::clone(&network));
            state.tables.static_nnue = Some(Arc::clone(&network));
            let stop = AtomicBool::new(false);
            let mut ctx = super::test_context(&mut board, &mut state, &stop);
            ctx.init_accumulator(0);

            for &mv in &moves {
                let (_, piece) = ctx.board.piece_at(mv.from()).unwrap();
                let undo = ctx.board.make_move(mv);
                ctx.update_accumulator_for_move(0, mv, piece, undo.captured_piece_info);
                let (white, black) = ctx.board.compute_nnue_features();
                let mut refreshed = NnueAccumulator::default();
                refreshed.refresh(&white, &black, &network);
                for stack in [&ctx.acc_stack, &ctx.static_acc_stack] {
                    assert_eq!(stack.at(1).white, refreshed.white, "{fen}: {mv}");
                    assert_eq!(stack.at(1).black, refreshed.black, "{fen}: {mv}");
                }
                ctx.board.unmake_move(mv, undo);
            }
        }
    }

    #[test]
    fn reverse_futility_cannot_claim_to_escape_a_forced_mate() {
        let mut board = Board::from_fen("8/8/8/3K4/8/4kp1R/6Q1/8 b - - 1 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        let alpha = -MATE_SCORE + 3;
        let score = ctx.alphabeta(2, alpha, alpha + 1, true, 1, EMPTY_MOVE);
        assert!(
            score <= alpha,
            "material pruning invented an escape: {score}"
        );
    }

    #[test]
    fn neural_evaluations_cannot_enter_the_mate_score_range() {
        use crate::board::nnue::network::BASE_INPUT_SIZE;
        use crate::board::nnue::{NnueNetwork, HIDDEN_SIZE, QA, QB};
        use crate::board::search::constants::MATE_THRESHOLD;

        for sign in [-1, 1] {
            let network = std::sync::Arc::new(NnueNetwork {
                input_size: BASE_INPUT_SIZE,
                feature_weights: vec![[0; HIDDEN_SIZE]; BASE_INPUT_SIZE],
                feature_bias: [QA as i16; HIDDEN_SIZE],
                output_weights_white: [(sign * QB) as i16; HIDDEN_SIZE],
                output_weights_black: [(sign * QB) as i16; HIDDEN_SIZE],
                output_bias: 0,
            });
            for static_network in [false, true] {
                let mut board = Board::new();
                let mut state = SearchState::new(1);
                if static_network {
                    state.tables.static_nnue = Some(std::sync::Arc::clone(&network));
                } else {
                    state.tables.nnue = Some(std::sync::Arc::clone(&network));
                }
                state.static_eval_options.nnue_pure = true;
                state.nnue_eval_scale = 400;
                state.nnue_static_eval_scale = 400;
                let stop = AtomicBool::new(false);
                let mut ctx = super::test_context(&mut board, &mut state, &stop);
                ctx.init_accumulator(0);

                assert!(ctx.evaluate_main(0).abs() < MATE_THRESHOLD);
                assert!(ctx.evaluate_static(0).abs() < MATE_THRESHOLD);
                assert!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 0, 0).abs() < MATE_THRESHOLD);
            }
        }
    }

    #[test]
    fn white_correction_does_not_bias_black_with_the_same_pawns() {
        let mut board = Board::new();
        let pawn_hash = board.pawn_hash();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        state
            .tables
            .correction_history
            .update(pawn_hash, crate::board::Color::White, 0, 400, 8);
        board.flip_side_to_move();
        assert_eq!(board.pawn_hash(), pawn_hash);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        let raw_eval = ctx.evaluate_static(1);

        ctx.alphabeta(1, -SCORE_INFINITE, SCORE_INFINITE, true, 1, EMPTY_MOVE);

        assert_eq!(ctx.static_eval[1], raw_eval);
    }

    #[test]
    fn transpositions_do_not_reuse_scores_across_fifty_move_clocks() {
        let stop = AtomicBool::new(false);
        for clocks in [[0, 99], [99, 0]] {
            let mut state = SearchState::new(1);
            for clock in clocks {
                let mut board = Board::from_fen(&format!("7k/8/8/8/8/8/4Q3/K7 w - - {clock} 1"));
                let mut ctx = super::test_context(&mut board, &mut state, &stop);
                let score = ctx.alphabeta(2, -SCORE_INFINITE, SCORE_INFINITE, false, 1, EMPTY_MOVE);
                if clock == 99 {
                    assert_eq!(score, 0, "reused winning score at fifty-move boundary");
                } else {
                    assert!(score > 500, "reused a fifty-move draw score: {score}");
                }
            }
        }
    }

    #[test]
    fn transposition_clock_mismatch_allows_cutoffs_only_before_the_rule_can_apply() {
        let stop = AtomicBool::new(false);
        let mut state = SearchState::new(1);
        for (clock, expected) in [
            (super::transposition::TT_EARLY_CLOCK_LIMIT - 1, Some(75)),
            (super::transposition::TT_EARLY_CLOCK_LIMIT, None),
        ] {
            let mut board = Board::from_fen(&format!("7k/8/8/8/8/8/4Q3/K7 w - - {clock} 1"));
            let mv = board.parse_move("e2e4").unwrap();
            state
                .tables
                .tt
                .store(board.hash(), 6, 75, BoundType::Exact, Some(mv), 0);
            let ctx = super::test_context(&mut board, &mut state, &stop);
            let tt = ctx.probe_tt(6, 0, 1, 2, false, false);
            assert_eq!(tt.best_move, mv);
            assert_eq!(tt.cutoff, expected);
        }
    }

    #[test]
    fn static_pruning_does_not_skip_an_imminent_fifty_move_draw() {
        let stop = AtomicBool::new(false);
        for (clock, depth) in [(99, 1), (98, 2)] {
            let mut board = Board::from_fen(&format!("7k/8/8/8/8/8/4Q3/K7 w - - {clock} 1"));
            let mut state = SearchState::new(1);
            let mut ctx = super::test_context(&mut board, &mut state, &stop);
            let score = ctx.alphabeta(depth, 0, 1, false, 1, EMPTY_MOVE);

            assert_eq!(score, 0, "static pruning ignored draw at clock {clock}");
        }
    }

    #[test]
    fn quiet_fifty_move_draw_survives_move_pruning() {
        // Nxf3 resets the clock and loses. Any quiet move reaches move 50
        // and saves the game, even though White is far behind on material.
        let mut board = Board::from_fen("7k/8/7q/8/6p1/5p2/8/K3N3 w - - 99 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.init_accumulator(2);

        assert_eq!(ctx.alphabeta(1, -1, 0, false, 2, EMPTY_MOVE), 0);
    }

    #[test]
    fn hard_only_time_limits_stop_search() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.start_time = Instant::now()
            .checked_sub(std::time::Duration::from_millis(100))
            .unwrap();
        ctx.hard_time_limit_ms = 20;

        assert!(
            ctx.should_stop(),
            "a hard limit must work without a soft limit"
        );
        ctx.hard_time_limit_ms = 10_000;
        assert!(!ctx.should_stop());
    }

    #[test]
    fn expired_live_deadlines_stop_even_when_their_millisecond_budget_is_zero() {
        for (has_soft, has_hard) in [(true, false), (false, true), (true, true)] {
            for nanos in [0, 1] {
                let mut board = Board::new();
                let mut state = SearchState::new(1);
                let stop = AtomicBool::new(false);
                let mut ctx = super::test_context(&mut board, &mut state, &stop);
                let start = Instant::now()
                    .checked_sub(std::time::Duration::from_millis(1))
                    .unwrap();
                let deadline = start + std::time::Duration::from_nanos(nanos);
                ctx.clock = Some(std::sync::Arc::new(crate::board::SearchClock::new(
                    start,
                    has_soft.then_some(deadline),
                    has_hard.then_some(deadline),
                )));

                assert!(
                    ctx.should_stop(),
                    "soft={has_soft} hard={has_hard} nanos={nanos}"
                );
            }
        }
    }

    #[test]
    fn live_hard_only_clock_enforces_deadline_updates() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        let start = Instant::now();
        let clock = std::sync::Arc::new(crate::board::SearchClock::new(
            start,
            None,
            Some(start + std::time::Duration::from_secs(10)),
        ));
        ctx.clock = Some(clock.clone());
        assert!(!ctx.should_stop());
        clock.reset(start, None, Some(start));
        assert!(ctx.should_stop());
    }

    #[test]
    fn stalemate_takes_precedence_over_static_pruning() {
        // White is materially ahead but every pawn is blocked and Ka1
        // has no legal move. A positive evaluation cannot justify a cutoff.
        for depth in 0..=8 {
            let mut board = Board::from_fen("b7/P7/P7/P7/P7/P7/P1k5/K7 w - - 0 1");
            assert!(board.is_stalemate());
            let mut state = SearchState::new(1);
            let stop = AtomicBool::new(false);
            let mut ctx = super::test_context(&mut board, &mut state, &stop);
            assert!(ctx.evaluate_static(1) > 0);
            assert_eq!(
                ctx.alphabeta(depth, 0, 1, true, 1, EMPTY_MOVE),
                0,
                "depth {depth}"
            );
        }
    }

    #[test]
    fn terminal_positions_are_recognized_at_the_ply_limit() {
        for (fen, expected) in [
            ("b7/P7/P7/P7/P7/P7/P1k5/K7 w - - 0 1", 0),
            (
                "7k/6Q1/6K1/8/8/8/8/8 b - - 0 1",
                -MATE_SCORE + MAX_PLY as i32,
            ),
        ] {
            let mut board = Board::from_fen(fen);
            let mut state = SearchState::new(1);
            let stop = AtomicBool::new(false);
            let mut ctx = super::test_context(&mut board, &mut state, &stop);
            assert_eq!(
                ctx.alphabeta(
                    1,
                    -SCORE_INFINITE,
                    SCORE_INFINITE,
                    true,
                    MAX_PLY,
                    EMPTY_MOVE
                ),
                expected
            );
            assert_eq!(
                ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, MAX_PLY, 0),
                expected
            );
        }
    }

    #[test]
    fn excluded_move_search_preserves_the_unrestricted_tt_entry() {
        for (alpha, beta) in [
            (-SCORE_INFINITE, SCORE_INFINITE),
            (-10_000, -9_999),
            (10_000, 10_001),
        ] {
            let mut board = Board::from_fen("7k/8/8/8/3q4/8/3R4/6K1 w - - 0 1");
            let excluded = board.parse_move("d2d4").unwrap();
            let mut state = SearchState::new(1);
            let stop = AtomicBool::new(false);
            state
                .tables
                .tt
                .store(board.hash(), 6, 900, BoundType::Exact, Some(excluded), 0);
            let mut ctx = super::test_context(&mut board, &mut state, &stop);

            ctx.alphabeta(2, alpha, beta, false, 1, excluded);

            let entry = ctx.state.tables.tt.probe(ctx.board.hash()).unwrap();
            assert_eq!(entry.depth(), 6, "window {alpha}..{beta}");
            assert_eq!(entry.score(), 900);
            assert_eq!(entry.bound_type(), BoundType::Exact);
            assert_eq!(entry.best_move(), Some(excluded));
        }
    }

    #[test]
    fn excluding_the_only_legal_move_fails_low_without_claiming_a_draw() {
        let mut board = Board::from_fen("7k/5K2/8/6Q1/8/8/8/8 b - - 0 1");
        let moves = board.generate_moves();
        assert_eq!(moves.len(), 1);
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);

        let score = ctx.alphabeta(2, -100, -99, false, 1, moves[0]);
        assert!(score < -99, "no alternatives must fail low, got {score}");
    }

    #[test]
    fn null_window_search_keeps_a_checking_move_attacked_by_a_pinned_pawn() {
        let mut board = Board::from_fen("7k/5K1p/8/4N3/8/8/8/7R w - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.futility_margin = 0;

        // At an interior non-PV node, SEE pruning is active. Rh7+ is
        // searched before Ng6#, so the first-move safeguard is not enough.
        let score = ctx.alphabeta(1, MATE_SCORE - 3, MATE_SCORE - 2, true, 1, EMPTY_MOVE);
        assert_eq!(score, MATE_SCORE - 2);
    }

    #[test]
    fn static_full_hce_uses_tuned_evaluation_when_enabled() {
        let mut board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        let mut state = SearchState::new(1);
        state.static_eval_options.use_full_hce = true;
        state.hce_options.use_tuned = true;
        let expected = board.evaluate_tuned_hce_cached(&state.tables.pawn_hash);
        let untuned = board.evaluate_cached(&state.tables.pawn_hash);
        assert_ne!(
            expected, untuned,
            "test position must distinguish HCE modes"
        );
        let stop = AtomicBool::new(false);
        let ctx = super::test_context(&mut board, &mut state, &stop);

        assert_eq!(ctx.evaluate_static(0), expected);
        assert_eq!(ctx.state.evaluate_static_position(ctx.board), expected);
    }

    #[cfg(feature = "embedded_nnue")]
    #[test]
    fn refreshed_static_score_matches_search_for_both_nnue_paths() {
        let mut board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 2 1");
        let mut state = SearchState::new(1);
        let network = std::sync::Arc::new(crate::board::nnue::NnueNetwork::from_embedded());
        state.tables.nnue = Some(network.clone());
        state.static_eval_options.nnue_pure = true;
        state.nnue_hce_blend = 75;
        state.nnue_static_blend = 35;
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);

        ctx.init_accumulator(0);
        assert_eq!(
            ctx.evaluate_static(0),
            ctx.state.evaluate_static_position(ctx.board)
        );
        ctx.state.tables.static_nnue = Some(network);
        ctx.init_accumulator(0);
        assert_eq!(
            ctx.evaluate_static(0),
            ctx.state.evaluate_static_position(ctx.board)
        );
    }

    #[test]
    fn interrupted_move_loop_returns_neutral_score_not_terminal_score() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(true);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        let moves = ctx.board.generate_moves();
        let node = NodeContext {
            ply: 0,
            raw_eval: 0,
            is_pv: false,
            in_check: false,
            improving: false,
            excluded_move: EMPTY_MOVE,
            tt_move: EMPTY_MOVE,
            singular_extension: 0,
        };

        assert_eq!(
            ctx.search_moves(&node, 1, -SCORE_INFINITE, SCORE_INFINITE, &moves, None),
            0
        );
    }

    #[test]
    fn transposition_table_mate_scores_preserve_distance_across_plies() {
        let winning_score = MATE_SCORE - 10;
        let losing_score = -MATE_SCORE + 10;

        assert_eq!(
            super::transposition::score_to_tt(winning_score, 10),
            MATE_SCORE
        );
        assert_eq!(
            super::transposition::score_from_tt(MATE_SCORE, 4),
            MATE_SCORE - 4
        );
        assert_eq!(
            super::transposition::score_to_tt(losing_score, 10),
            -MATE_SCORE
        );
        assert_eq!(
            super::transposition::score_from_tt(-MATE_SCORE, 4),
            -MATE_SCORE + 4
        );

        for score in [winning_score, losing_score, 437, -437] {
            assert_eq!(
                super::transposition::score_from_tt(
                    super::transposition::score_to_tt(score, 10),
                    10
                ),
                score,
                "score {score} must round-trip at its storage ply"
            );
        }
    }

    #[test]
    fn transposition_table_probe_decodes_mate_score_at_current_ply() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let score_at_storage_ply = MATE_SCORE - 8;
        state.tables.tt.store(
            board.hash(),
            4,
            super::transposition::score_to_tt(score_at_storage_ply, 8),
            BoundType::Exact,
            None,
            state.generation,
        );
        let ctx = super::test_context(&mut board, &mut state, &stop);

        let tt = ctx.probe_tt(4, -SCORE_INFINITE, SCORE_INFINITE, 3, false, false);

        assert_eq!(tt.score, MATE_SCORE - 3);
        assert_eq!(tt.cutoff, Some(MATE_SCORE - 3));
    }

    #[test]
    fn iteration_overruns_soft_target_only_within_hard_budget() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let started_earlier = Instant::now()
            .checked_sub(std::time::Duration::from_millis(50))
            .expect("process uptime exceeds 50ms");
        let mut ctx = super::test_context(&mut board, &mut state, &stop);
        ctx.start_time = started_earlier;
        ctx.time_limit_ms = 40;

        // Without a hard budget the soft target aborts mid-tree (legacy).
        assert!(ctx.should_stop(), "soft target alone must abort at 40ms");

        // A generous hard budget allows up to 2x the soft target.
        ctx.hard_time_limit_ms = 1_000;
        assert!(
            !ctx.should_stop(),
            "50ms elapsed is within the 80ms overrun cap"
        );

        // The hard budget always wins over the overrun factor.
        ctx.hard_time_limit_ms = 45;
        assert!(ctx.should_stop(), "hard budget of 45ms must abort at 50ms");
    }

    #[test]
    fn search_ply_limit_returns_static_score_without_descending() {
        let mut board = Board::new();
        let expected = board.evaluate_simple();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::test_context(&mut board, &mut state, &stop);

        assert_eq!(
            ctx.alphabeta(
                4,
                -SCORE_INFINITE,
                SCORE_INFINITE,
                true,
                MAX_PLY,
                EMPTY_MOVE
            ),
            expected
        );
        assert_eq!(ctx.nodes, 1, "ply-limit evaluation is a single leaf");
        assert_eq!(ctx.state.stats.seldepth, MAX_PLY as u32);
    }
}
