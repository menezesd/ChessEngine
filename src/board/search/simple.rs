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

pub use iterative::{simple_search, simple_search_multipv};

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::tt::BoundType;

use super::constants::{
    COUNTER_SCORE, KILLER1_SCORE, KILLER2_SCORE, KILLER3_SCORE, LMR_SCORE_THRESHOLD,
    LMR_TABLE_MAX_DEPTH, LMR_TABLE_MAX_IDX, MATE_THRESHOLD, PAWN_EXTENSION_RANK_BLACK,
    PAWN_EXTENSION_RANK_WHITE, SCORE_INFINITE, SCORE_NEAR_MATE, SCORE_SAFE_MAX, TT_MOVE_SCORE,
};
use super::{SearchInfoCallback, SearchState, MATE_SCORE};
use crate::board::nnue::network::feature_index;
use crate::board::nnue::{NnueAccumulator, NnueNetwork};
use crate::board::{Board, Color, Move, MoveList, ScoredMoveList, Square, EMPTY_MOVE, MAX_PLY};
use crate::eval_math::{blended_eval, scaled_eval};

use super::super::Piece;

const SHORT_TIME_FUTILITY_LIMIT_MS: u64 = 300;
const SHORT_TIME_FUTILITY_MARGIN: i32 = 130;
const STATIC_TRACE_MAX_PLY: usize = 8;
const STATIC_TRACE_NODE_INTERVAL_LOG2: u32 = 6;

/// Result of an SMP preflight that may avoid creating worker threads.
pub(super) enum RootSearchResolution {
    /// The root is terminal or exactly resolved, with an optional legal move.
    Resolved(Option<Move>),
    /// Normal tree search is required.
    RequiresSearch,
}

/// Whether the side to move has a legal mate in one.
fn has_mate_in_one(board: &mut Board) -> bool {
    for mv in board.generate_moves() {
        let info = board.make_move(mv);
        let is_mate = board.is_checkmate();
        board.unmake_move(mv, info);

        if is_mate {
            return true;
        }
    }
    false
}

/// Convert a root-relative mate score into a TT-relative score.
///
/// The same position can be reached at a different ply through a
/// transposition. Storing mate scores without this adjustment makes a mate
/// found deeper in one line appear equally distant when probed from another.
#[inline]
fn score_to_tt(score: i32, ply: usize) -> i32 {
    let ply_score = ply.min(MAX_PLY) as i32;
    if score >= MATE_THRESHOLD {
        score.saturating_add(ply_score)
    } else if score <= -MATE_THRESHOLD {
        score.saturating_sub(ply_score)
    } else {
        score
    }
}

/// Convert a TT-relative mate score back into a root-relative score.
#[inline]
fn score_from_tt(score: i32, ply: usize) -> i32 {
    let ply_score = ply.min(MAX_PLY) as i32;
    if score >= MATE_THRESHOLD {
        score.saturating_sub(ply_score)
    } else if score <= -MATE_THRESHOLD {
        score.saturating_add(ply_score)
    } else {
        score
    }
}

/// Return a mating move for the exceptional K+NN versus K mate-in-one case,
/// or a drawing move for every other position in that material class.
pub(super) fn exact_two_knights_root_move(
    board: &mut Board,
    available_moves: &[Move],
) -> Option<Move> {
    let knight_side = board.two_knights_vs_bare_king_side()?;

    if board.side_to_move() != knight_side {
        // A pair of knights cannot force mate, but a bare king can still
        // blunder into an immediate mate.  Preserve the draw by selecting a
        // legal reply that does not allow one, rather than returning an
        // arbitrary king move.
        for &mv in available_moves {
            let info = board.make_move(mv);
            let permits_mate = has_mate_in_one(board);
            board.unmake_move(mv, info);

            if !permits_mate {
                return Some(mv);
            }
        }

        // K+NN cannot force mate against a bare king, so a safe move should
        // always exist in a legal position. Keep a legal fallback for
        // malformed positions and future move-generation changes.
        return available_moves.first().copied();
    }

    for &mv in available_moves {
        let info = board.make_move(mv);
        let is_mate = board.is_checkmate();
        board.unmake_move(mv, info);

        if is_mate {
            return Some(mv);
        }
    }

    available_moves.first().copied()
}

