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

mod accumulator;
mod context;
mod iterative;
mod move_ordering;
mod move_search;
mod pruning;
mod pv;
mod quiescence;
#[cfg(test)]
mod tests;
mod transposition;

pub use iterative::{simple_search, simple_search_multipv, SimpleSearchRequest};

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::tt::BoundType;

use super::config::duration_millis_saturating;
use super::constants::{MATE_THRESHOLD, SCORE_INFINITE, SCORE_SAFE_MAX};
use super::{SearchInfoCallback, SearchState, MATE_SCORE};
use crate::board::nnue::NnueAccumulator;
use crate::board::{Board, Move, MoveList, EMPTY_MOVE, MAX_PLY};

use super::super::Piece;

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
    /// Previous piece type at each ply for continuation history
    pub previous_piece: [Option<Piece>; MAX_PLY],
    /// Optional callback for reporting iteration info
    pub info_callback: Option<SearchInfoCallback>,
    /// Root moves to consider (for `MultiPV` support - empty means all moves)
    pub root_moves: Vec<Move>,
    /// NNUE accumulator stack indexed by ply (heap-allocated)
    pub acc_stack: Box<[NnueAccumulator]>,
    /// Static-eval NNUE accumulator stack indexed by ply (heap-allocated)
    pub static_acc_stack: Box<[NnueAccumulator]>,
}

pub(super) use context::{MoveContext, NodeContext, StagedMoveResult};

fn increment_node_count(nodes: &mut u64) {
    *nodes = nodes.saturating_add(1);
}

fn is_pv_window(alpha: i32, beta: i32) -> bool {
    beta > alpha.saturating_add(1)
}

