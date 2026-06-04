use crate::board::{Move, MoveList, Piece, ScoredMoveList, EMPTY_MOVE, MAX_PLY};
use crate::tt::BoundType;

use super::super::constants::{
    COUNTER_SCORE, KILLER1_SCORE, KILLER2_SCORE, KILLER3_SCORE, TT_MOVE_SCORE,
};
use super::SimpleSearchContext;

const COUNTERMOVE_HISTORY_DIVISOR: i32 = 2;

impl SimpleSearchContext<'_> {
    fn counter_move_after(&self, prev_move: Move) -> Move {
        if prev_move == EMPTY_MOVE {
            return EMPTY_MOVE;
        }

        self.state
            .tables
            .counter_moves
            .get(prev_move.from().index(), prev_move.to().index())
    }

    fn previous_piece_context(&self, ply: usize, prev_move: Move) -> (Option<Piece>, usize) {
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
        (prev_piece, prev_to)
    }

    fn quiet_history_score(&self, m: Move, prev_piece: Option<Piece>, prev_to: usize) -> i32 {
        let hist = self.state.tables.history_score(&m);
        let cont_hist = prev_piece.map_or(0, |piece| {
            self.state
                .tables
                .continuation_history
                .score(piece, prev_to, m)
        });
        let cm_hist = prev_piece.map_or(0, |opp_piece| {
            self.board.piece_at(m.from()).map_or(0, |(_, our_piece)| {
                self.state
                    .tables
                    .countermove_history
                    .score(opp_piece, prev_to, our_piece, m)
                    / COUNTERMOVE_HISTORY_DIVISOR
            })
        });
        hist + cont_hist + cm_hist
    }

    fn update_quiet_beta_cutoff_history(&mut self, m: Move, ply: usize, depth: u32) {
        self.state.tables.killer_moves.update(ply, m);

        if ply == 0 {
            return;
        }

        let prev = self.previous_move[ply - 1];
        if prev != EMPTY_MOVE {
            self.state
                .tables
                .counter_moves
                .set(prev.from().index(), prev.to().index(), m);
        }

        if let Some(prev_piece) = self.previous_piece[ply - 1] {
            let prev_to = prev.to().index();
            self.state
                .tables
                .continuation_history
                .update(prev_piece, prev_to, m, depth);

            if let Some((_, our_piece)) = self.board.piece_at(m.from()) {
                self.state
                    .tables
                    .countermove_history
                    .update(prev_piece, prev_to, our_piece, m, depth);
            }
        }
    }

    fn update_capture_beta_cutoff_history(&mut self, m: Move, depth: u32) {
        let Some((_, attacker)) = self.board.piece_at(m.from()) else {
            return;
        };
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

    /// Order moves for better pruning (TT move > killers > counter > captures > history + continuation)
    pub(super) fn order_moves(
        &mut self,
        moves: &MoveList,
        tt_move: Move,
        ply: usize,
        prev_move: Move,
    ) -> ScoredMoveList {
        let counter = self.counter_move_after(prev_move);
        let (prev_piece, prev_to) = self.previous_piece_context(ply, prev_move);

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
            } else if m.is_capture() {
                self.state.tables.mvv_lva_score(self.board, m)
            } else {
                self.quiet_history_score(*m, prev_piece, prev_to)
            };
            scored.push(*m, score);
        }
        scored
    }

    /// Handle beta cutoff: update killers, history, counter moves, continuation history, and TT.
    pub(super) fn handle_beta_cutoff(
        &mut self,
        m: Move,
        ply: usize,
        depth: u32,
        score: i32,
        best_move: Move,
    ) {
        if !m.is_capture() && ply < MAX_PLY {
            self.update_quiet_beta_cutoff_history(m, ply, depth);
        } else if m.is_capture() {
            self.update_capture_beta_cutoff_history(m, depth);
        }

        self.state.tables.update_history(&m, depth);

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
}
