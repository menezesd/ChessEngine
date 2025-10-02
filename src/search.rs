use crate::tt::{BoundType, TranspositionTable};
use crate::{color_to_zobrist_index, square_to_zobrist_index};
use crate::{Board, Move};
use std::time::Instant;

// TT mate score conversion helpers: encode/decode mate distances by ply
#[inline]
fn score_to_tt(score: i32, ply: i32) -> i32 {
    let near_mate = crate::MATE_SCORE - 1000;
    if score >= near_mate {
        (score + ply).min(crate::MATE_SCORE)
    } else if score <= -near_mate {
        (score - ply).max(-crate::MATE_SCORE)
    } else {
        score
    }
}

#[inline]
fn score_from_tt(score: i32, ply: i32) -> i32 {
    let near_mate = crate::MATE_SCORE - 1000;
    if score >= near_mate {
        (score - ply).clamp(-crate::MATE_SCORE, crate::MATE_SCORE)
    } else if score <= -near_mate {
        (score + ply).clamp(-crate::MATE_SCORE, crate::MATE_SCORE)
    } else {
        score
    }
}

#[derive(Clone)]
pub struct SearchHeuristics {
    pub(crate) killers: Vec<[Option<Move>; 2]>,
    pub(crate) history: [[[i32; 64]; 64]; 2],
    pub(crate) butterfly: [[[i32; 64]; 64]; 2], // Total move attempts for normalization
    pub(crate) node_count: u64,
    pub(crate) countermove: [[[Option<Move>; 64]; 64]; 2],
    pub(crate) last_moves: Vec<Option<Move>>, // opponent's last move leading to this ply
    // Early-abort support
    pub(crate) deadline: Option<Instant>,
    pub(crate) abort: bool,
    pub(crate) nodes_between_checks: u64,
    pub(crate) next_check: u64,
    // SEE cache per ply: simple fixed-size ring storing last N (from,to) scores
    pub(crate) see_cache: Vec<[(u8, u8, i16); 8]>,
    pub(crate) see_cache_len: Vec<usize>,
}

impl SearchHeuristics {
    pub fn new(max_ply: usize) -> Self {
        SearchHeuristics {
            killers: vec![[None, None]; max_ply.max(1)],
            history: [[[0; 64]; 64]; 2],
            butterfly: [[[0; 64]; 64]; 2],
            node_count: 0,
            countermove: [[[None; 64]; 64]; 2],
            last_moves: vec![None; max_ply.max(1)],
            deadline: None,
            abort: false,
            nodes_between_checks: 2048,
            next_check: 2048,
            see_cache: vec!([(255, 255, 0); 8]; max_ply.max(1)),
            see_cache_len: vec![0; max_ply.max(1)],
        }
    }
    
    // Get relative history score (success rate)
    pub fn get_history_score(&self, color_idx: usize, from: usize, to: usize) -> i32 {
        let total = self.butterfly[color_idx][from][to];
        if total > 0 {
            (self.history[color_idx][from][to] * 10000) / total
        } else {
            0
        }
    }
    pub fn ensure_ply(&mut self, ply: usize) {
        if ply >= self.killers.len() {
            self.killers.resize(ply + 1, [None, None]);
        }
        if ply >= self.last_moves.len() {
            self.last_moves.resize(ply + 1, None);
        }
        if ply >= self.see_cache.len() {
            self.see_cache.resize(ply + 1, [(255, 255, 0); 8]);
            self.see_cache_len.resize(ply + 1, 0);
        }
    }

    #[inline]
    pub fn check_abort(&mut self) -> bool {
        if self.abort {
            return true;
        }
        if let Some(deadline) = self.deadline {
            if self.node_count >= self.next_check {
                if Instant::now() >= deadline {
                    self.abort = true;
                    return true;
                }
                // Schedule next check (increase spacing gradually)
                self.nodes_between_checks = (self.nodes_between_checks + 2048).min(1 << 20);
                self.next_check = self.node_count.saturating_add(self.nodes_between_checks);
            }
        }
        false
    }
}

