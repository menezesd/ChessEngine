use super::super::constants::{MATE_THRESHOLD, MAX_QSEARCH_DEPTH};
use super::SimpleSearchContext;
use crate::board::ScoredMoveList;

impl SimpleSearchContext<'_> {
    /// Quiescence search for tactical stability with SEE pruning
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, qdepth: i32) -> i32 {
        let stand_pat = self.evaluate_simple();

        // Depth limit
        if qdepth >= MAX_QSEARCH_DEPTH {
            return stand_pat;
        }

        let in_check = self.board.is_in_check(self.board.current_color());
        let mut best_score = if in_check { -30000 } else { stand_pat };

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

        // Sort captures by MVV-LVA (using stack-allocated list)
        let mut sorted_moves = ScoredMoveList::new();
        for m in &moves {
            let score = self.state.tables.mvv_lva_score(self.board, m);
            sorted_moves.push(*m, score);
        }
        if sorted_moves.len() > 3 {
            sorted_moves.sort_by_score_desc();
        }

        for scored in sorted_moves.iter() {
            let m = scored.mv;
            // SEE pruning: skip bad captures unless we are in check
            if !in_check {
                let see_score = self.board.see(m.from(), m.to());
                if see_score < 0 {
                    continue;
                }
            }

            self.nodes += 1;
            let info = self.board.make_move(m);
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