/// Resolve root positions that do not require tree expansion.
///
/// Used by SMP before worker creation. The regular search path performs the
/// equivalent checks after filtering `MultiPV` exclusions.
pub(super) fn immediate_root_result(board: &mut Board) -> RootSearchResolution {
    let moves = board.generate_moves();
    if moves.is_empty() {
        return RootSearchResolution::Resolved(None);
    }
    if moves.len() == 1 || board.is_theoretical_draw() {
        return RootSearchResolution::Resolved(moves.first());
    }

    exact_two_knights_root_move(board, moves.as_slice())
        .map_or(RootSearchResolution::RequiresSearch, |best_move| {
            RootSearchResolution::Resolved(Some(best_move))
        })
}

#[inline]
pub(super) fn effective_futility_margin(time_limit_ms: u64, configured_margin: i32) -> i32 {
    if time_limit_ms > 0 && time_limit_ms <= SHORT_TIME_FUTILITY_LIMIT_MS {
        configured_margin.min(SHORT_TIME_FUTILITY_MARGIN)
    } else {
        configured_margin
    }
}

/// Search context for a single search
pub struct SimpleSearchContext<'a> {
    pub board: &'a mut Board,
    pub state: &'a mut SearchState,
    pub stop: &'a AtomicBool,
    pub start_time: Instant,
    pub time_limit_ms: u64,
    pub node_limit: u64,
    pub nodes: u64,
    pub futility_margin: i32,
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
    /// Optional static-eval NNUE accumulator stack indexed by ply.
    pub static_acc_stack: Box<[NnueAccumulator]>,
}

#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct NodeContext {
    ply: usize,
    is_pv: bool,
    in_check: bool,
    improving: bool,
    excluded_move: Move,
    tt_move: Move,
    tt_score: i32,
    tt_bound: BoundType,
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

/// Result of trying TT move before full move generation
struct StagedMoveResult {
    /// Score from searching the TT move
    score: i32,
    /// Whether TT move raised alpha (but didn't cause cutoff)
    raised_alpha: bool,
}

