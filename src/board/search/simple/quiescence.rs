use super::super::constants::{MAX_QSEARCH_DEPTH, QSEARCH_CHECK_EXTENSION, SCORE_INFINITE};
use super::super::move_order::piece_value;
use super::super::MATE_SCORE;
use super::SimpleSearchContext;
use crate::board::draw::FIFTY_MOVE_RULE_HALFMOVES;
use crate::board::{Move, MoveList, Piece, ScoredMoveList, EMPTY_MOVE};

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
        attacker: Piece,
        stand_pat: i32,
        alpha: i32,
        qdepth: i32,
    ) -> bool {
        // Delta pruning counts only the victim, and coordinate-based SEE
        // assumes a queen promotion. Search every promotion directly.
        if !m.is_capture() || m.is_promotion() {
            return false;
        }

        let victim = if m.is_en_passant() {
            Piece::Pawn
        } else if let Some((_, victim)) = self.board.piece_at(m.to()) {
            victim
        } else {
            return false;
        };
        let margin = if qdepth <= SEE_SHALLOW_DEPTH {
            DELTA_MARGIN
        } else {
            DELTA_MARGIN + DELTA_MARGIN_DEEP
        };
        if stand_pat + piece_value(victim) + margin < alpha {
            return true;
        }

        let see_threshold = if qdepth <= SEE_SHALLOW_DEPTH {
            SEE_THRESHOLD_SHALLOW
        } else if qdepth <= SEE_MEDIUM_DEPTH {
            SEE_THRESHOLD_MEDIUM
        } else {
            SEE_THRESHOLD_DEEP
        };
        let exchange = if m.is_en_passant() {
            self.board.see(m.from(), m.to())
        } else {
            self.board
                .see_with_pieces(m.from(), m.to(), attacker, victim)
        };
        exchange < see_threshold
    }

    fn order_quiescence_moves(&self, moves: &mut MoveList) {
        if moves.len() <= 3 {
            return;
        }

        let tt_move = self
            .state
            .tables
            .tt
            .probe(self.board.hash)
            .and_then(|e| e.best_move())
            .unwrap_or(EMPTY_MOVE);
        let mut sorted_moves = ScoredMoveList::new();
        for m in moves.iter() {
            let score = if *m == tt_move {
                1_000_000 // TT move first
            } else {
                self.state.tables.mvv_lva_score(self.board, m)
            };
            sorted_moves.push(*m, score);
        }
        sorted_moves.sort_by_score_desc();
        for (mv, sorted) in moves.iter_mut().zip(sorted_moves.iter()) {
            *mv = sorted.mv;
        }
    }

    /// Quiescence search for tactical stability with SEE and delta pruning.
    /// `ply` is the total ply from root (for correct mate score adjustment).
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, ply: usize, qdepth: i32) -> i32 {
        if !self.try_visit_node(ply) {
            return 0;
        }

        // Quiescence is also entered directly at alpha-beta leaves, so it
        // must honor draw rules independently of the full-width search.
        // In particular, a TT entry cannot safely stand in for this check:
        // repetition history and the halfmove clock are not in its key.
        if self.is_search_draw(ply) {
            return 0;
        }

        if let Some(score) = self.exact_endgame_score(ply) {
            return score;
        }

        let in_check = self.board.is_in_check(self.board.side_to_move());
        let claim_next_move = self.board.halfmove_clock() == FIFTY_MOVE_RULE_HALFMOVES - 1;
        let depth_limit = MAX_QSEARCH_DEPTH + if in_check { QSEARCH_CHECK_EXTENSION } else { 0 };
        // Keep searching evasions briefly past the capture horizon. The
        // full-width search can already have reached the fixed stack limit.
        if ply >= crate::board::MAX_PLY || qdepth >= depth_limit {
            return self.score_at_search_limit(ply);
        }

        // At the fifty-move boundary, quiet moves can force a draw,
        // deliver mate before the claim, or reset the clock with a pawn.
        let search_all_moves = in_check || claim_next_move;
        let stand_pat = if search_all_moves {
            -SCORE_INFINITE
        } else {
            self.evaluate_static(ply)
        };
        let mut best_score = stand_pat;

        if !search_all_moves && stand_pat >= beta {
            return if self.board.has_legal_move_out_of_check() {
                stand_pat
            } else {
                0
            };
        }
        alpha = alpha.max(stand_pat);

        let mut moves = MoveList::new();
        if in_check {
            self.board.generate_evasions_into(&mut moves);
        } else if claim_next_move {
            self.board.generate_moves_into(&mut moves);
        } else {
            self.board.generate_tactical_moves_into(&mut moves);
        }
        if moves.is_empty() {
            return if in_check {
                -MATE_SCORE + ply as i32
            } else if !search_all_moves && self.board.has_legal_move_out_of_check() {
                stand_pat
            } else {
                0
            };
        }

        self.order_quiescence_moves(&mut moves);
        for &m in &moves {
            if self.should_stop() {
                // `best_score` is negative infinity while in check. Returning
                // it on interruption can poison the parent search with a
                // fictitious mate score, so use the neutral stop score used
                // by alpha-beta instead.
                return 0;
            }

            // Skip non-capture moves in quiescence (shouldn't happen but be safe)
            if !search_all_moves && !m.is_tactical() {
                continue;
            }

            let Some((_, piece)) = self.board.piece_at(m.from()) else {
                continue;
            };
            // A checking sacrifice or immediate draw can succeed even when
            // the material estimate predicts that the capture loses.
            if !search_all_moves
                && self.should_prune_quiescence_capture(m, piece, stand_pat, alpha, qdepth)
            {
                let saves_draw = alpha < 0
                    && !m.is_en_passant()
                    && self.board.is_insufficient_material_without(m.to());
                if !saves_draw && !self.board.capture_gives_check(m, piece) {
                    continue;
                }
            }

            let info = self.board.make_search_move(m);
            // Prefetch TT for child position
            self.state.tables.tt.prefetch(self.board.hash);
            self.update_accumulator_for_move(ply, m, piece, info.captured_piece_info);
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

    use crate::board::{Board, SearchState};

    use super::{MATE_SCORE, MAX_QSEARCH_DEPTH, SCORE_INFINITE};

    #[test]
    fn quiescence_searches_quiet_moves_at_the_fifty_move_boundary() {
        for (fen, expected) in [
            ("7k/8/8/8/8/8/2B5/K5R1 w - - 99 1", 0),
            ("7k/8/8/8/8/8/1B6/K5R1 b - - 99 1", 0),
            ("7k/5K2/8/8/8/8/B7/R7 w - - 99 1", MATE_SCORE - 2),
        ] {
            let mut board = Board::from_fen(fen);
            let mut state = SearchState::new(1);
            let stop = AtomicBool::new(false);
            let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
            assert_eq!(
                ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 1, 0),
                expected,
                "{fen}"
            );
        }
    }

    #[test]
    fn quiescence_keeps_checkmating_capture_despite_delta_pruning() {
        for (alpha, beta) in [(-SCORE_INFINITE, SCORE_INFINITE), (3000, 3001)] {
            let mut board = Board::from_fen("7k/5K1p/6p1/4N3/8/8/8/7R w - - 0 1");
            let mut state = SearchState::new(1);
            let stop = AtomicBool::new(false);
            let mut ctx = super::super::test_context(&mut board, &mut state, &stop);

            let score = ctx.quiesce(alpha, beta, 1, 0);

            assert_eq!(
                score,
                MATE_SCORE - 2,
                "pruned Nxg6# with window {alpha}..{beta}"
            );
        }
    }

    #[test]
    fn en_passant_capture_can_discover_check_through_both_removed_pawns() {
        let mut board = Board::from_fen("8/8/8/R4pPk/8/8/8/K7 w - f6 0 1");
        let mv = board.parse_move("g5f6").unwrap();

        assert!(board.capture_gives_check(mv, crate::board::Piece::Pawn));
    }

    #[test]
    fn quiescence_searches_capture_that_secures_insufficient_material_draw() {
        let mut board = Board::from_fen("7k/8/8/8/5b2/6b1/pK6/8 w - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        ctx.init_accumulator(1);

        assert_eq!(ctx.quiesce(-1, 0, 1, 0), 0);
    }

    #[test]
    fn quiescence_recognizes_checkmate_at_depth_limit() {
        let mut board = Board::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);

        assert_eq!(
            ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 0, MAX_QSEARCH_DEPTH),
            -MATE_SCORE
        );
    }

    #[test]
    fn quiescence_can_capture_a_checking_piece_at_the_capture_horizon() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/4r3/4K3 w - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        assert_eq!(
            ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 1, MAX_QSEARCH_DEPTH),
            0
        );
    }

    #[test]
    fn quiescence_returns_draw_score_for_fifty_move_position() {
        let mut board = Board::from_fen("7k/8/8/8/8/8/3Q4/K7 w - - 100 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);

        assert_eq!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 0, 0), 0);
    }

    #[test]
    fn quiescence_respects_search_line_twofold_repetition() {
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
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);
        ctx.line.hashes[0] = repeated_hash;

        assert_eq!(ctx.quiesce(-SCORE_INFINITE, SCORE_INFINITE, 4, 0), 0);
    }

    #[test]
    fn quiescence_returns_draw_score_for_stalemate() {
        let mut board = Board::from_fen("k7/8/1QK5/8/8/8/8/8 b - - 0 1");
        let mut state = SearchState::new(1);
        let stop = AtomicBool::new(false);
        let mut ctx = super::super::test_context(&mut board, &mut state, &stop);

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
        let ctx = super::super::test_context(&mut board, &mut state, &stop);

        assert!(!ctx.should_prune_quiescence_capture(
            promotion,
            crate::board::Piece::Pawn,
            -1_000,
            0,
            0
        ));
    }
}