impl Board {
    #[inline]
    fn see_cached(&self, m: &Move, heur: &mut SearchHeuristics, ply: usize) -> i32 {
        // Only cache for simple captures/EP without promotion component
        if !(m.captured_piece.is_some() || m.is_en_passant) { return 0; }
        heur.ensure_ply(ply);
        let key_from = crate::square_to_zobrist_index(m.from) as u8;
        let key_to = crate::square_to_zobrist_index(m.to) as u8;
        let cache = &mut heur.see_cache[ply];
        let len = &mut heur.see_cache_len[ply];
        for i in 0..*len {
            let (f, t, v) = cache[i];
            if f == key_from && t == key_to { return v as i32; }
        }
        // Miss: compute and insert (simple FIFO within 8 slots)
        let v = self.see(m) as i16;
        if *len < cache.len() { cache[*len] = (key_from, key_to, v); *len += 1; } else { cache[0] = (key_from, key_to, v); }
        v as i32
    }
    pub(crate) fn negamax(
        &mut self,
        tt: &mut TranspositionTable,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        heur: &mut SearchHeuristics,
        ply: usize,
    ) -> i32 {
    self.negamax_with_config(tt, depth, alpha, beta, heur, ply, &crate::bench::SearchConfig::tuned_optimal())
    }

