use super::super::constants::{MATE_THRESHOLD, MAX_QSEARCH_DEPTH, SCORE_INFINITE};
use super::super::move_order::piece_value;
use super::SimpleSearchContext;
use crate::board::{ScoredMoveList, EMPTY_MOVE};

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
    /// Quiescence search for tactical stability with SEE and delta pruning
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, qdepth: i32) -> i32 {
        let stand_pat = self.evaluate_simple();

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
                return -MATE_THRESHOLD; // Checkmate
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
            let m = scored.mv;

            // Skip non-capture moves in quiescence (shouldn't happen but be safe)
            if !in_check && !m.is_capture() && !m.is_promotion() {
                continue;
            }

            // Delta pruning: if even winning the captured piece + margin won't raise alpha, skip
            // Use slightly larger margin at deep depths to be less aggressive
            if !in_check && m.is_capture() {
                if let Some((_, captured)) = self.board.piece_at(m.to()) {
                    let margin = if qdepth <= SEE_SHALLOW_DEPTH {
                        DELTA_MARGIN
                    } else {
                        DELTA_MARGIN + DELTA_MARGIN_DEEP
                    };
                    let delta = piece_value(captured) + margin;
                    if stand_pat + delta < alpha {
                        continue;
                    }
                }
            }

            // SEE pruning: depth-dependent thresholds
            // At shallow qsearch, prune all bad captures
            // At deeper qsearch, allow slightly bad captures to find tactics
            if !in_check {
                let see_score = self.board.see(m.from(), m.to());
                let see_threshold = if qdepth <= SEE_SHALLOW_DEPTH {
                    SEE_THRESHOLD_SHALLOW
                } else if qdepth <= SEE_MEDIUM_DEPTH {
                    SEE_THRESHOLD_MEDIUM
                } else {
                    SEE_THRESHOLD_DEEP
                };
                if see_score < see_threshold {
                    continue;
                }
            }

            self.nodes += 1;
            let info = self.board.make_move(m);
            // Prefetch TT for child position
            self.state.tables.tt.prefetch(self.board.hash);
            let score = -self.quiesce(-beta, -alpha, qdepth + 1);
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
