use crate::board::nnue::network::feature_index;
use crate::board::nnue::{NnueAccumulator, NnueNetwork};
use crate::board::{Board, Color, Move, Piece, Square};
use crate::eval_math::{blended_eval, scaled_eval};

use super::SimpleSearchContext;

const EVAL_SCALE_DENOMINATOR: i32 = 100;
const STATIC_TRACE_MAX_PLY: usize = 8;
const STATIC_TRACE_NODE_INTERVAL_LOG2: u32 = 6;

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
    if moving_piece == Piece::King {
        return;
    }
    let acc = &mut stack[ply + 1];

    let feat = |piece: Piece, color: Color, sq: usize| -> (usize, usize) {
        let white_king = board.king_square_index(Color::White);
        let black_king = board.king_square_index(Color::Black);
        (
            feature_index(piece.index(), color.index(), sq, 0, white_king),
            feature_index(piece.index(), color.index(), sq, 1, black_king),
        )
    };

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

impl SimpleSearchContext<'_> {
    /// Evaluate position from side-to-move's perspective.
    /// Uses NNUE with incremental accumulator if available, otherwise HCE.
    #[inline]
    pub(super) fn evaluate(&self, ply: usize) -> i32 {
        if ply < self.acc_stack.len() {
            if let Some(ref nnue) = self.state.tables.nnue {
                return scaled_eval(
                    nnue.evaluate(&self.acc_stack[ply], self.board.white_to_move),
                    self.state.nnue_eval_scale,
                );
            }
        }
        self.board.evaluate_simple()
    }

    /// Evaluation for pruning and qsearch (main workhorse).
    /// Uses the configured NNUE/HCE static blend when NNUE is available.
    #[inline]
    pub(super) fn evaluate_simple(&self, ply: usize) -> i32 {
        if self.state.trace
            && ply <= STATIC_TRACE_MAX_PLY
            && self.nodes.trailing_zeros() >= STATIC_TRACE_NODE_INTERVAL_LOG2
        {
            println!("info string staticevalfen {}", self.board.to_fen());
        }
        if let Some(ref static_nnue) = self.state.tables.static_nnue {
            if ply < self.static_acc_stack.len() {
                return scaled_eval(
                    static_nnue.evaluate(&self.static_acc_stack[ply], self.board.white_to_move),
                    self.state.nnue_static_eval_scale,
                );
            }
        }
        if ply < self.acc_stack.len() {
            if let Some(ref nnue) = self.state.tables.nnue {
                let nnue_eval = scaled_eval(
                    nnue.evaluate(&self.acc_stack[ply], self.board.white_to_move),
                    self.state.nnue_eval_scale,
                );
                let blend = self.state.nnue_static_blend;
                if blend >= EVAL_SCALE_DENOMINATOR {
                    return nnue_eval;
                }
                let hce_eval = self.board.evaluate_simple();
                return blended_eval(nnue_eval, hce_eval, blend);
            }
        }
        self.board.evaluate_simple()
    }

    /// Initialize the accumulator at the given ply from the current board state.
    pub(super) fn init_accumulator(&mut self, ply: usize) {
        if ply >= self.acc_stack.len() {
            return;
        }
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
    pub(super) fn update_accumulator_for_move(
        &mut self,
        ply: usize,
        m: Move,
        moving_piece: Piece,
        moving_color: Color,
    ) {
        if let Some(ref nnue) = self.state.tables.nnue {
            update_accumulator_stack_for_move(
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
            update_accumulator_stack_for_move(
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
    pub(super) fn copy_accumulator_for_null_move(&mut self, ply: usize) {
        if ply + 1 < self.acc_stack.len() {
            self.acc_stack[ply + 1] = self.acc_stack[ply].clone();
        }
        if ply + 1 < self.static_acc_stack.len() {
            self.static_acc_stack[ply + 1] = self.static_acc_stack[ply].clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blended_eval, scaled_eval};

    #[test]
    fn scaled_eval_applies_percent_scale() {
        assert_eq!(scaled_eval(240, 50), 120);
        assert_eq!(scaled_eval(-240, 125), -300);
    }

    #[test]
    fn blended_eval_weights_nnue_and_hce() {
        assert_eq!(blended_eval(100, 300, 100), 100);
        assert_eq!(blended_eval(100, 300, 0), 300);
        assert_eq!(blended_eval(100, 300, 25), 250);
    }
}
