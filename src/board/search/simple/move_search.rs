use std::sync::OnceLock;

use crate::board::{Move, MoveList, Piece, EMPTY_MOVE, MAX_PLY};

use super::super::constants::{
    LMR_IDX_BASE, LMR_SCORE_THRESHOLD, LMR_TABLE_MAX_DEPTH, LMR_TABLE_MAX_IDX, SCORE_INFINITE,
    SCORE_NEAR_MATE,
};
use super::{mate_score_for_ply, MoveContext, NodeContext, SimpleSearchContext, StagedMoveResult};

const MAX_QUIETS_TRACKED: usize = 64;
const LMR_GOOD_QUIET_SCORE: i32 = 1000;
const LMR_TABLE_START_INDEX: usize = 1;
const LMR_BASE: f64 = 0.77;
const LMR_LOG_DIVISOR: f64 = 2.36;
const LMR_MOVE_COUNT_DIVISOR: usize = 4;
const LMR_MIN_DEPTH: u32 = 2;
const QUIET_SEE_PRUNING_MAX_DEPTH: u32 = 3;
const QUIET_SEE_MIN_MOVE_COUNT: usize = 2;

fn null_window_alpha(alpha: i32) -> i32 {
    let window = -(i64::from(alpha) + 1);
    window.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn child_depth(depth: u32, extension: u32, only_legal_move: bool) -> u32 {
    if only_legal_move {
        depth.saturating_add(extension)
    } else {
        depth.saturating_sub(1).saturating_add(extension)
    }
}

impl SimpleSearchContext<'_> {
    /// Precomputed LMR table - slightly more aggressive than before.
    #[allow(clippy::cast_precision_loss)]
    fn lmr_table() -> &'static [[u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH] {
        static TABLE: OnceLock<[[u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH]> = OnceLock::new();
        TABLE.get_or_init(|| {
            let mut t = [[0u32; LMR_TABLE_MAX_IDX]; LMR_TABLE_MAX_DEPTH];
            for (depth, row) in t.iter_mut().enumerate().skip(LMR_TABLE_START_INDEX) {
                for (idx, cell) in row.iter_mut().enumerate().skip(LMR_TABLE_START_INDEX) {
                    // Stockfish-like LMR: base 0.77, divisor 2.36.
                    let val = (LMR_BASE
                        + (depth as f64).ln() * (idx as f64).ln() / LMR_LOG_DIVISOR)
                        .floor();
                    *cell = val.max(0.0) as u32;
                }
            }
            t
        })
    }

    /// Compute LMR reduction for a move.
    ///
    /// Uses `NodeContext` and `MoveContext` to reduce parameter count.
    fn compute_lmr_reduction(
        move_idx: usize,
        move_count: usize,
        depth: u32,
        node: &NodeContext,
        move_ctx: &MoveContext,
        tt_tactical: bool,
    ) -> u32 {
        let lmr_ok = move_idx > LMR_IDX_BASE + move_count / LMR_MOVE_COUNT_DIVISOR
            && move_ctx.move_score < LMR_SCORE_THRESHOLD
            && depth >= LMR_MIN_DEPTH
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

            // Reduce less when position is improving.
            if node.improving {
                reduction = reduction.saturating_sub(1);
            }

            if move_ctx.move_score > LMR_GOOD_QUIET_SCORE {
                reduction = reduction.saturating_sub(1);
            }

            reduction.min(depth.saturating_sub(1))
        } else {
            0
        }
    }

    fn update_correction_history_after_search(
        &mut self,
        ply: usize,
        depth: u32,
        in_check: bool,
        raised_alpha: bool,
        best_score: i32,
    ) {
        if !raised_alpha || ply >= MAX_PLY || in_check || best_score.abs() >= SCORE_NEAR_MATE {
            return;
        }

        let pawn_hash = self.board.pawn_hash();
        let raw_eval = self.static_eval[ply];
        let old_correction = self.state.tables.correction_history.get(pawn_hash);
        let static_eval_raw = raw_eval.saturating_sub(old_correction);
        self.state
            .tables
            .correction_history
            .update(pawn_hash, static_eval_raw, best_score, depth);
    }

    fn should_skip_quiet_by_see(
        &self,
        m: Move,
        is_quiet: bool,
        depth: u32,
        in_check: bool,
        move_count: usize,
    ) -> bool {
        is_quiet
            && depth <= QUIET_SEE_PRUNING_MAX_DEPTH
            && !in_check
            && move_count >= QUIET_SEE_MIN_MOVE_COUNT
            && !self.board.see_quiet_safe(m.from(), m.to())
    }

    fn search_child_move(
        &mut self,
        new_depth: u32,
        reduction: u32,
        alpha: i32,
        beta: i32,
        ply: usize,
        is_first: bool,
    ) -> i32 {
        if is_first {
            return -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);
        }

        let mut score = -self.alphabeta(
            new_depth.saturating_sub(reduction),
            null_window_alpha(alpha),
            -alpha,
            true,
            ply + 1,
            EMPTY_MOVE,
        );

        if reduction > 0 && score > alpha {
            score = -self.alphabeta(
                new_depth,
                null_window_alpha(alpha),
                -alpha,
                true,
                ply + 1,
                EMPTY_MOVE,
            );
        }

        if score > alpha && score < beta {
            score = -self.alphabeta(new_depth, -beta, -alpha, true, ply + 1, EMPTY_MOVE);
        }

        score
    }

    fn penalize_failed_quiets(
        &mut self,
        quiets_tried: &[Move; MAX_QUIETS_TRACKED],
        quiets_count: usize,
        best_move: Move,
        depth: u32,
    ) {
        for quiet_mv in quiets_tried.iter().take(quiets_count) {
            if *quiet_mv != best_move && *quiet_mv != EMPTY_MOVE {
                self.state.tables.history.penalize(quiet_mv, depth);
            }
        }
    }

    fn previous_move_at_ply(&self, ply: usize) -> Move {
        if ply > 0 && ply < MAX_PLY {
            self.previous_move[ply - 1]
        } else {
            EMPTY_MOVE
        }
    }

    fn initial_move_search_state(
        staged: Option<StagedMoveResult>,
        tt_move: Move,
        alpha: &mut i32,
    ) -> (i32, Move, bool, bool) {
        if let Some(result) = staged {
            if result.raised_alpha {
                *alpha = result.score;
            }
            (result.score, tt_move, result.raised_alpha, true)
        } else {
            (-SCORE_INFINITE, EMPTY_MOVE, false, false)
        }
    }

    fn no_moves_tried_score(in_check: bool, ply: usize) -> i32 {
        if in_check {
            mate_score_for_ply(-1, ply)
        } else {
            0
        }
    }

    /// Search the ordered move list and return the best score.
    pub(super) fn search_moves(
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
        let prev_move = self.previous_move_at_ply(ply);
        let mut scored_moves = self.order_moves(moves, node.tt_move, ply, prev_move);
        let move_count = scored_moves.len();
        let tt_tactical = node.tt_move.is_capture() || node.tt_move.is_promotion();

        let (mut best_score, mut best_move, mut raised_alpha, tt_move_searched) =
            Self::initial_move_search_state(staged, node.tt_move, &mut alpha);

        let mut moves_tried = usize::from(tt_move_searched);
        let mut quiets_tried: [Move; MAX_QUIETS_TRACKED] = [EMPTY_MOVE; MAX_QUIETS_TRACKED];
        let mut quiets_count = 0usize;

        let mut i = 0;
        while let Some(scored) = scored_moves.pick_best(i) {
            let m = scored.mv;
            let move_score = scored.score;
            i += 1;
            if self.should_stop() {
                break;
            }

            if m == node.excluded_move {
                continue;
            }

            if tt_move_searched && m == node.tt_move {
                continue;
            }

            let is_quiet = !m.is_capture() && !m.is_promotion();
            if self.should_skip_quiet_by_see(m, is_quiet, depth, in_check, move_count) {
                continue;
            }

            if is_quiet && quiets_count < MAX_QUIETS_TRACKED {
                quiets_tried[quiets_count] = m;
                quiets_count += 1;
            }

            let moving_piece = self.board.piece_at(m.from()).map(|(_, p)| p);
            if let Some(piece) = moving_piece {
                self.update_accumulator_for_move(ply, m, piece, self.board.side_to_move());
            }

            let info = self.board.make_move(m);
            if moving_piece == Some(Piece::King) {
                self.init_accumulator(ply + 1);
            }

            self.state.tables.tt.prefetch(self.board.hash);
            let gives_check = self.board.is_in_check(self.board.side_to_move());

            if ply < MAX_PLY {
                self.previous_move[ply] = m;
                self.previous_piece[ply] = moving_piece;
            }

            moves_tried += 1;

            let move_ctx = MoveContext::new(m, move_score, gives_check, moving_piece);

            if self.should_prune_quiet(&move_ctx, node, depth, moves_tried, alpha) {
                self.board.unmake_move(m, info);
                continue;
            }

            let reduction = Self::compute_lmr_reduction(
                i - 1,
                move_count,
                depth,
                node,
                &move_ctx,
                tt_tactical || gives_check || m.is_capture(),
            );
            let extension = Self::compute_extensions(&move_ctx, node);
            let new_depth = child_depth(depth, extension, move_count == 1);

            let score = self.search_child_move(new_depth, reduction, alpha, beta, ply, i == 1);

            self.board.unmake_move(m, info);

            if self.should_stop() {
                break;
            }

            if score > best_score {
                best_score = score;
                best_move = m;

                if score > alpha {
                    if score >= beta {
                        self.penalize_failed_quiets(&quiets_tried, quiets_count, m, depth);
                        self.handle_beta_cutoff(m, ply, depth, score, best_move);
                        return score;
                    }
                    alpha = score;
                    raised_alpha = true;
                }
            }
        }

        if moves_tried == 0 {
            return Self::no_moves_tried_score(in_check, ply);
        }

        self.store_tt(depth, best_score, raised_alpha, best_move);
        self.update_correction_history_after_search(ply, depth, in_check, raised_alpha, best_score);

        best_score
    }
}

#[cfg(test)]
mod tests {
    use super::{child_depth, null_window_alpha};

    #[test]
    fn null_window_alpha_saturates_extreme_input() {
        assert_eq!(null_window_alpha(i32::MAX), i32::MIN);
        assert_eq!(null_window_alpha(10), -11);
    }

    #[test]
    fn child_depth_saturates_depth_and_extension() {
        assert_eq!(child_depth(u32::MAX, 1, true), u32::MAX);
        assert_eq!(child_depth(u32::MAX, 1, false), u32::MAX);
        assert_eq!(child_depth(3, 1, false), 3);
    }
}
