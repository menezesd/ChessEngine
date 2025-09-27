use crate::tt::{BoundType, TranspositionTable};
use crate::{color_to_zobrist_index, mvv_lva_score, square_to_zobrist_index};
use crate::{Board, Move};

#[derive(Clone)]
pub struct SearchHeuristics {
    pub(crate) killers: Vec<[Option<Move>; 2]>,
    pub(crate) history: [[[i32; 64]; 64]; 2],
}

impl SearchHeuristics {
    pub fn new(max_ply: usize) -> Self {
        SearchHeuristics {
            killers: vec![[None, None]; max_ply.max(1)],
            history: [[[0; 64]; 64]; 2],
        }
    }
    pub fn ensure_ply(&mut self, ply: usize) {
        if ply >= self.killers.len() {
            self.killers.resize(ply + 1, [None, None]);
        }
    }
}

// Search functions remain attached to Board in main.rs for now.
impl Board {
    pub(crate) fn negamax(
        &mut self,
        tt: &mut TranspositionTable,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        heur: &mut SearchHeuristics,
        ply: usize,
    ) -> i32 {
        // Mate distance pruning: tighten alpha/beta by remaining distance to mate
        let ply_i32 = ply as i32;
        alpha = alpha.max(-crate::MATE_SCORE + ply_i32);
        beta = beta.min(crate::MATE_SCORE - ply_i32);
        if alpha >= beta {
            return alpha;
        }

        let original_alpha = alpha;
        let current_hash = self.hash;

        if self.is_fifty_move_draw() || self.is_threefold_repetition() {
            return 0;
        }

        let mut hash_move: Option<Move> = None;
        if let Some(entry) = tt.probe(current_hash) {
            if entry.depth >= depth {
                match entry.bound_type {
                    BoundType::Exact => return entry.score,
                    BoundType::LowerBound => alpha = alpha.max(entry.score),
                    BoundType::UpperBound => beta = beta.min(entry.score),
                }
                if alpha >= beta {
                    return entry.score;
                }
            }
            hash_move = entry.best_move.clone();
        }

        if depth == 0 {
            return self.quiesce(tt, alpha, beta, heur, ply);
        }

        let in_check = self.is_in_check(self.current_color());

        if depth >= 3 && !in_check {
            let r = if depth >= 6 { 3 } else { 2 };
            let (prev_ep, prev_hash, prev_halfmove) = self.make_null_move();
            let score = -self.negamax(tt, depth - 1 - r, -beta, -beta + 1, heur, ply + 1);
            self.unmake_null_move(prev_ep, prev_hash, prev_halfmove);
            if score >= beta {
                return score;
            }
        }

        let mut legal_moves = self.generate_moves();
        heur.ensure_ply(ply);
        let color_idx = color_to_zobrist_index(self.current_color());
        let killers = heur.killers[ply].clone();
        let tt_move = hash_move.clone();
        legal_moves.sort_by_key(|m| {
            let mut score = 0i32;
            if let Some(ref hm) = tt_move {
                if m == hm {
                    score += 1_000_000;
                }
            }
            if m.captured_piece.is_some() || m.is_en_passant {
                let see_gain = self.see(m);
                score += 100_000 + mvv_lva_score(m, self) + see_gain;
            } else {
                if let Some(k0) = &killers[0] {
                    if m == k0 {
                        score += 90_000;
                    }
                }
                if let Some(k1) = &killers[1] {
                    if m == k1 {
                        score += 80_000;
                    }
                }
                let from_idx = square_to_zobrist_index(m.from) as usize;
                let to_idx = square_to_zobrist_index(m.to) as usize;
                score += heur.history[color_idx][from_idx][to_idx];
            }
            score
        });
        legal_moves.reverse();

        if legal_moves.is_empty() {
            let current_color = self.current_color();
            return if self.is_in_check(current_color) {
                // Checkmate: scale by ply for proper mate distance
                -(crate::MATE_SCORE - ply as i32)
            } else {
                0
            };
        }

        if let Some(hm) = &hash_move {
            if let Some(pos) = legal_moves.iter().position(|m| m == hm) {
                legal_moves.swap(0, pos);
            }
        }

        let mut best_score = -crate::MATE_SCORE * 2;
        let mut best_move_found: Option<Move> = None;

        // Precompute static eval for futility pruning at shallow depths
        let static_eval_opt = if depth <= 2 && !in_check {
            Some(self.evaluate())
        } else {
            None
        };

        for (i, m) in legal_moves.iter().enumerate() {
            let info = self.make_move(&m);
            let gives_check = self.is_in_check(self.current_color());
            let is_capture = m.captured_piece.is_some() || m.is_en_passant || m.promotion.is_some();
            let mut reduced_depth = depth - 1;
            if i > 3 && depth >= 3 && !is_capture && !gives_check {
                reduced_depth = depth - 2;
            }
            if gives_check {
                reduced_depth = reduced_depth.saturating_add(1);
            }

            // Futility pruning for quiet moves at shallow depth when far below alpha
            if let Some(static_eval) = static_eval_opt {
                if !gives_check && !is_capture && m.promotion.is_none() {
                    let fut_margin = 150 * (depth as i32);
                    if static_eval + fut_margin <= alpha {
                        self.unmake_move(&m, info);
                        continue;
                    }
                }
            }

            let score = if i == 0 {
                -self.negamax(tt, reduced_depth, -beta, -alpha, heur, ply + 1)
            } else {
                let mut score = -self.negamax(tt, reduced_depth, -alpha - 1, -alpha, heur, ply + 1);
                if score > alpha && score < beta {
                    score = -self.negamax(tt, reduced_depth, -beta, -alpha, heur, ply + 1);
                }
                score
            };
            self.unmake_move(&m, info);

            if score > best_score {
                best_score = score;
                best_move_found = Some(m.clone());
            }

            alpha = alpha.max(best_score);
            if alpha >= beta {
                if !is_capture && m.promotion.is_none() {
                    heur.ensure_ply(ply);
                    if heur.killers[ply][0].as_ref() != Some(m) {
                        heur.killers[ply][1] = heur.killers[ply][0].clone();
                        heur.killers[ply][0] = Some(m.clone());
                    }
                    let from_idx = square_to_zobrist_index(m.from) as usize;
                    let to_idx = square_to_zobrist_index(m.to) as usize;
                    heur.history[color_idx][from_idx][to_idx] += (depth as i32) * (depth as i32);
                }
                break;
            }
        }

        let bound_type = if best_score <= original_alpha {
            BoundType::UpperBound
        } else if best_score >= beta {
            BoundType::LowerBound
        } else {
            BoundType::Exact
        };
        tt.store(current_hash, depth, best_score, bound_type, best_move_found);
        best_score
    }

