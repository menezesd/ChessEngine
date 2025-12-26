//! Core search implementation.
//!
//! This module implements:
//! - Iterative deepening with aspiration windows
//! - Alpha-beta search with PVS
//! - Null move pruning with verification
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

mod iterative;
mod pruning;
mod quiescence;

pub use iterative::simple_search;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::tt::BoundType;

use super::constants::{
    COUNTER_SCORE, KILLER1_SCORE, KILLER2_SCORE, LMR_IDX_BASE, LMR_SCORE_THRESHOLD,
    LMR_TABLE_MAX_DEPTH, LMR_TABLE_MAX_IDX, MATE_THRESHOLD, TT_MOVE_SCORE,
};
use super::{SearchInfoCallback, SearchState, MATE_SCORE};
use crate::board::{Board, Move, MoveList, ScoredMoveList, EMPTY_MOVE, MAX_PLY};

/// Search context for a single search
pub struct SimpleSearchContext<'a> {
    pub board: &'a mut Board,
    pub state: &'a mut SearchState,
    pub stop: &'a AtomicBool,
    pub start_time: Instant,
    pub time_limit_ms: u64,
    pub node_limit: u64,
    pub nodes: u64,
    pub initial_depth: u32,
    /// Static eval at each ply for improving detection
    pub static_eval: [i32; MAX_PLY],
    /// Previous move at each ply for counter-move heuristic
    pub previous_move: [Move; MAX_PLY],
    /// Optional callback for reporting iteration info
    pub info_callback: Option<SearchInfoCallback>,
}

#[derive(Clone, Copy)]
struct NodeContext {
    ply: usize,
    is_pv: bool,
    in_check: bool,
    improving: bool,
    excluded_move_active: bool,
    tt_move: Move,
    tt_score: i32,
    tt_bound: BoundType,
}

#[derive(Clone, Copy)]
struct MovePruning {
    skip: bool,
    reduction: u32,
}

