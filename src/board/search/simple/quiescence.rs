use super::super::constants::{MAX_QSEARCH_DEPTH, SCORE_INFINITE};
use super::super::move_order::piece_value;
use super::{mate_score_for_ply, SimpleSearchContext};
use crate::board::{Piece, ScoredMoveList, EMPTY_MOVE};

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
const TT_MOVE_ORDERING_SCORE: i32 = 1_000_000;
const MIN_MOVES_TO_SORT: usize = 4;

fn delta_margin(qdepth: i32) -> i32 {
    if qdepth <= SEE_SHALLOW_DEPTH {
        DELTA_MARGIN
    } else {
        DELTA_MARGIN.saturating_add(DELTA_MARGIN_DEEP)
    }
}

fn delta_pruning_upper_bound(stand_pat: i32, captured_value: i32, margin: i32) -> i32 {
    stand_pat
        .saturating_add(captured_value)
        .saturating_add(margin)
}

fn see_threshold(qdepth: i32) -> i32 {
    if qdepth <= SEE_SHALLOW_DEPTH {
        SEE_THRESHOLD_SHALLOW
    } else if qdepth <= SEE_MEDIUM_DEPTH {
        SEE_THRESHOLD_MEDIUM
    } else {
        SEE_THRESHOLD_DEEP
    }
}

impl SimpleSearchContext<'_> {
    /// Quiescence search for tactical stability with SEE and delta pruning.
    /// `ply` is the total ply from root (for correct mate score adjustment).
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, ply: usize, qdepth: i32) -> i32 {
        let stand_pat = self.evaluate_simple(ply);

        // Depth limit
        if qdepth >= MAX_QSEARCH_DEPTH {
            return stand_pat;
        }

        let in_check = self.board.is_in_check(self.board.side_to_move());
        let mut best_score = if in_check { -SCORE_INFINITE } else { stand_pat };

        // Generate moves: all moves if in check, captures only otherwise
        let moves = if in_check {
            let moves = self.board.generate_moves();
            if moves.is_empty() {
                return mate_score_for_ply(-1, ply); // Checkmate (ply-adjusted)
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
            self.board.generate_tactical_moves()
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
                TT_MOVE_ORDERING_SCORE
            } else {
                self.state.tables.mvv_lva_score(self.board, m)
            };
            sorted_moves.push(*m, score);
        }
        if sorted_moves.len() >= MIN_MOVES_TO_SORT {
            sorted_moves.sort_by_score_desc();
        }

        for scored in sorted_moves.iter() {
            let m = scored.mv;

            // Skip non-capture moves in quiescence (shouldn't happen but be safe)
            if !in_check && !m.is_capture() && !m.is_promotion() {
                continue;
            }

            // Delta pruning: if even winning the captured piece + margin won't raise alpha, skip
            // Use slightly larger margin at deep depths to be less aggressive
            if !in_check && m.is_capture() {
                let captured_value = if m.is_en_passant() {
                    piece_value(crate::board::Piece::Pawn)
                } else if let Some((_, captured)) = self.board.piece_at(m.to()) {
                    piece_value(captured)
                } else {
                    0
                };
                if delta_pruning_upper_bound(stand_pat, captured_value, delta_margin(qdepth))
                    < alpha
                {
                    continue;
                }
            }

            // SEE pruning: depth-dependent thresholds
            // At shallow qsearch, prune all bad captures
            // At deeper qsearch, allow slightly bad captures to find tactics
            if !in_check {
                let see_score = self.board.see(m.from(), m.to());
                if see_score < see_threshold(qdepth) {
                    continue;
                }
            }

            super::increment_node_count(&mut self.nodes);

            // Update NNUE accumulator before make_move
            let moving_piece = self.board.piece_at(m.from()).map(|(_, piece)| piece);
            if let Some(piece) = moving_piece {
                self.update_accumulator_for_move(ply, m, piece, self.board.side_to_move());
            }

            let info = self.board.make_move(m);
            if moving_piece == Some(Piece::King) {
                self.init_accumulator(ply + 1);
            }
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
    use super::{delta_margin, delta_pruning_upper_bound, see_threshold};

    #[test]
    fn delta_margin_increases_after_shallow_qdepth() {
        assert_eq!(delta_margin(2), 200);
        assert_eq!(delta_margin(3), 300);
    }

    #[test]
    fn see_threshold_relaxes_with_qdepth() {
        assert_eq!(see_threshold(2), 0);
        assert_eq!(see_threshold(5), -100);
        assert_eq!(see_threshold(6), -200);
    }

    #[test]
    fn delta_pruning_upper_bound_saturates_extreme_inputs() {
        assert_eq!(
            delta_pruning_upper_bound(i32::MAX, i32::MAX, i32::MAX),
            i32::MAX
        );
        assert_eq!(
            delta_pruning_upper_bound(i32::MIN, i32::MIN, i32::MIN),
            i32::MIN
        );
    }
}
