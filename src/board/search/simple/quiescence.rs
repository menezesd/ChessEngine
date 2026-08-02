use super::super::constants::{MAX_QSEARCH_DEPTH, SCORE_INFINITE};
use super::super::move_order::piece_value;
use super::super::MATE_SCORE;
use super::SimpleSearchContext;
use crate::board::{Move, ScoredMoveList, EMPTY_MOVE};

/// Delta pruning safety margin (centipawns)
const DELTA_MARGIN: i32 = 200;

/// Additional delta margin at deeper qsearch depths
const DELTA_MARGIN_DEEP: i32 = 100;

/// SEE threshold at shallow qsearch (prune all losing captures)
const SEE_THRESHOLD_SHALLOW: i32 = 0;

/// SEE threshold at medium qsearch (allow slightly bad captures)
const SEE_THRESHOLD_MEDIUM: i32 = -100;

/// SEE threshold at deep qsearch (allow more speculative captures)
const SEE_THRESHOLD_DEEP: i32 = -200;

/// Depth thresholds for SEE pruning
const SEE_SHALLOW_DEPTH: i32 = 2;
const SEE_MEDIUM_DEPTH: i32 = 5;

impl SimpleSearchContext<'_> {
    fn should_prune_quiescence_capture(
        &self,
        m: Move,
        stand_pat: i32,
        alpha: i32,
        qdepth: i32,
    ) -> bool {
        // SEE and delta pruning value only the captured piece. A promotion
        // also changes the moving pawn into a stronger piece, so treating a
        // capture-promotion as an ordinary capture can incorrectly prune a
        // decisive promotion before it is searched.
        if !m.is_capture() || m.is_promotion() {
            return false;
        }

        let captured_value = if m.is_en_passant() {
            piece_value(crate::board::Piece::Pawn)
        } else if let Some((_, captured)) = self.board.piece_at(m.to()) {
            piece_value(captured)
        } else {
            0
        };
        let margin = if qdepth <= SEE_SHALLOW_DEPTH {
            DELTA_MARGIN
        } else {
            DELTA_MARGIN + DELTA_MARGIN_DEEP
        };
        if stand_pat + captured_value + margin < alpha {
            return true;
        }

        let see_threshold = if qdepth <= SEE_SHALLOW_DEPTH {
            SEE_THRESHOLD_SHALLOW
        } else if qdepth <= SEE_MEDIUM_DEPTH {
            SEE_THRESHOLD_MEDIUM
        } else {
            SEE_THRESHOLD_DEEP
        };
        self.board.see(m.from(), m.to()) < see_threshold
    }

    /// Quiescence search for tactical stability with SEE and delta pruning.
    /// `ply` is the total ply from root (for correct mate score adjustment).
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, ply: usize, qdepth: i32) -> i32 {
        if self.should_stop() {
            return 0;
        }

        // Quiescence is also entered directly at alpha-beta leaves, so it
        // must honor draw rules independently of the full-width search.
        // In particular, a TT entry cannot safely stand in for this check:
        // repetition history and the halfmove clock are not in its key.
        if self.board.is_theoretical_draw() {
            return 0;
        }

        if let Some(score) = self.two_knights_vs_bare_king_score(ply) {
            return score;
        }

        // Alpha-beta can arrive here after a long extension sequence.  Do
        // not let quiescence recurse beyond the fixed search-state stacks.
        if ply >= crate::board::MAX_PLY {
            return self.evaluate_simple(ply);
        }

        let stand_pat = self.evaluate_simple(ply);
        let in_check = self.board.is_in_check(self.board.side_to_move());

        // At the depth limit we still need to recognize checkmate. A stand-pat
        // score is invalid when the side to move is in check.
        if qdepth >= MAX_QSEARCH_DEPTH {
            if !self.board.has_legal_move() {
                return if in_check {
                    -MATE_SCORE + ply as i32
                } else {
                    0
                };
            }
            return stand_pat;
        }

        let mut best_score = if in_check { -SCORE_INFINITE } else { stand_pat };

        // Generate moves: all moves if in check, captures only otherwise
        let moves = if in_check {
            let moves = self.board.generate_moves();
            if moves.is_empty() {
                return -MATE_SCORE + ply as i32; // Checkmate (ply-adjusted)
            }
            moves
        } else {
            // Stand pat
            if stand_pat >= beta {
                return stand_pat;
            }
            if alpha < stand_pat {
                alpha = stand_pat;
            }
            let tactical_moves = self.board.generate_tactical_moves();
            if tactical_moves.is_empty() && !self.board.has_legal_move() {
                return 0;
            }
            tactical_moves
        };

        // Probe TT for move ordering only (not cutoff - depth semantics differ)
        let tt_move = self
            .state
            .tables
            .tt
            .probe(self.board.hash)
            .and_then(|e| e.best_move())
            .unwrap_or(EMPTY_MOVE);

        // Sort captures by MVV-LVA, with TT move first (using stack-allocated list)
        let mut sorted_moves = ScoredMoveList::new();
        for m in &moves {
            let score = if *m == tt_move {
                1_000_000 // TT move first
            } else {
                self.state.tables.mvv_lva_score(self.board, m)
            };
            sorted_moves.push(*m, score);
        }
        if sorted_moves.len() > 3 {
            sorted_moves.sort_by_score_desc();
        }

        for scored in sorted_moves.iter() {
            if self.should_stop() {
                // `best_score` is negative infinity while in check. Returning
                // it on interruption can poison the parent search with a
                // fictitious mate score, so use the neutral stop score used
                // by alpha-beta instead.
                return 0;
            }

            let m = scored.mv;

            // Skip non-capture moves in quiescence (shouldn't happen but be safe)
            if !in_check && !m.is_capture() && !m.is_promotion() {
                continue;
            }

            if !in_check && self.should_prune_quiescence_capture(m, stand_pat, alpha, qdepth) {
                continue;
            }

            if !self.try_visit_node() {
                return 0;
            }

            // Update NNUE accumulator before make_move
            if let Some((_, piece)) = self.board.piece_at(m.from()) {
                self.update_accumulator_for_move(ply, m, piece, self.board.side_to_move());
            }

            let info = self.board.make_move(m);
            // Prefetch TT for child position
            self.state.tables.tt.prefetch(self.board.hash);
            let score = -self.quiesce(-beta, -alpha, ply + 1, qdepth + 1);
            self.board.unmake_move(m, info);

            if score >= beta {
                return score;
            }
            if score > alpha {
                alpha = score;
            }
            if score > best_score {
                best_score = score;
            }
        }

        best_score
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    use crate::board::nnue::NnueAccumulator;
    use crate::board::{Board, SearchState, EMPTY_MOVE, MAX_PLY};

    use super::{SimpleSearchContext, MATE_SCORE, MAX_QSEARCH_DEPTH, SCORE_INFINITE};

    #[test]
    fn quiescence_recognizes_checkmate_at_depth_limit() {
        let mut board = Board::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            hard_time_limit_ms: 0,
            clock: None,
            node_limit: 0,
            nodes: 0,
            futility_margin: 0,
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
            ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 0, MAX_QSEARCH_DEPTH),
            -MATE_SCORE
        );
    }

    #[test]
    fn quiescence_returns_draw_score_for_fifty_move_position() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/3Q4/K7 w - - 100 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            hard_time_limit_ms: 0,
            clock: None,
            node_limit: 0,
            nodes: 0,
            futility_margin: 0,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };

        assert_eq!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 0, 0), 0);
    }

    #[test]
    fn quiescence_returns_draw_score_for_stalemate() {
        let mut board = Board::from_fen("k7/8/1QK5/8/8/8/8/8 b - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            hard_time_limit_ms: 0,
            clock: None,
            node_limit: 0,
            nodes: 0,
            futility_margin: 0,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };

        assert_eq!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 0, 0), 0);
    }

    #[test]
    fn quiescence_never_delta_prunes_capture_promotions() {
        // gxh8=Q+ wins a rook and promotes. With a deliberately low
        // stand-pat score, ordinary capture delta pruning would discard it
        // if it counted only the rook on h8.
        let mut board = Board::from_fen("7r/6Pk/8/8/8/8/8/K7 w - - 0 1");
        let promotion = board
            .generate_tactical_moves()
            .into_iter()
            .find(|mv| mv.is_capture() && mv.is_promotion())
            .expect("capture promotion");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let ctx = SimpleSearchContext {
            board: &mut board,
            state: &mut state,
            stop: &stop,
            start_time: Instant::now(),
            time_limit_ms: 0,
            hard_time_limit_ms: 0,
            clock: None,
            node_limit: 0,
            nodes: 0,
            futility_margin: 0,
            initial_depth: 1,
            static_eval: [0; MAX_PLY],
            previous_move: [EMPTY_MOVE; MAX_PLY],
            previous_piece: [None; MAX_PLY],
            info_callback: None,
            root_moves: Vec::new(),
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
            static_acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 16].into_boxed_slice(),
        };

        assert!(!ctx.should_prune_quiescence_capture(promotion, -1_000, 0, 0));
    }
}