impl SimpleSearchContext<'_> {
    /// Precomputed LMR table
    fn lmr_table() -> &'static [[u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH] {
        use std::sync::OnceLock;
        static TABLE: OnceLock<[[u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH]> = OnceLock::new();
        TABLE.get_or_init(|| {
            let mut t = [[0u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH];
            for depth in 1..LMR_TABLE_MAX_DEPTH {
                for idx in 1..LMR_TABLE_MAX_IDX {
                    let val = (0.53 + (depth as f64).ln() * (idx as f64).ln() / 2.44).floor();
                    t[depth][idx] = val.max(0.0) as u32;
                }
            }
            t
        })
    }

    /// Extract Principal Variation from TT
    /// Returns a vector of moves representing the best line
    fn extract_pv(&mut self, max_len: usize) -> Vec<Move> {
        let mut pv = Vec::with_capacity(max_len);
        // Use fixed array instead of HashSet - max_len is bounded by MAX_PLY
        let mut seen_hashes = [0u64; MAX_PLY];
        let mut seen_count = 0usize;
        let mut unmake_infos = Vec::with_capacity(max_len);

        for _ in 0..max_len {
            // Avoid infinite loops from TT collisions - linear scan is faster for small N
            let hash = self.board.hash;
            if seen_hashes[..seen_count].contains(&hash) {
                break;
            }
            seen_hashes[seen_count] = hash;
            seen_count += 1;

            // Get best move from TT
            let tt_move = if let Some(entry) = self.state.tables.tt.probe(self.board.hash) {
                entry.best_move()
            } else {
                None
            };

            let Some(mv) = tt_move else { break };
            if mv == EMPTY_MOVE {
                break;
            }

            // Verify move is legal using fast single-move check
            if !self.board.is_legal_move(mv) {
                break;
            }

            pv.push(mv);
            let info = self.board.make_move(mv);
            unmake_infos.push((mv, info));
        }

        // Restore board position
        for (mv, info) in unmake_infos.into_iter().rev() {
            self.board.unmake_move(mv, info);
        }

        pv
    }

    /// Format PV moves as a space-separated string of UCI moves
    fn format_pv(pv: &[Move]) -> String {
        pv.iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Search the ordered move list and return the best score.
    fn search_moves(
        &mut self,
        node: &NodeContext,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        moves: &MoveList,
    ) -> i32 {
        let is_pv = node.is_pv;
        let ply = node.ply;
        let in_check = node.in_check;
        let improving = node.improving;

        // Get previous move for counter-move ordering
        let prev_move = if ply > 0 && ply < MAX_PLY {
            self.previous_move[ply - 1]
        } else {
            EMPTY_MOVE
        };

        // Move ordering: TT move, killers, counter, captures, history
        let mut scored_moves = self.order_moves(moves, node.tt_move, ply, prev_move);
        if depth > 1 {
            scored_moves.sort_by_score_desc();
        }

        let move_count = scored_moves.len();
        let tt_tactical = node.tt_move.is_capture() || node.tt_move.is_promotion();

        let mut best_score = -30000i32;
        let mut best_move = EMPTY_MOVE;
        let mut raised_alpha = false;
        let mut moves_tried = 0;
        let mut _quiet_moves_tried = 0;

        for (i, scored) in scored_moves.iter().enumerate() {
            let m = scored.mv;
            let move_score = scored.score;
            if self.should_stop() {
                break;
            }

            // Track if this is a quiet move
            let is_quiet = !m.is_capture() && !m.is_promotion();
            if is_quiet {
                _quiet_moves_tried += 1;
            }

            // Make move first (we'll check for check after)
            let info = self.board.make_move(m);

            // Check if move gives check
            let gives_check = self.board.is_in_check(self.board.current_color());

            if ply < MAX_PLY {
                self.previous_move[ply] = m;
            }

            moves_tried += 1;

            let mut pruning = MovePruning {
                skip: false,
                reduction: 0,
            };

            pruning.reduction = self.compute_lmr_reduction(
                i,
                move_count,
                move_score,
                depth,
                in_check,
                gives_check,
                is_quiet,
                improving,
                is_pv,
                tt_tactical || gives_check || m.is_capture(),
            );

            if pruning.skip {
                self.board.unmake_move(m, info);
                continue;
            }

            let new_depth = if move_count == 1 { depth } else { depth - 1 };

            let mut score: i32;

            if i > 0 {
                // PVS: null window search for non-first moves
                score = -self.alphabeta(
                    new_depth.saturating_sub(pruning.reduction),
                    -alpha - 1,
                    -alpha,
                    true,
                    ply + 1,
                    false,
                );

                // Re-search at full depth if reduced search found something
                if pruning.reduction > 0 && score > alpha {
                    score = -self.alphabeta(new_depth, -alpha - 1, -alpha, true, ply + 1, false);
                }

                // Re-search with full window if PVS found improvement
                if score > alpha && score < beta {
                    score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, false);
                }
            } else {
                // First move: full window search
                score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, false);
            }

            self.board.unmake_move(m, info);

            if self.should_stop() {
                break;
            }

            if score > best_score {
                best_score = score;
                best_move = m;

                if score > alpha {
                    if score >= beta {
                        self.handle_beta_cutoff(m, ply, depth, score, best_move);
                        return score;
                    }
                    alpha = score;
                    raised_alpha = true;
                }
            }
        }

        // Check for checkmate/stalemate
        if moves_tried == 0 {
            return if in_check {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }

        self.store_tt(depth, best_score, raised_alpha, best_move);
        best_score
    }

    /// Check if we should stop searching
    #[inline]
    fn should_stop(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        if self.node_limit > 0 && self.nodes >= self.node_limit {
            return true;
        }
        if self.time_limit_ms > 0 && (self.nodes & 1023) == 0 {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if elapsed >= self.time_limit_ms {
                return true;
            }
        }

        false
    }

    /// Evaluate position from side-to-move's perspective
    #[inline]
    fn evaluate(&self) -> i32 {
        // Use simple evaluation for faster search
        self.board.evaluate_simple()
    }

    /// Fast/simple evaluation for pruning decisions
    #[inline]
    fn evaluate_simple(&self) -> i32 {
        self.board.evaluate_simple()
    }

    /// Check for repetition (returns true if position repeated)
    #[inline]
    fn is_repetition(&self) -> bool {
        self.board.repetition_counts.get(self.board.hash) > 1
    }

    /// Check if the position is improving (eval better than 2 plies ago)
    #[inline]
    fn is_improving(&self, ply: usize, eval: i32) -> bool {
        if ply < 2 {
            true
        } else {
            eval > self.static_eval[ply - 2]
        }
    }

    /// Order moves for better pruning (TT move > killers > counter > captures > history)
    fn order_moves(
        &mut self,
        moves: &MoveList,
        tt_move: Move,
        ply: usize,
        prev_move: Move,
    ) -> ScoredMoveList {
        // Get counter move for the previous move (if any)
        let counter = if prev_move != EMPTY_MOVE {
            let from = prev_move.from().index().as_usize();
            let to = prev_move.to().index().as_usize();
            self.state.tables.counter_moves.get(from, to)
        } else {
            EMPTY_MOVE
        };

        let mut scored = ScoredMoveList::new();
        for m in moves {
            let score = if *m == tt_move {
                TT_MOVE_SCORE
            } else if ply < MAX_PLY && *m == self.state.tables.killer_moves.primary(ply) {
                KILLER1_SCORE
            } else if ply < MAX_PLY && *m == self.state.tables.killer_moves.secondary(ply) {
                KILLER2_SCORE
            } else if *m == counter {
                COUNTER_SCORE
            } else if m.is_capture() {
                self.state.tables.mvv_lva_score(self.board, m)
            } else {
                self.state.tables.history_score(m)
            };
            scored.push(*m, score);
        }
        scored
    }

    /// Handle beta cutoff: update killers, history, counter moves, and TT
    fn handle_beta_cutoff(&mut self, m: Move, ply: usize, depth: u32, score: i32, best_move: Move) {
        // Update killers for quiet moves
        if !m.is_capture() && ply < MAX_PLY {
            self.state.tables.killer_moves.update(ply, m);

            // Update counter move: what move refuted the opponent's previous move?
            if ply > 0 {
                let prev = self.previous_move[ply - 1];
                if prev != EMPTY_MOVE {
                    let from = prev.from().index().as_usize();
                    let to = prev.to().index().as_usize();
                    self.state.tables.counter_moves.set(from, to, m);
                }
            }
        }

        // Update history
        self.state.tables.update_history(&m, depth);

        // Store in TT (allow mate scores too)
        if !self.should_stop() {
            self.state.tables.tt.store(
                self.board.hash,
                depth,
                score,
                BoundType::LowerBound,
                Some(best_move),
                self.state.generation,
            );
        }
    }

    /// Store position in transposition table
    fn store_tt(&mut self, depth: u32, score: i32, raised_alpha: bool, best_move: Move) {
        if self.should_stop() || best_move == EMPTY_MOVE {
            return;
        }
        let bound = if raised_alpha {
            BoundType::Exact
        } else {
            BoundType::UpperBound
        };
        self.state.tables.tt.store(
            self.board.hash,
            depth,
            score,
            bound,
            Some(best_move),
            self.state.generation,
        );
    }

    /// Probe TT and check for cutoff.
    /// Returns (tt_move, tt_score, tt_bound, Option<cutoff_score>)
    fn probe_tt_for_cutoff(
        &self,
        depth: u32,
        alpha: i32,
        beta: i32,
        is_pv: bool,
        excluded_move_active: bool,
    ) -> (Move, i32, BoundType, Option<i32>) {
        let Some(entry) = self.state.tables.tt.probe(self.board.hash) else {
            return (EMPTY_MOVE, 0, BoundType::Exact, None);
        };

        let tt_move = entry.best_move().unwrap_or(EMPTY_MOVE);
        let tt_score = entry.score();
        let tt_bound = entry.bound_type();

        // Check for cutoff
        if !excluded_move_active && entry.depth() >= depth && !self.is_repetition() {
            let score = entry.score();
            let cutoff = match entry.bound_type() {
                BoundType::Exact => {
                    if !is_pv || (score > alpha && score < beta) {
                        Some(score)
                    } else {
                        None
                    }
                }
                BoundType::LowerBound => {
                    if score >= beta {
                        Some(score)
                    } else {
                        None
                    }
                }
                BoundType::UpperBound => {
                    if score <= alpha {
                        Some(score)
                    } else {
                        None
                    }
                }
            };
            return (tt_move, tt_score, tt_bound, cutoff);
        }

        (tt_move, tt_score, tt_bound, None)
    }

    /// Compute LMR reduction for a move.
    fn compute_lmr_reduction(
        &self,
        move_idx: usize,
        move_count: usize,
        move_score: i32,
        depth: u32,
        in_check: bool,
        gives_check: bool,
        is_quiet: bool,
        _improving: bool,
        is_pv: bool,
        tt_tactical: bool,
    ) -> u32 {
        let lmr_ok = move_idx > LMR_IDX_BASE + move_count / 4
            && move_score < LMR_SCORE_THRESHOLD
            && depth > 1
            && !in_check
            && !gives_check
            && is_quiet
            && !is_pv
            && !tt_tactical;

        if lmr_ok {
            let table = Self::lmr_table();
            let depth_idx = depth.min((LMR_TABLE_MAX_DEPTH - 1) as u32) as usize;
            let move_idx_clamped = move_idx.min(LMR_TABLE_MAX_IDX - 1);
            let base = table[depth_idx][move_idx_clamped];
            base.min(depth.saturating_sub(1))
        } else {
            0
        }
    }

    /// Alpha-beta search with all pruning and extension techniques
    pub fn alphabeta(
        &mut self,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        allow_null: bool,
        ply: usize,
        excluded_move_active: bool,
    ) -> i32 {
        let is_root = ply == 0;
        let is_pv = beta > alpha + 1;
        let mut node = NodeContext {
            ply,
            is_pv,
            in_check: false,
            improving: false,
            excluded_move_active,
            tt_move: EMPTY_MOVE,
            tt_score: 0,
            tt_bound: BoundType::Exact,
        };

        // Repetition check
        if !is_root && self.is_repetition() {
            return 0;
        }

        // Quiescence at leaf
        if depth == 0 {
            return self.quiesce(alpha, beta, 0);
        }

        self.nodes += 1;
        if (ply as u32 + 1) > self.state.stats.seldepth {
            self.state.stats.seldepth = ply as u32 + 1;
        }

        // Check stopping conditions periodically
        if self.should_stop() {
            return 0;
        }

        // Check for missing king (illegal position)
        if self.board.find_king(self.board.current_color()).is_none() {
            return -29000;
        }

        let in_check = self.board.is_in_check(self.board.current_color());
        node.in_check = in_check;

        // Mate distance pruning
        if !is_root {
            alpha = alpha.max(-MATE_SCORE + ply as i32);
            beta = beta.min(MATE_SCORE - ply as i32 + 1);
            if alpha >= beta {
                return alpha;
            }
        }

        // Probe TT for best move and potential cutoff
        let (tt_move, tt_score, tt_bound, tt_cutoff) =
            self.probe_tt_for_cutoff(depth, alpha, beta, is_pv, excluded_move_active);
        node.tt_move = tt_move;
        node.tt_score = tt_score;
        node.tt_bound = tt_bound;
        if let Some(cutoff_score) = tt_cutoff {
            self.state.stats.tt_hits = self.state.stats.tt_hits.saturating_add(1);
            return cutoff_score;
        }

        // Generate moves
        let moves = self.board.generate_moves();
        if moves.is_empty() {
            return if in_check {
                -MATE_SCORE + ply as i32 // Checkmate
            } else {
                0 // Stalemate
            };
        }

        // Static evaluation for pruning decisions
        let eval = if in_check {
            -30000 // Don't use static eval when in check
        } else {
            self.evaluate_simple()
        };

        // Store eval for improving detection
        if ply < MAX_PLY {
            self.static_eval[ply] = eval;
        }

        let improving = self.is_improving(ply, eval);
        node.improving = improving;

        // ========================================================================
        // NODE-LEVEL PRUNING (before move loop)
        // ========================================================================

        if !is_pv && !in_check && !excluded_move_active {
            if let Some(score) =
                self.prune_before_move_loop(depth, alpha, beta, eval, &node, allow_null)
            {
                return score;
            }
        }

        // Internal Iterative Reduction disabled for now

        self.search_moves(&node, depth, alpha, beta, &moves)
    }
}