impl SimpleSearchContext<'_> {
    fn update_accumulator_stack_for_move(
        board: &Board,
        stack: &mut [NnueAccumulator],
        network: &NnueNetwork,
        ply: usize,
        m: Move,
        moving_piece: Piece,
        moving_color: Color,
    ) {
        if ply + 1 >= stack.len() {
            return;
        }

        stack[ply + 1] = stack[ply].clone();
        let acc = &mut stack[ply + 1];

        let feat = |piece: Piece, color: Color, sq: usize| -> (usize, usize) {
            (
                feature_index(piece.index(), color.index(), sq, 0),
                feature_index(piece.index(), color.index(), sq, 1),
            )
        };

        if m.is_castling() {
            let (wf, bf) = feat(Piece::King, moving_color, m.from().index());
            acc.sub_feature(wf, bf, network);
            let (wf, bf) = feat(Piece::King, moving_color, m.to().index());
            acc.add_feature(wf, bf, network);

            let (rook_from_file, rook_to_file) = if m.to().file() == 6 { (7, 5) } else { (0, 3) };
            let rank = m.from().rank();
            let rook_from = Square::new(rank, rook_from_file).index();
            let rook_to = Square::new(rank, rook_to_file).index();
            let (wf, bf) = feat(Piece::Rook, moving_color, rook_from);
            acc.sub_feature(wf, bf, network);
            let (wf, bf) = feat(Piece::Rook, moving_color, rook_to);
            acc.add_feature(wf, bf, network);
        } else {
            if m.is_en_passant() {
                let cap_rank = if moving_color == Color::White {
                    m.to().rank() - 1
                } else {
                    m.to().rank() + 1
                };
                let cap_sq = Square::new(cap_rank, m.to().file()).index();
                let (wf, bf) = feat(Piece::Pawn, moving_color.opponent(), cap_sq);
                acc.sub_feature(wf, bf, network);
            } else if m.is_capture() {
                if let Some((cap_color, cap_piece)) = board.piece_at(m.to()) {
                    let (wf, bf) = feat(cap_piece, cap_color, m.to().index());
                    acc.sub_feature(wf, bf, network);
                }
            }

            let (wf, bf) = feat(moving_piece, moving_color, m.from().index());
            acc.sub_feature(wf, bf, network);

            let placed_piece = m.promotion().unwrap_or(moving_piece);
            let (wf, bf) = feat(placed_piece, moving_color, m.to().index());
            acc.add_feature(wf, bf, network);
        }
    }

    #[inline]
    fn evaluate_static_nnue(&self, ply: usize) -> Option<i32> {
        let static_nnue = self.state.tables.static_nnue.as_ref()?;
        if ply >= self.static_acc_stack.len() {
            return None;
        }
        let score = if static_nnue.supports_tactical_features() {
            let mut acc = self.static_acc_stack[ply].clone();
            self.board.add_nnue_dynamic_features(&mut acc, static_nnue);
            static_nnue.evaluate(&acc, self.board.white_to_move)
        } else {
            static_nnue.evaluate(&self.static_acc_stack[ply], self.board.white_to_move)
        };
        Some(scaled_eval(score, self.state.nnue_static_eval_scale))
    }

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

    /// Check if a quiet move should be pruned (futility pruning or LMP)
    fn should_prune_quiet(
        &self,
        ctx: &MoveContext,
        node: &NodeContext,
        depth: u32,
        moves_tried: usize,
        alpha: i32,
    ) -> bool {
        if !ctx.is_quiet || node.in_check || ctx.gives_check || node.is_pv {
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
            if static_eval + futility_margin <= alpha {
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

    /// Extract Principal Variation from TT
    /// Returns a vector of moves representing the best line
    fn extract_pv(&mut self, max_len: usize) -> Vec<Move> {
        self.extract_pv_with_first_move_opt(None, max_len)
    }

    /// Extract PV with a specific first move, then continue from TT
    /// Used for `MultiPV` to ensure the PV starts with the correct best move
    fn extract_pv_with_first_move(&mut self, first_move: Move, max_len: usize) -> Vec<Move> {
        self.extract_pv_with_first_move_opt(Some(first_move), max_len)
    }

    /// Extract PV, optionally starting with a specified first move
    fn extract_pv_with_first_move_opt(
        &mut self,
        first_move: Option<Move>,
        max_len: usize,
    ) -> Vec<Move> {
        let mut pv = Vec::with_capacity(max_len);
        // Use fixed array instead of HashSet - max_len is bounded by MAX_PLY
        let mut seen_hashes = [0u64; MAX_PLY];
        let mut unmake_infos = Vec::with_capacity(max_len);

        let effective_max = max_len.min(MAX_PLY);
        for (seen_count, _) in (0..effective_max).enumerate() {
            // Avoid infinite loops from TT collisions - linear scan is faster for small N
            let hash = self.board.hash;
            if seen_hashes[..seen_count].contains(&hash) {
                break;
            }
            seen_hashes[seen_count] = hash;

            // For the first move, use provided first_move if given
            let mv = if let (0, Some(fm)) = (seen_count, first_move) {
                fm
            } else {
                // Get best move from TT
                let tt_move = if let Some(entry) = self.state.tables.tt.probe(self.board.hash) {
                    entry.best_move()
                } else {
                    None
                };

                let Some(m) = tt_move else { break };
                if m == EMPTY_MOVE {
                    break;
                }
                m
            };

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
        let mut scored_moves = self.order_moves(moves, node.tt_move, ply, prev_move);

        let move_count = scored_moves.len();
        let tt_tactical = node.tt_move.is_capture() || node.tt_move.is_promotion();

        // If TT move was already searched (staged), use its result as initial state
        let (mut best_score, mut best_move, mut raised_alpha, tt_move_searched) =
            if let Some(result) = staged {
                // TT move was searched, didn't cause cutoff
                // Update alpha if TT move raised it
                if result.raised_alpha {
                    alpha = result.score;
                }
                (result.score, node.tt_move, result.raised_alpha, true)
            } else {
                (-SCORE_INFINITE, EMPTY_MOVE, false, false)
            };

        let mut moves_tried = usize::from(tt_move_searched);

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

            // Skip TT move if already searched in staged generation
            if tt_move_searched && m == node.tt_move {
                continue;
            }

            // Track if this is a quiet move
            let is_quiet = !m.is_capture() && !m.is_promotion();

            // SEE pruning for quiet moves at shallow depths
            // Skip moves that lose material by moving to an attacked square
            if is_quiet
                && depth <= 3
                && !in_check
                && move_count > 1
                && !self.board.see_quiet_safe(m.from(), m.to())
            {
                continue;
            }

            // Get the piece that's moving for continuation history (before make_move)
            let moving_piece = self.board.piece_at(m.from()).map(|(_, p)| p);

            // Update NNUE accumulator incrementally (before make_move modifies board)
            if let Some(piece) = moving_piece {
                self.update_accumulator_for_move(ply, m, piece, self.board.side_to_move());
            }

            // Make move first (we'll check for check after)
            let info = self.board.make_move(m);

            // Prefetch TT entry for this position to hide memory latency
            // By the time we call alphabeta, the cache line should be loaded
            self.state.tables.tt.prefetch(self.board.hash);

            // Check if move gives check
            let gives_check = self.board.is_in_check(self.board.side_to_move());

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
            if self.should_prune_quiet(&move_ctx, node, depth, moves_tried, alpha) {
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

            // LMR reduction
            let reduction = self.compute_lmr_reduction(
                i - 1,
                move_count,
                depth,
                node,
                &move_ctx,
                tt_tactical || gives_check || m.is_capture(),
            );

            // Compute extensions
            let extension = Self::compute_extensions(&move_ctx, node);

            let new_depth = if move_count == 1 {
                depth + extension
            } else {
                depth.saturating_sub(1) + extension
            };

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

            self.board.unmake_move(m, info);

            if self.should_stop() {
                break;
            }

            if score > best_score {
                best_score = score;
                best_move = m;

                if score > alpha {
                    if score >= beta {
                        // Penalize quiet moves that didn't cause the cutoff (negative history)
                        // Don't penalize the cutoff move itself
                        for quiet_mv in quiets_tried.iter().take(quiets_count) {
                            if *quiet_mv != m && *quiet_mv != EMPTY_MOVE {
                                self.state.tables.history.penalize(quiet_mv, depth);
                            }
                        }
                        self.handle_beta_cutoff(m, ply, depth, score, best_move);
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
            return if in_check {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }

        self.store_tt(depth, best_score, raised_alpha, best_move, ply);

        // Update correction history for exact bounds (when we have reliable score vs static eval)
        if raised_alpha && ply < MAX_PLY && !in_check && best_score.abs() < SCORE_NEAR_MATE {
            let pawn_hash = self.board.pawn_hash();
            let raw_eval = self.static_eval[ply];
            // Remove the previously applied correction to get raw eval
            let old_correction = self.state.tables.correction_history.get(pawn_hash);
            let static_eval_raw = raw_eval.saturating_sub(old_correction);
            self.state.tables.correction_history.update(
                pawn_hash,
                static_eval_raw,
                best_score,
                depth,
            );
        }

        best_score
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
        if self.node_limit > 0 && self.nodes >= self.node_limit {
            return true;
        }
        if self.time_limit_ms > 0 && self.nodes.trailing_zeros() >= 10 {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if elapsed >= self.time_limit_ms {
                return true;
            }
        }

        false
    }

    /// Account for a searched node without crossing a caller's hard node cap.
    #[inline]
    fn try_visit_node(&mut self) -> bool {
        if self.should_stop() {
            return false;
        }
        self.nodes = self.nodes.saturating_add(1);
        true
    }

    /// Evaluate position from side-to-move's perspective.
    /// Uses NNUE with incremental accumulator if available, otherwise HCE.
    #[inline]
    fn evaluate(&self, ply: usize) -> i32 {
        let hce_eval = || {
            if self.state.hce_options.use_full {
                if self.state.hce_options.use_tuned {
                    self.board
                        .evaluate_tuned_hce_cached(&self.state.tables.pawn_hash)
                } else {
                    self.board.evaluate_cached(&self.state.tables.pawn_hash)
                }
            } else {
                self.board.evaluate_simple()
            }
        };
        if let Some(ref nnue) = self.state.tables.nnue {
            let nnue_score = if nnue.supports_tactical_features() {
                let mut acc = self.acc_stack[ply].clone();
                self.board.add_nnue_dynamic_features(&mut acc, nnue);
                nnue.evaluate(&acc, self.board.white_to_move)
            } else {
                nnue.evaluate(&self.acc_stack[ply], self.board.white_to_move)
            };
            let nnue_eval = scaled_eval(nnue_score, self.state.nnue_eval_scale);
            if self.state.nnue_hce_blend >= 100 {
                nnue_eval
            } else {
                blended_eval(nnue_eval, hce_eval(), self.state.nnue_hce_blend)
            }
        } else {
            hce_eval()
        }
    }

    /// Evaluation for pruning and qsearch (main workhorse).
    /// Uses HCE by default for qsearch/pruning stability. `NnuePureStaticEval`
    /// switches this to NNUE for no-HCE experiments.
    #[inline]
    fn evaluate_simple(&self, ply: usize) -> i32 {
        if self.state.trace
            && ply <= STATIC_TRACE_MAX_PLY
            && self.nodes.trailing_zeros() >= STATIC_TRACE_NODE_INTERVAL_LOG2
        {
            println!("info string staticevalfen {}", self.board.to_fen());
        }

        let hce_eval = || {
            if self.state.static_eval_options.use_full_hce {
                if self.state.hce_options.use_tuned {
                    self.board
                        .evaluate_tuned_hce_cached(&self.state.tables.pawn_hash)
                } else {
                    self.board.evaluate_cached(&self.state.tables.pawn_hash)
                }
            } else {
                self.board.evaluate_simple()
            }
        };

        if self.state.nnue_static_blend <= 0 && self.state.tables.static_nnue.is_some() {
            hce_eval()
        } else if let Some(nnue_eval) = self.evaluate_static_nnue(ply) {
            if self.state.nnue_static_blend >= 100 {
                nnue_eval
            } else {
                blended_eval(nnue_eval, hce_eval(), self.state.nnue_static_blend)
            }
        } else if self.state.static_eval_options.nnue_pure {
            self.evaluate(ply)
        } else if self.state.static_eval_options.use_full_hce {
            if self.state.hce_options.use_tuned {
                self.board
                    .evaluate_tuned_hce_cached(&self.state.tables.pawn_hash)
            } else {
                self.board.evaluate_cached(&self.state.tables.pawn_hash)
            }
        } else {
            self.board.evaluate_simple()
        }
    }

    /// Initialize the accumulator at the given ply from the current board state.
    fn init_accumulator(&mut self, ply: usize) {
        if let Some(ref nnue) = self.state.tables.nnue {
            let (wf, bf) = self.board.compute_nnue_features();
            self.acc_stack[ply] = NnueAccumulator::new(&nnue.feature_bias);
            self.acc_stack[ply].refresh(&wf, &bf, nnue);
        }
        if let Some(ref static_nnue) = self.state.tables.static_nnue {
            let (wf, bf) = self.board.compute_nnue_features();
            self.static_acc_stack[ply] = NnueAccumulator::new(&static_nnue.feature_bias);
            self.static_acc_stack[ply].refresh(&wf, &bf, static_nnue);
        }
    }

    /// Update accumulator incrementally for a move.
    /// Must be called BEFORE `board.make_move()` while the board is in the pre-move state.
    fn update_accumulator_for_move(
        &mut self,
        ply: usize,
        m: Move,
        moving_piece: Piece,
        moving_color: Color,
    ) {
        if let Some(ref nnue) = self.state.tables.nnue {
            Self::update_accumulator_stack_for_move(
                self.board,
                &mut self.acc_stack,
                nnue,
                ply,
                m,
                moving_piece,
                moving_color,
            );
        }
        if let Some(ref static_nnue) = self.state.tables.static_nnue {
            Self::update_accumulator_stack_for_move(
                self.board,
                &mut self.static_acc_stack,
                static_nnue,
                ply,
                m,
                moving_piece,
                moving_color,
            );
        }
    }

    /// Copy accumulator forward for null moves (no pieces change).
    #[inline]
    fn copy_accumulator_for_null_move(&mut self, ply: usize) {
        if ply + 1 < self.acc_stack.len() {
            self.acc_stack[ply + 1] = self.acc_stack[ply].clone();
        }
        if ply + 1 < self.static_acc_stack.len() {
            self.static_acc_stack[ply + 1] = self.static_acc_stack[ply].clone();
        }
    }

    /// Check for repetition (returns true if position repeated)
    #[inline]
    fn is_repetition(&self) -> bool {
        self.board.repetition_counts.get(self.board.hash) > 1
    }

    /// Score the exact K+NN versus K material class without expanding a
    /// non-forcing search tree. Mate-in-one positions must be searched first:
    /// two knights cannot force mate, but legal mating positions do exist.
    fn two_knights_vs_bare_king_score(&mut self, ply: usize) -> Option<i32> {
        let knight_side = self.board.two_knights_vs_bare_king_side()?;
        let moves = self.board.generate_moves();

        if moves.is_empty() {
            return Some(if self.board.is_in_check(self.board.side_to_move()) {
                -MATE_SCORE + ply as i32
            } else {
                0
            });
        }

        if self.board.side_to_move() == knight_side {
            for m in &moves {
                let info = self.board.make_move(*m);
                let is_mate = self.board.is_checkmate();
                self.board.unmake_move(*m, info);

                if is_mate {
                    return Some(MATE_SCORE - ply as i32 - 1);
                }
            }
        }

        Some(0)
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
        &mut self,
        moves: &MoveList,
        tt_move: Move,
        ply: usize,
        prev_move: Move,
    ) -> ScoredMoveList {
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

        let mut scored = ScoredMoveList::new();
        for m in moves {
            let score = if *m == tt_move {
                TT_MOVE_SCORE
            } else if ply < MAX_PLY && *m == self.state.tables.killer_moves.primary(ply) {
                KILLER1_SCORE
            } else if ply < MAX_PLY && *m == self.state.tables.killer_moves.secondary(ply) {
                KILLER2_SCORE
            } else if ply < MAX_PLY && *m == self.state.tables.killer_moves.tertiary(ply) {
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
        scored
    }

    /// Handle beta cutoff: update killers, history, counter moves, continuation history, and TT
    fn handle_beta_cutoff(&mut self, m: Move, ply: usize, depth: u32, score: i32, best_move: Move) {
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

        // Store in TT (allow mate scores too)
        if !self.should_stop() {
            self.state.tables.tt.store(
                self.board.hash,
                depth,
                score_to_tt(score, ply),
                BoundType::LowerBound,
                Some(best_move),
                self.state.generation,
            );
        }
    }

    /// Store position in transposition table
    fn store_tt(
        &mut self,
        depth: u32,
        score: i32,
        raised_alpha: bool,
        best_move: Move,
        ply: usize,
    ) {
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
            score_to_tt(score, ply),
            bound,
            Some(best_move),
            self.state.generation,
        );
    }

    /// Probe TT and check for cutoff.
    /// Returns (`tt_move`, `tt_score`, `tt_bound`, `tt_depth`, `Option<cutoff_score>`)
    fn probe_tt_for_cutoff(
        &self,
        depth: u32,
        alpha: i32,
        beta: i32,
        ply: usize,
        is_pv: bool,
        excluded_move_active: bool,
    ) -> (Move, i32, BoundType, u32, Option<i32>) {
        let Some(entry) = self.state.tables.tt.probe(self.board.hash) else {
            return (EMPTY_MOVE, 0, BoundType::Exact, 0, None);
        };

        let tt_move = entry.best_move().unwrap_or(EMPTY_MOVE);
        let tt_score = score_from_tt(entry.score(), ply);
        let tt_bound = entry.bound_type();
        let tt_depth = entry.depth();

        // Check for cutoff
        if !excluded_move_active && entry.depth() >= depth && !self.is_repetition() {
            let score = tt_score;
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
            return (tt_move, tt_score, tt_bound, tt_depth, cutoff);
        }

        (tt_move, tt_score, tt_bound, tt_depth, None)
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

        let is_root = ply == 0;
        let is_pv = beta > alpha + 1;
        let excluded_move_active = excluded_move != EMPTY_MOVE;
        let mut node = NodeContext {
            ply,
            is_pv,
            in_check: false,
            improving: false,
            excluded_move,
            tt_move: EMPTY_MOVE,
            tt_score: 0,
            tt_bound: BoundType::Exact,
            singular_extension: 0,
        };

        // A repeated position on the current line can be claimed as a draw.
        // The board-level rules also cover the fifty-move rule and positions
        // with insufficient mating material. Do this before probing the TT: a
        // transposition entry does not encode the halfmove clock or history.
        if !is_root && (self.is_repetition() || self.board.is_theoretical_draw()) {
            return 0;
        }

        // K+NN versus K is theoretically drawn except for an existing mate
        // or a mate in one after a defender blunder. Resolve that exceptional
        // case exactly, then avoid spending depth on a non-forcing ending.
        if !is_root {
            if let Some(score) = self.two_knights_vs_bare_king_score(ply) {
                return score;
            }
        }

        // Extensions can make the game-tree ply exceed the nominal search
        // depth.  Keep recursion inside the fixed heuristic stacks rather
        // than risking an out-of-bounds continuation/NNUE access in an
        // unusually long checking sequence.
        if ply >= MAX_PLY {
            return self.evaluate_simple(ply);
        }

        // Quiescence at leaf
        if depth == 0 {
            return self.quiesce(alpha, beta, ply, 0);
        }

        if !self.try_visit_node() {
            return 0;
        }
        if (ply as u32 + 1) > self.state.stats.seldepth {
            self.state.stats.seldepth = ply as u32 + 1;
        }

        // Check for missing king (illegal position)
        let in_check = self.board.is_in_check(self.board.side_to_move());
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
        let (tt_move, tt_score, tt_bound, tt_depth, tt_cutoff) =
            self.probe_tt_for_cutoff(depth, alpha, beta, ply, is_pv, excluded_move_active);
        node.tt_move = tt_move;
        node.tt_score = tt_score;
        node.tt_bound = tt_bound;

        // At root with restricted moves (MultiPV), only use TT cutoff if TT move is in root_moves
        let use_tt_cutoff = if is_root && !self.root_moves.is_empty() {
            tt_move != EMPTY_MOVE && self.root_moves.contains(&tt_move)
        } else {
            true
        };

        if use_tt_cutoff {
            if let Some(cutoff_score) = tt_cutoff {
                self.state.stats.tt_hits = self.state.stats.tt_hits.saturating_add(1);
                return cutoff_score;
            }
        }

        // Static evaluation for pruning decisions (needed before node-level pruning)
        let raw_eval = if in_check {
            -SCORE_INFINITE // Don't use static eval when in check
        } else {
            self.evaluate_simple(ply)
        };

        // Apply correction history to improve static eval accuracy
        let eval = if in_check {
            raw_eval
        } else {
            let pawn_hash = self.board.pawn_hash();
            let correction = self.state.tables.correction_history.get(pawn_hash);
            (raw_eval + correction).clamp(-SCORE_SAFE_MAX, SCORE_SAFE_MAX)
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
            && tt_depth + 3 >= depth
            && tt_score.abs() < MATE_THRESHOLD
            && matches!(tt_bound, BoundType::LowerBound | BoundType::Exact)
            && self.board.is_legal_move(tt_move)
        {
            let singular_beta = tt_score - SINGULAR_MARGIN * depth as i32;
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

        // Generate moves (at root, use root_moves if available for MultiPV)
        let moves = if is_root && !self.root_moves.is_empty() {
            let mut move_list = MoveList::new();
            for m in &self.root_moves {
                move_list.push(*m);
            }
            move_list
        } else {
            self.board.generate_moves()
        };

        // Handle empty move list (checkmate/stalemate)
        // Note: if staged_result is Some, TT move was legal so we have at least one move
        if moves.is_empty() {
            return if let Some(result) = staged_result {
                // TT move was the only legal move, return its score
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
        {
            depth - 1
        } else {
            depth
        };

        // Convert staged_result: if it caused cutoff we already returned above,
        // so here it's either None or Some with score < beta
        let staged = staged_result.filter(|r| r.score < beta);

        self.search_moves(&node, search_depth, alpha, beta, &moves, staged)
    }

    /// Try the TT move before generating all moves.
    /// Returns Some(result) if TT move was legal and searched.
    /// The result contains the score and whether it raised alpha.
    /// If score >= beta, caller should return immediately (beta cutoff).
    fn try_tt_move_first(
        &mut self,
        tt_move: Move,
        node: &NodeContext,
        depth: u32,
        alpha: i32,
        beta: i32,
    ) -> Option<StagedMoveResult> {
        // Validate TT move is legal (much cheaper than generating all moves)
        if !self.board.is_legal_move(tt_move) {
            return None;
        }

        let ply = node.ply;

        // Get the piece that's moving for continuation history
        let moving_piece = self.board.piece_at(tt_move.from()).map(|(_, p)| p);

        // Update NNUE accumulator incrementally (before make_move modifies board)
        if let Some(piece) = moving_piece {
            self.update_accumulator_for_move(ply, tt_move, piece, self.board.side_to_move());
        }

        // Make the move
        let info = self.board.make_move(tt_move);

        // Prefetch TT entry for the new position
        self.state.tables.tt.prefetch(self.board.hash);

        // Check if move gives check
        let gives_check = self.board.is_in_check(self.board.side_to_move());

        if ply < MAX_PLY {
            self.previous_move[ply] = tt_move;
            self.previous_piece[ply] = moving_piece;
        }

        // Compute extensions for TT move
        let is_quiet = !tt_move.is_capture() && !tt_move.is_promotion();
        let move_ctx = MoveContext {
            m: tt_move,
            move_score: TT_MOVE_SCORE,
            is_quiet,
            gives_check,
            moving_piece,
        };
        let extension = Self::compute_extensions(&move_ctx, node);
        // Standard depth reduction when descending into child node
        let new_depth = depth.saturating_sub(1) + extension;

        // Full window search for TT move (it's the first move)
        let score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);

        self.board.unmake_move(tt_move, info);

        if self.should_stop() {
            return None;
        }

        // Check for beta cutoff
        if score >= beta {
            // Update history heuristics on cutoff
            self.handle_beta_cutoff(tt_move, ply, depth, score, tt_move);
            return Some(StagedMoveResult {
                score,
                raised_alpha: true,
            });
        }

        // TT move didn't cause cutoff, but may have raised alpha
        Some(StagedMoveResult {
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

    use super::{NodeContext, SimpleSearchContext, MATE_SCORE, SCORE_INFINITE};
    use crate::tt::BoundType;

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
        let futility_margin = state.params.futility_margin;
        let ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            node_limit: 0,
            nodes: 0,
            futility_margin,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };

        assert_eq!(ctx.evaluate_simple(0), expected);
    }

    #[test]
    fn interrupted_move_loop_returns_neutral_score_not_terminal_score() {
        let mut board = Board::new();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(true);
        let futility_margin = state.params.futility_margin;
        let mut ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            node_limit: 0,
            nodes: 0,
            futility_margin,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };
        let moves = ctx.board.generate_moves();
        let node = NodeContext {
            ply: 0,
            is_pv: false,
            in_check: false,
            improving: false,
            excluded_move: EMPTY_MOVE,
            tt_move: EMPTY_MOVE,
            tt_score: 0,
            tt_bound: BoundType::Exact,
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

        assert_eq!(super::score_to_tt(winning_score, 10), MATE_SCORE);
        assert_eq!(super::score_from_tt(MATE_SCORE, 4), MATE_SCORE - 4);
        assert_eq!(super::score_to_tt(losing_score, 10), -MATE_SCORE);
        assert_eq!(super::score_from_tt(-MATE_SCORE, 4), -MATE_SCORE + 4);

        for score in [winning_score, losing_score, 437, -437] {
            assert_eq!(
                super::score_from_tt(super::score_to_tt(score, 10), 10),
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
        let futility_margin = state.params.futility_margin;
        let score_at_storage_ply = MATE_SCORE - 8;
        state.tables.tt.store(
            board.hash(),
            4,
            super::score_to_tt(score_at_storage_ply, 8),
            BoundType::Exact,
            None,
            state.generation,
        );
        let ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            node_limit: 0,
            nodes: 0,
            futility_margin,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };

        let (_, score, _, _, cutoff) =
            ctx.probe_tt_for_cutoff(4, -SCORE_INFINITE, SCORE_INFINITE, 3, false, false);

        assert_eq!(score, MATE_SCORE - 3);
        assert_eq!(cutoff, Some(MATE_SCORE - 3));
    }

    #[test]
    fn search_ply_limit_returns_static_score_without_descending() {
        let mut board = Board::new();
        let expected = board.evaluate_simple();
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let futility_margin = state.params.futility_margin;
        let mut ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            node_limit: 0,
            nodes: 0,
            futility_margin,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };

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
        assert_eq!(ctx.nodes, 0, "ply-limit guard must not expand a node");
    }
}
