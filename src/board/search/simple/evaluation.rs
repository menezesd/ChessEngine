//! Search evaluation and incremental NNUE accumulators.

use super::SimpleSearchContext;
use crate::board::nnue::network::feature_index;
use crate::board::nnue::{NnueAccumulator, NnueNetwork};
use crate::board::{Board, Color, Move, Piece, SearchState, MAX_PLY};

const STATIC_TRACE_MAX_PLY: usize = 8;
const STATIC_TRACE_NODE_INTERVAL_LOG2: u32 = 6;

pub(super) struct AccumulatorStack {
    entries: Box<[NnueAccumulator]>,
    output_fits_i32: bool,
}

impl AccumulatorStack {
    pub(super) fn new(slots: usize, network: Option<&NnueNetwork>) -> Self {
        Self {
            entries: vec![NnueAccumulator::default(); slots].into_boxed_slice(),
            output_fits_i32: slots > 0 && network.is_some_and(NnueNetwork::output_fits_i32),
        }
    }

    #[cfg(test)]
    pub(super) fn at(&self, ply: usize) -> &NnueAccumulator {
        &self.entries[ply]
    }

    fn refresh(&mut self, board: &Board, ply: usize, network: Option<&NnueNetwork>) {
        if let (Some(accumulator), Some(network)) = (self.entries.get_mut(ply), network) {
            *accumulator = board.nnue_accumulator(network);
        }
    }

    fn copy_to_child(&mut self, ply: usize) {
        if ply + 1 < self.entries.len() {
            self.entries[ply + 1] = self.entries[ply].clone();
        }
    }
}

pub(super) fn accumulator_stacks(state: &SearchState) -> (AccumulatorStack, AccumulatorStack) {
    let (main_slots, static_slots) = accumulator_slots(state);
    (
        AccumulatorStack::new(main_slots, state.active_main_nnue()),
        AccumulatorStack::new(static_slots, state.active_static_nnue()),
    )
}

fn accumulator_slots(state: &SearchState) -> (usize, usize) {
    let full_stack = MAX_PLY + 1;
    let main_slots = if state.active_main_nnue().is_none() {
        0
    } else if state.uses_main_evaluator_for_pruning() {
        full_stack
    } else {
        1
    };
    let static_slots = if state.active_static_nnue().is_some() {
        full_stack
    } else {
        0
    };
    (main_slots, static_slots)
}