    pub(crate) fn negamax_with_config(
        &mut self,
        tt: &mut TranspositionTable,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        heur: &mut SearchHeuristics,
        ply: usize,
        config: &crate::bench::SearchConfig,
    ) -> i32 {
        if heur.check_abort() { return alpha; }
        heur.node_count += 1;
        // Mate distance pruning
        let ply_i32 = ply as i32;
        alpha = alpha.max(-crate::MATE_SCORE + ply_i32);
        beta = beta.min(crate::MATE_SCORE - ply_i32);
        if alpha >= beta { return alpha; }

        let original_alpha = alpha;
        let current_hash = self.hash;

        // Draws
        if self.is_fifty_move_draw() || self.is_threefold_repetition() { return 0; }

        // Probe TT
        let mut hash_move: Option<Move> = None;
        if let Some(entry) = tt.probe(current_hash) {
            if entry.depth >= depth {
                let tt_sc = score_from_tt(entry.score, ply_i32);
                match entry.bound_type {
                    BoundType::Exact => return tt_sc,
                    BoundType::LowerBound => alpha = alpha.max(tt_sc),
                    BoundType::UpperBound => beta = beta.min(tt_sc),
                }
                if alpha >= beta { return tt_sc; }
            }
            hash_move = entry.best_move.clone();
        }

    if depth == 0 { return self.quiesce(tt, alpha, beta, heur, ply); }

        let in_check = self.is_in_check(self.current_color());

        // Adaptive null move pruning with crude zugzwang guard
        if depth >= config.null_move_depth_threshold && !in_check {
            let current_color = self.current_color();
            let mut has_pawn = false;
            let mut non_pawn_count = 0;
            for r in 0..8 { for f in 0..8 {
                if let Some((c, p)) = self.squares[r][f] {
                    if c == current_color {
                        if p == crate::Piece::Pawn { has_pawn = true; }
                        else if p != crate::Piece::King { non_pawn_count += 1; }
                    }
                }
            } }
            let zugzwang_risk = !has_pawn && non_pawn_count <= 2;
            if !zugzwang_risk {
                let mut r = config.null_move_r_base;
                if depth >= 8 { r += 2; } else if depth >= 5 { r += 1; }
                if non_pawn_count >= 5 && depth >= 6 { r += 1; }
                let (prev_ep, prev_hash, prev_halfmove) = self.make_null_move();
                // If making a null move leaves the side to move in check, it's an illegal null move; skip search
                let mut score = -crate::MATE_SCORE;
                if !self.is_in_check(self.current_color()) {
                    score = -self.negamax_with_config(tt, depth.saturating_sub(1 + r), -beta, -beta + 1, heur, ply + 1, config);
                }
                self.unmake_null_move(prev_ep, prev_hash, prev_halfmove);
                if score >= beta { return score; }
            }
        }

        // Razoring: if static eval far below alpha at shallow depth, try quiescence instead
        if depth <= 3 && !in_check {
            let static_eval = self.evaluate();
            let margin = 200 * (depth as i32);
            if static_eval + margin < alpha {
                let razor_score = self.quiesce(tt, alpha, beta, heur, ply);
                if razor_score <= alpha { return razor_score; }
            }
        }

        // Generate and order moves
    let mut legal_moves = self.generate_moves();
        heur.ensure_ply(ply);
        let color_idx = color_to_zobrist_index(self.current_color());
        let last_move_opt = if ply > 0 { heur.last_moves.get(ply - 1).cloned().flatten() } else { None };
        
        // Use enhanced move ordering
        crate::move_ordering::order_moves(&mut legal_moves, self, heur, hash_move.as_ref(), ply);

        if legal_moves.is_empty() {
            let current_color = self.current_color();
            return if self.is_in_check(current_color) { -(crate::MATE_SCORE - ply as i32) } else { 0 };
        }

    if let Some(hm) = &hash_move { if let Some(pos) = legal_moves.iter().position(|m| m == hm) { legal_moves.swap(0, pos); } }

        let mut best_score = -crate::MATE_SCORE * 2;
        let mut best_move_found: Option<Move> = None;

        // Static eval for futility
        let static_eval_opt = if depth <= config.futility_depth_threshold && !in_check { Some(self.evaluate()) } else { None };

        for (i, m) in legal_moves.iter().enumerate() {
            if heur.check_abort() { break; }
            let is_capture = m.captured_piece.is_some() || m.is_en_passant || m.promotion.is_some();
            let from_idx = square_to_zobrist_index(m.from) as usize;
            let to_idx = square_to_zobrist_index(m.to) as usize;
            let hist_val = heur.get_history_score(color_idx, from_idx, to_idx);
            
            // Update butterfly table for all moves
            heur.butterfly[color_idx][from_idx][to_idx] += 1;

            // SEE-based pruning for bad captures
            if is_capture && depth <= 3 && self.see_cached(&m, heur, ply) < config.see_capture_threshold { continue; }

            // Mild late move pruning
            if !is_capture && m.promotion.is_none() && !in_check {
                if depth <= config.lmp_depth_threshold && i > config.lmp_move_threshold {
                    if depth <= 2 || (depth == 3 && hist_val < 1000) { continue; }
                }
            }

            heur.ensure_ply(ply);
            heur.last_moves[ply] = Some(*m);
            let info = self.make_move(&m);
            let gives_check = self.is_in_check(self.current_color());

            // LMR
            let mut reduced_depth = depth - 1;
            if depth >= config.lmr_depth_threshold && i >= config.lmr_move_threshold && !is_capture && !gives_check && m.promotion.is_none() {
                let mut r = config.lmr_base_reduction;
                if depth >= 5 && i > 6 && hist_val < 2000 { r += 1; }
                reduced_depth = reduced_depth.saturating_sub(r);
            }
            if gives_check { reduced_depth = reduced_depth.saturating_add(1); }

            // Futility pruning
            if let Some(static_eval) = static_eval_opt {
                if !gives_check && !is_capture && m.promotion.is_none() && depth <= config.futility_depth_threshold {
                    let fut_margin = config.futility_margin_base * (depth as i32);
                    if static_eval + fut_margin <= alpha { self.unmake_move(&m, info); continue; }
                }
            }

            // Reverse futility pruning: when we're far above beta at shallow depth, cut
            if depth <= 2 && !gives_check && !is_capture && m.promotion.is_none() {
                let static_eval = static_eval_opt.unwrap_or_else(|| self.evaluate());
                let margin = 120 * (depth as i32);
                if static_eval - margin >= beta { self.unmake_move(&m, info); break; }
            }

            let score = if i == 0 {
                -self.negamax_with_config(tt, reduced_depth, -beta, -alpha, heur, ply + 1, config)
            } else {
                let mut score = -self.negamax_with_config(tt, reduced_depth, -alpha - 1, -alpha, heur, ply + 1, config);
                if score > alpha && score < beta { score = -self.negamax_with_config(tt, reduced_depth, -beta, -alpha, heur, ply + 1, config); }
                score
            };
            self.unmake_move(&m, info);

            if score > best_score { best_score = score; best_move_found = Some(m.clone()); }

            if !is_capture && m.promotion.is_none() && best_score > original_alpha {
                let bonus = (depth as i32) * (depth as i32);
                heur.history[color_idx][from_idx][to_idx] += bonus;
            }

            alpha = alpha.max(best_score);
            if alpha >= beta {
                if !is_capture && m.promotion.is_none() {
                    heur.ensure_ply(ply);
                    // Update killers
                    if heur.killers[ply][0].as_ref() != Some(m) { 
                        heur.killers[ply][1] = heur.killers[ply][0].clone(); 
                        heur.killers[ply][0] = Some(m.clone()); 
                    }
                    // Update history with bonus
                    let bonus = (depth as i32) * (depth as i32);
                    heur.history[color_idx][from_idx][to_idx] += bonus;
                    // Update countermove
                    if let Some(lm) = last_move_opt { 
                        let l_from = square_to_zobrist_index(lm.from) as usize; 
                        let l_to = square_to_zobrist_index(lm.to) as usize; 
                        heur.countermove[color_idx][l_from][l_to] = Some(*m); 
                    }
                }
                break;
            }
        }

        let bound_type = if best_score <= original_alpha { BoundType::UpperBound } else if best_score >= beta { BoundType::LowerBound } else { BoundType::Exact };
        tt.store(current_hash, depth, score_to_tt(best_score, ply_i32), bound_type, best_move_found.clone());
        best_score
    }