fn mate_score_for_ply(sign: i32, ply: usize) -> i32 {
    let ply = i64::try_from(ply).unwrap_or(i64::MAX);
    let score = if sign < 0 {
        -i64::from(MATE_SCORE) + ply
    } else {
        i64::from(MATE_SCORE) - ply
    };
    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn singular_beta(tt_score: i32, margin: i32, depth: u32) -> i32 {
    let score = i64::from(tt_score) - i64::from(margin) * i64::from(depth);
    score.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn null_window_upper(alpha: i32) -> i32 {
    alpha.saturating_add(1)
}

impl SimpleSearchContext<'_> {
    #[inline]
    fn elapsed_ms(&self) -> u64 {
        duration_millis_saturating(self.start_time.elapsed())
    }

    fn node_context(ply: usize, alpha: i32, beta: i32, excluded_move: Move) -> NodeContext {
        NodeContext {
            ply,
            is_pv: is_pv_window(alpha, beta),
            in_check: false,
            improving: false,
            excluded_move,
            tt_move: EMPTY_MOVE,
            tt_score: 0,
            tt_bound: BoundType::Exact,
            singular_extension: 0,
        }
    }

    fn should_use_tt_cutoff(&self, is_root: bool, tt_move: Move) -> bool {
        if is_root && !self.root_moves.is_empty() {
            tt_move != EMPTY_MOVE && self.root_moves.contains(&tt_move)
        } else {
            true
        }
    }

    fn moves_for_node(&mut self, is_root: bool) -> MoveList {
        if is_root && !self.root_moves.is_empty() {
            let mut move_list = MoveList::new();
            for m in &self.root_moves {
                move_list.push(*m);
            }
            move_list
        } else {
            self.board.generate_moves()
        }
    }

    fn empty_move_score(
        in_check: bool,
        ply: usize,
        staged_result: Option<&StagedMoveResult>,
    ) -> i32 {
        if let Some(result) = staged_result {
            result.score
        } else if in_check {
            mate_score_for_ply(-1, ply)
        } else {
            0
        }
    }

    fn singular_extension(
        &mut self,
        depth: u32,
        node: &NodeContext,
        is_root: bool,
        excluded_move_active: bool,
    ) -> u32 {
        const SINGULAR_MIN_DEPTH: u32 = 6;
        const SINGULAR_MARGIN: i32 = 3;

        if excluded_move_active
            || is_root
            || depth < SINGULAR_MIN_DEPTH
            || node.tt_move == EMPTY_MOVE
            || node.tt_score.abs() >= MATE_THRESHOLD
            || !matches!(node.tt_bound, BoundType::LowerBound | BoundType::Exact)
        {
            return 0;
        }

        let singular_beta = singular_beta(node.tt_score, SINGULAR_MARGIN, depth);
        let singular_depth = (depth - 1) / 2;
        let singular_score = self.alphabeta(
            singular_depth,
            singular_beta.saturating_sub(1),
            singular_beta,
            false,
            node.ply,
            node.tt_move,
        );

        u32::from(singular_score < singular_beta)
    }

    fn corrected_static_eval(&mut self, in_check: bool, ply: usize) -> i32 {
        if in_check {
            return -SCORE_INFINITE;
        }

        let raw_eval = self.evaluate_simple(ply);
        let pawn_hash = self.board.pawn_hash();
        let correction = self.state.tables.correction_history.get(pawn_hash);
        raw_eval
            .saturating_add(correction)
            .clamp(-SCORE_SAFE_MAX, SCORE_SAFE_MAX)
    }

    fn update_node_eval_state(&mut self, node: &mut NodeContext, ply: usize, eval: i32) {
        if ply < MAX_PLY {
            self.static_eval[ply] = eval;
        }
        node.improving = self.is_improving(ply, eval);
    }

    fn apply_mate_distance_pruning(alpha: &mut i32, beta: &mut i32, ply: usize) -> Option<i32> {
        *alpha = (*alpha).max(mate_score_for_ply(-1, ply));
        *beta = (*beta).min(null_window_upper(mate_score_for_ply(1, ply)));
        if *alpha >= *beta {
            Some(*alpha)
        } else {
            None
        }
    }

    fn enter_search_node(&mut self, ply: usize) -> Option<i32> {
        increment_node_count(&mut self.nodes);
        if (ply as u32 + 1) > self.state.stats.seldepth {
            self.state.stats.seldepth = ply as u32 + 1;
        }

        if self.should_stop() {
            return Some(0);
        }

        None
    }

    /// Alpha-beta search with all pruning and extension techniques
    pub fn alphabeta(
        &mut self,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        allow_null: bool,
        ply: usize,
        excluded_move: Move,
    ) -> i32 {
        let is_root = ply == 0;
        let excluded_move_active = excluded_move != EMPTY_MOVE;
        let mut node = Self::node_context(ply, alpha, beta, excluded_move);

        // Repetition check
        if !is_root && self.is_repetition() {
            return 0;
        }

        // Quiescence at leaf
        if depth == 0 {
            return self.quiesce(alpha, beta, ply, 0);
        }

        if let Some(score) = self.enter_search_node(ply) {
            return score;
        }

        let in_check = self.board.is_in_check(self.board.side_to_move());
        node.in_check = in_check;

        // Mate distance pruning
        if !is_root {
            if let Some(score) = Self::apply_mate_distance_pruning(&mut alpha, &mut beta, ply) {
                return score;
            }
        }

        // Probe TT for best move and potential cutoff
        let (tt_move, tt_score, tt_bound, tt_cutoff) =
            self.probe_tt_for_cutoff(depth, alpha, beta, node.is_pv, excluded_move_active);
        node.tt_move = tt_move;
        node.tt_score = tt_score;
        node.tt_bound = tt_bound;

        if self.should_use_tt_cutoff(is_root, tt_move) {
            if let Some(cutoff_score) = tt_cutoff {
                self.state.stats.tt_hits = self.state.stats.tt_hits.saturating_add(1);
                return cutoff_score;
            }
        }

        let eval = self.corrected_static_eval(in_check, ply);
        self.update_node_eval_state(&mut node, ply, eval);

        // ========================================================================
        // NODE-LEVEL PRUNING (before move loop)
        // ========================================================================

        if !node.is_pv && !in_check && !excluded_move_active {
            if let Some(score) =
                self.prune_before_move_loop(depth, alpha, beta, eval, &node, allow_null)
            {
                return score;
            }
        }

        // ========================================================================
        // STAGED MOVE GENERATION: Try TT move before generating all moves
        // ========================================================================
        // After node-level pruning, try TT move first. If it causes a beta cutoff,
        // we avoid the cost of generating all moves.
        let staged_result = if tt_move != EMPTY_MOVE
            && !excluded_move_active
            && !is_root // At root we need all moves for proper PV
            && !in_check
        // Simpler handling when not in check
        {
            self.try_tt_move_first(tt_move, &node, depth, alpha, beta)
        } else {
            None
        };

        // If TT move caused beta cutoff, return immediately
        if let Some(ref result) = staged_result {
            if result.score >= beta {
                return result.score;
            }
        }

        let moves = self.moves_for_node(is_root);

        // Handle empty move list (checkmate/stalemate)
        // Note: if staged_result is Some, TT move was legal so we have at least one move
        if moves.is_empty() {
            return Self::empty_move_score(in_check, ply, staged_result.as_ref());
        }

        node.singular_extension =
            self.singular_extension(depth, &node, is_root, excluded_move_active);

        // Internal Iterative Reduction (IIR)
        // If we have no TT move at high depth, reduce depth to find a move faster
        let search_depth = if tt_move == EMPTY_MOVE && depth >= 4 && !excluded_move_active {
            depth - 1
        } else {
            depth
        };

        // Convert staged_result: if it caused cutoff we already returned above,
        // so here it's either None or Some with score < beta
        let staged = staged_result.filter(|r| r.score < beta);

        self.search_moves(&node, search_depth, alpha, beta, &moves, staged)
    }
}