impl SimpleSearchContext<'_> {
    /// Evaluate the position from the side-to-move perspective.
    #[inline]
    pub(super) fn evaluate_main(&self, ply: usize) -> i32 {
        self.state.evaluate_main(self.board, |network| {
            self.evaluate_accumulator(&self.acc_stack, ply, network)
        })
    }

    /// Evaluation for pruning and quiescence.
    #[inline]
    pub(super) fn evaluate_static(&self, ply: usize) -> i32 {
        if self.state.trace
            && ply <= STATIC_TRACE_MAX_PLY
            && self.nodes.trailing_zeros() >= STATIC_TRACE_NODE_INTERVAL_LOG2
        {
            println!("info string staticevalfen {}", self.board.to_fen());
        }

        self.state.evaluate_for_pruning(
            self.board,
            |network| self.evaluate_accumulator(&self.acc_stack, ply, network),
            |network| self.evaluate_accumulator(&self.static_acc_stack, ply, network),
        )
    }

    /// Initialize the accumulator at the given ply from the current board state.
    pub(super) fn init_accumulator(&mut self, ply: usize) {
        self.acc_stack
            .refresh(self.board, ply, self.state.active_main_nnue());
        self.static_acc_stack
            .refresh(self.board, ply, self.state.active_static_nnue());
    }

    /// Update the child accumulator for a move that passed pruning.
    /// The board is on the child's turn after making the move.
    pub(super) fn update_accumulator_for_move(
        &mut self,
        ply: usize,
        m: Move,
        moving_piece: Piece,
        captured: Option<(Color, Piece)>,
    ) {
        let moving_color = self.board.side_to_move().opponent();
        if let Some(ref nnue) = self.state.tables.nnue {
            Self::update_accumulator_stack_for_move(
                &mut self.acc_stack.entries,
                nnue,
                ply,
                m,
                moving_piece,
                moving_color,
                captured,
            );
        }
        if let Some(ref static_nnue) = self.state.tables.static_nnue {
            Self::update_accumulator_stack_for_move(
                &mut self.static_acc_stack.entries,
                static_nnue,
                ply,
                m,
                moving_piece,
                moving_color,
                captured,
            );
        }
    }

    /// Copy accumulator forward for null moves (no pieces change).
    #[inline]
    pub(super) fn copy_accumulator_for_null_move(&mut self, ply: usize) {
        self.acc_stack.copy_to_child(ply);
        self.static_acc_stack.copy_to_child(ply);
    }

    fn update_accumulator_stack_for_move(
        stack: &mut [NnueAccumulator],
        network: &NnueNetwork,
        ply: usize,
        m: Move,
        moving_piece: Piece,
        moving_color: Color,
        captured: Option<(Color, Piece)>,
    ) {
        if ply + 1 >= stack.len() {
            return;
        }

        let (parents, children) = stack.split_at_mut(ply + 1);
        let parent = &parents[ply];
        let acc = &mut children[0];

        let feat = |piece: Piece, color: Color, sq: usize| -> (usize, usize) {
            (
                feature_index(piece.index(), color.index(), sq, 0),
                feature_index(piece.index(), color.index(), sq, 1),
            )
        };

        if m.is_castling() {
            acc.clone_from(parent);
            let (wf, bf) = feat(Piece::King, moving_color, m.from().index());
            acc.sub_feature(wf, bf, network);
            let (wf, bf) = feat(Piece::King, moving_color, m.to().index());
            acc.add_feature(wf, bf, network);

            let (rook_from, rook_to) = m.castling_rook_squares();
            let (wf, bf) = feat(Piece::Rook, moving_color, rook_from.index());
            acc.sub_feature(wf, bf, network);
            let (wf, bf) = feat(Piece::Rook, moving_color, rook_to.index());
            acc.add_feature(wf, bf, network);
        } else {
            let captured =
                captured.map(|(color, piece)| feat(piece, color, m.capture_square().index()));

            let from = feat(moving_piece, moving_color, m.from().index());
            let to = feat(
                m.promotion().unwrap_or(moving_piece),
                moving_color,
                m.to().index(),
            );
            acc.update_from_parent(parent, from, to, captured, network);
        }
    }

    #[inline]
    fn evaluate_accumulator(
        &self,
        stack: &AccumulatorStack,
        ply: usize,
        network: &NnueNetwork,
    ) -> i32 {
        let acc = &stack.entries[ply];
        let evaluate = |acc| {
            network.evaluate_with_bounded_output(
                acc,
                self.board.white_to_move,
                stack.output_fits_i32,
            )
        };
        if network.supports_tactical_features() {
            let mut acc = acc.clone();
            self.board.add_nnue_dynamic_features(&mut acc, network);
            evaluate(&acc)
        } else {
            evaluate(acc)
        }
    }
}

#[cfg(all(test, feature = "embedded_nnue"))]
mod tests {
    #[test]
    fn accumulator_slots_follow_the_active_evaluator() {
        use super::{accumulator_slots, SearchState, MAX_PLY};
        use crate::board::nnue::NnueNetwork;
        use std::sync::Arc;

        let mut state = SearchState::new(1);
        assert_eq!(accumulator_slots(&state), (0, 0));
        let network = Arc::new(NnueNetwork::from_embedded());
        state.tables.nnue = Some(network.clone());
        assert_eq!(accumulator_slots(&state), (1, 0));
        state.static_eval_options.nnue_pure = true;
        assert_eq!(accumulator_slots(&state), (MAX_PLY + 1, 0));
        state.nnue_hce_blend = 0;
        assert_eq!(accumulator_slots(&state), (0, 0));
        state.nnue_hce_blend = 100;
        state.tables.static_nnue = Some(network);
        state.nnue_static_blend = 0;
        assert_eq!(accumulator_slots(&state), (1, 0));
        state.nnue_static_blend = 50;
        assert_eq!(accumulator_slots(&state), (1, MAX_PLY + 1));
        state.tables.nnue = None;
        assert_eq!(accumulator_slots(&state), (0, MAX_PLY + 1));
    }
}