    pub(crate) fn quiesce(
        &mut self,
        tt: &mut TranspositionTable,
        mut alpha: i32,
        beta: i32,
        _heur: &mut SearchHeuristics,
        ply: usize,
    ) -> i32 {
        if self.is_fifty_move_draw() || self.is_threefold_repetition() {
            return 0;
        }
        let stand_pat_score = self.evaluate();
        if stand_pat_score >= beta {
            return beta;
        }
        alpha = alpha.max(stand_pat_score);

        let mut tactical_moves = self.generate_tactical_moves();
        tactical_moves.sort_by_key(|m| -mvv_lva_score(m, self));

        let delta_margin = 150;
        let mut best_score = stand_pat_score;
        for m in tactical_moves {
            if let Some(victim) = m.captured_piece {
                let gain = crate::piece_value(victim);
                if stand_pat_score + gain + delta_margin < alpha {
                    continue;
                }
                if self.see(&m) < 0 {
                    continue;
                }
            }
            let info = self.make_move(&m);
            let score = -self.quiesce(tt, -beta, -alpha, _heur, ply + 1);
            self.unmake_move(&m, info);
            best_score = best_score.max(score);
            alpha = alpha.max(best_score);
            if alpha >= beta {
                break;
            }
        }
        alpha
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color();
        let mut pseudo_tactical_moves = Vec::new();
        for r in 0..8 {
            for f in 0..8 {
                if let Some((c, piece)) = self.squares[r][f] {
                    if c == current_color {
                        let from = crate::Square(r, f);
                        match piece {
                            crate::Piece::Pawn => {
                                self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves);
                            }
                            _ => {
                                let piece_moves = self.generate_piece_moves(from, piece);
                                for m in piece_moves {
                                    if m.captured_piece.is_some() || m.is_en_passant {
                                        pseudo_tactical_moves.push(m);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut legal_tactical_moves = Vec::new();
        for m in pseudo_tactical_moves {
            if m.is_castling {
                continue;
            }
            let info = self.make_move(&m);
            if !self.is_in_check(current_color) {
                legal_tactical_moves.push(m.clone());
            }
            self.unmake_move(&m, info);
        }
        legal_tactical_moves
    }
}