    pub(crate) fn extract_pv(&self, tt: &TranspositionTable, max_len: usize) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut clone = self.clone();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..max_len {
            if seen.contains(&clone.hash) { break; }
            seen.insert(clone.hash);
            let entry = match tt.probe(clone.hash) { Some(e) => e, None => break };
            let mv = match &entry.best_move { Some(m) => *m, None => break };
            let legal = clone.generate_moves().into_iter().any(|lm| lm == mv);
            if !legal { break; }
            let info = clone.make_move(&mv);
            pv.push(mv);
            let _ = info;
        }
        pv
    }

    pub(crate) fn quiesce(
        &mut self,
        _tt: &mut TranspositionTable,
        mut alpha: i32,
        beta: i32,
        heur: &mut SearchHeuristics,
        _ply: usize,
    ) -> i32 {
        if heur.check_abort() { return alpha; }
        heur.node_count += 1;
        if self.is_fifty_move_draw() || self.is_threefold_repetition() { return 0; }
        let stand_pat = self.evaluate();
        if stand_pat >= beta { return beta; }
        alpha = alpha.max(stand_pat);

        let mut tactical_moves = self.generate_tactical_moves();
        crate::move_ordering::order_tactical_moves(&mut tactical_moves, self);

        let delta_margin = 150;
        let mut best = stand_pat;
        for m in tactical_moves {
            if heur.check_abort() { break; }
            if let Some(victim) = m.captured_piece {
                let gain = crate::piece_value(victim);
                if stand_pat + gain + delta_margin < alpha { continue; }
                if self.see_cached(&m, heur, _ply) < 0 { continue; }
            }
            let info = self.make_move(&m);
            let score = -self.quiesce(_tt, -beta, -alpha, heur, _ply + 1);
            self.unmake_move(&m, info);
            best = best.max(score);
            alpha = alpha.max(best);
            if alpha >= beta { break; }
        }
        alpha
    }

    pub(crate) fn generate_tactical_moves(&mut self) -> Vec<Move> {
        let current_color = self.current_color();
        let mut pseudo_tactical_moves = Vec::new();
        for r in 0..8 { for f in 0..8 { if let Some((c, piece)) = self.squares[r][f] { if c == current_color { let from = crate::Square(r, f); match piece { crate::Piece::Pawn => { self.generate_pawn_tactical_moves(from, &mut pseudo_tactical_moves); }, _ => { let piece_moves = self.generate_piece_moves(from, piece); for m in piece_moves { if m.captured_piece.is_some() || m.is_en_passant { pseudo_tactical_moves.push(m); } } } } } } } }
        let mut legal_tactical_moves = Vec::new();
        for m in pseudo_tactical_moves { if m.is_castling { continue; } let info = self.make_move(&m); if !self.is_in_check(current_color) { legal_tactical_moves.push(m.clone()); } self.unmake_move(&m, info); }
        legal_tactical_moves
    }
}
