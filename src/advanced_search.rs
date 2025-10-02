use crate::{Board, Move, TranspositionTable, MATE_SCORE};
use crate::search::SearchHeuristics;
use crate::tt::BoundType;
use crate::lmr::{LmrTable, lmp_threshold};
use crate::singular::{SingularExtension, InternalIterativeReduction, Probcut};
use crate::board::{color_to_zobrist_index, square_to_zobrist_index};

/// Enhanced search implementing advanced techniques
impl Board {
    /// Enhanced negamax search with advanced pruning techniques
    pub fn negamax_enhanced(
        &mut self,
        tt: &mut TranspositionTable,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        heur: &mut SearchHeuristics,
        ply: usize,
        lmr_table: &LmrTable,
        was_null_move: bool,
        excluded_move: Option<Move>
    ) -> i32 {
        if heur.check_abort() { return alpha; }
        heur.node_count += 1;
        
        let ply_i32 = ply as i32;
        let is_root = ply == 0;
        let is_pv = beta > alpha + 1;
        
        // Mate distance pruning
        alpha = alpha.max(-MATE_SCORE + ply_i32);
        let beta = beta.min(MATE_SCORE - ply_i32);
        if alpha >= beta { return alpha; }
        
        // Quiescence search at leaf nodes
        if depth == 0 {
            return self.quiesce(tt, alpha, beta, heur, ply);
        }
        
        let original_alpha = alpha;
        let current_hash = self.hash;
        
        // Draw detection
        if !is_root && (self.is_fifty_move_draw() || self.is_threefold_repetition()) {
            return 0;
        }
        
        // Transposition table probe
        let mut hash_move: Option<Move> = None;
        let mut tt_score = 0;
        let mut tt_bound = BoundType::Exact;
        let found_tt = if excluded_move.is_none() {
            if let Some(entry) = tt.probe(current_hash) {
                hash_move = entry.best_move.clone();
                if entry.depth >= depth {
                    let score = self.score_from_tt(entry.score, ply_i32);
                    tt_score = score;
                    tt_bound = entry.bound_type;
                    
                    match entry.bound_type {
                        BoundType::Exact => return score,
                        BoundType::LowerBound => {
                            if score >= beta { return score; }
                            alpha = alpha.max(score);
                        },
                        BoundType::UpperBound => {
                            if score <= alpha { return score; }
                        }
                    }
                    if alpha >= beta { return score; }
                }
                true
            } else {
                false
            }
        } else {
            false
        };

        // Internal Iterative Deepening (IID): if no TT move and we're in a PV node at decent depth,
        // do a shallower search to obtain a likely good hash move to improve ordering.
        if hash_move.is_none()
            && crate::search_optimizations::SearchOptimizations::should_use_iid(depth, None, is_pv)
            && depth > 2
            && excluded_move.is_none()
        {
            let _ = self.negamax_enhanced(
                tt,
                depth - 2,
                alpha,
                beta,
                heur,
                ply,
                lmr_table,
                false,
                None,
            );
            if let Some(entry) = tt.probe(current_hash) {
                hash_move = entry.best_move.clone();
            }
        }
        
        // Evaluate position
        let in_check = self.is_in_check(self.current_color());
        let static_eval = if in_check { -MATE_SCORE } else { self.evaluate() };
        
        // TT eval adjustment 
        let eval = if found_tt && !in_check {
            match tt_bound {
                BoundType::LowerBound if tt_score > static_eval => tt_score,
                BoundType::UpperBound if tt_score < static_eval => tt_score,
                _ => static_eval,
            }
        } else {
            static_eval
        };
        
        // Improving flag (looking 2 plies back)
        let improving = if ply >= 2 {
            // This would need access to eval history - simplified for now
            eval >= static_eval - 25
        } else {
            true
        };
        
        // Node-level pruning (skip in check, PV, after null move, or when excluded)
        if !was_null_move && !in_check && !is_pv && excluded_move.is_none() {
            // Reverse Futility Pruning (Static Null Move)
            if depth <= 6 {
                let rfp_margin = 135 * depth as i32;
                if eval - rfp_margin >= beta {
                    return eval - rfp_margin;
                }
            }
            
            // Razoring
            if depth <= 3 && eval + 200 * (depth as i32) < beta {
                let razor_score = self.quiesce(tt, alpha, beta, heur, ply);
                if razor_score < beta {
                    return razor_score;
                }
            }
            
            // Null Move Pruning
            if depth > 1 && eval >= beta && self.can_try_null_move() {
                let reduction = 3 + depth / 6 + if eval - beta > 200 { 1 } else { 0 };
                
                let (prev_ep, prev_hash, prev_halfmove) = self.make_null_move();
                let null_score = -self.negamax_enhanced(
                    tt, 
                    depth.saturating_sub(1 + reduction), 
                    -beta, 
                    -beta + 1, 
                    heur, 
                    ply + 1, 
                    lmr_table,
                    true,
                    None
                );
                self.unmake_null_move(prev_ep, prev_hash, prev_halfmove);
                
                if heur.check_abort() { return alpha; }
                
                // Null move verification for deeper searches
                if depth - reduction > 5 && null_score >= beta {
                    let verify_score = self.negamax_enhanced(
                        tt,
                        depth.saturating_sub(reduction + 4),
                        alpha,
                        beta,
                        heur,
                        ply,
                        lmr_table,
                        true,
                        None
                    );
                    if verify_score >= beta {
                        return verify_score;
                    }
                }
                
                if null_score >= beta {
                    return null_score;
                }
            }
            
            // Probcut
            if let Some(probcut_score) = Probcut::try_probcut(self, tt, depth, beta, heur, ply) {
                return probcut_score;
            }
        }
        
        // Futility pruning flag
        let can_do_futility = depth <= 6 && !in_check && !is_pv && eval + 75 * (depth as i32) < beta;
        
        // Internal Iterative Reduction
        let mut search_depth = depth;
        if InternalIterativeReduction::should_reduce(depth, is_pv, hash_move.as_ref(), in_check) {
            search_depth = depth.saturating_sub(1);
        }
        
        // Generate and order moves
        let mut legal_moves = self.generate_moves();
        heur.ensure_ply(ply);
        crate::move_ordering::order_moves(&mut legal_moves, self, heur, hash_move.as_ref(), ply);
        
        if legal_moves.is_empty() {
            return if in_check { -MATE_SCORE + ply_i32 } else { 0 };
        }
        
        // Singular extension setup
        let mut singular_move = None;
        let mut try_singular = false;
        if let Some(ref tt_move) = hash_move {
            if SingularExtension::should_try_singular(depth, is_root, Some(tt_move), excluded_move) 
                && matches!(tt_bound, BoundType::LowerBound) 
                && tt_score < MATE_SCORE - 1000 {
                singular_move = Some(*tt_move);
                try_singular = true;
            }
        }
        
        let mut best_score = -MATE_SCORE * 2;
        let mut best_move: Option<Move> = None;
        let mut moves_tried = 0;
        let mut quiet_moves_tried = 0;
        
        for (move_idx, mv) in legal_moves.iter().enumerate() {
            if heur.check_abort() { break; }
            
            // Skip excluded move in singular search
            if let Some(excluded) = excluded_move {
                if *mv == excluded { continue; }
            }
            
            let is_capture = mv.captured_piece.is_some() || mv.is_en_passant || mv.promotion.is_some();
            let color_idx = color_to_zobrist_index(self.current_color());
            let from_idx = square_to_zobrist_index(mv.from) as usize;
            let to_idx = square_to_zobrist_index(mv.to) as usize;
            let hist_score = heur.get_history_score(color_idx, from_idx, to_idx);
            
            // Update butterfly table
            heur.butterfly[color_idx][from_idx][to_idx] += 1;
            
            // SEE pruning for bad captures (use cached SEE in heuristics)
            if is_capture && depth <= 3 && self.see(mv) < -50 * depth as i32 {
                continue;
            }
            
            // Late move pruning
            if !is_capture && mv.promotion.is_none() && !in_check && move_idx > 0 {
                let lmp_limit = lmp_threshold(depth, improving);
                if depth <= 3 && quiet_moves_tried >= lmp_limit {
                    continue;
                }
            }
            
            // Futility pruning (light): skip some late quiets in clear non-PV cases
            if can_do_futility && moves_tried > 0 && !is_capture && mv.promotion.is_none() {
                continue;
            }
            
            // Make move
            heur.ensure_ply(ply);
            if ply < heur.last_moves.len() {
                heur.last_moves[ply] = Some(*mv);
            }
            let info = self.make_move(mv);
            let gives_check = self.is_in_check(self.current_color());
            
            // Extensions
            let mut extension = 0;
            
            // Check extension
            if gives_check && (is_pv || depth < 4) {
                extension = 1;
            }
            
            // Singular extension
            if try_singular && Some(*mv) == singular_move && extension == 0 {
                if SingularExtension::try_singular_extension(
                    self, tt, *mv, tt_score, depth, alpha, beta, heur, ply
                ) {
                    extension = 1;
                }
            }
            
            let new_depth = search_depth.saturating_sub(1).saturating_add(extension);
            
            // Late Move Reduction
            let is_killer = ply < heur.killers.len() && 
                (heur.killers[ply][0] == Some(*mv) || heur.killers[ply][1] == Some(*mv));
            let reduction = if !is_capture && mv.promotion.is_none() {
                lmr_table.get_reduction(
                    is_pv,
                    depth,
                    moves_tried + 1,
                    improving,
                    gives_check,
                    is_killer,
                    hist_score
                )
            } else {
                0
            };
            
            let reduced_depth = new_depth.saturating_sub(reduction);
            
            // Search
            let score = if moves_tried == 0 {
                // First move - full window
                -self.negamax_enhanced(
                    tt, 
                    new_depth, 
                    -beta, 
                    -alpha, 
                    heur, 
                    ply + 1, 
                    lmr_table,
                    false,
                    None
                )
            } else {
                // Later moves - try reduced depth first if applicable
                let mut score = if reduction > 0 {
                    -self.negamax_enhanced(
                        tt,
                        reduced_depth,
                        -alpha - 1,
                        -alpha,
                        heur,
                        ply + 1,
                        lmr_table,
                        false,
                        None
                    )
                } else {
                    alpha + 1 // Skip reduced search
                };
                
                // Re-search at full depth if reduced search failed high
                if score > alpha {
                    score = -self.negamax_enhanced(
                        tt,
                        new_depth,
                        -alpha - 1,
                        -alpha,
                        heur,
                        ply + 1,
                        lmr_table,
                        false,
                        None
                    );
                    
                    // PV search if score is still above alpha
                    if score > alpha && score < beta {
                        score = -self.negamax_enhanced(
                            tt,
                            new_depth,
                            -beta,
                            -alpha,
                            heur,
                            ply + 1,
                            lmr_table,
                            false,
                            None
                        );
                    }
                }
                score
            };
            
            self.unmake_move(mv, info);
            
            if heur.check_abort() { break; }
            
            moves_tried += 1;
            if !is_capture && mv.promotion.is_none() {
                quiet_moves_tried += 1;
            }
            
            // Update best score
            if score > best_score {
                best_score = score;
                best_move = Some(*mv);
            }
            
            // Update history for good moves
            if !is_capture && mv.promotion.is_none() && score > original_alpha {
                let bonus = (depth as i32) * (depth as i32);
                heur.history[color_idx][from_idx][to_idx] += bonus;
            }
            
            alpha = alpha.max(score);
            if alpha >= beta {
                // Beta cutoff - update killers and history
                if !is_capture && mv.promotion.is_none() {
                    heur.ensure_ply(ply);
                    if heur.killers[ply][0].as_ref() != Some(mv) {
                        heur.killers[ply][1] = heur.killers[ply][0];
                        heur.killers[ply][0] = Some(*mv);
                    }
                    let bonus = (depth as i32) * (depth as i32);
                    heur.history[color_idx][from_idx][to_idx] += bonus;
                }
                break;
            }
        }
        
        // Store in transposition table (skip if excluded move search)
        if excluded_move.is_none() {
            let bound_type = if best_score <= original_alpha {
                BoundType::UpperBound
            } else if best_score >= beta {
                BoundType::LowerBound
            } else {
                BoundType::Exact
            };
            
            tt.store(
                current_hash,
                depth,
                self.score_to_tt(best_score, ply_i32),
                bound_type,
                best_move
            );
        }
        
        best_score
    }
    
    /// Check if null move is allowed (avoid zugzwang)
    fn can_try_null_move(&self) -> bool {
        let current_color = self.current_color();
    let _color_idx = if current_color == crate::board::Color::White { 0 } else { 1 };
        
        // Count non-pawn pieces
        let mut piece_count = 0;
        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.squares[rank][file] {
                    if color == current_color && piece != crate::Piece::Pawn && piece != crate::Piece::King {
                        piece_count += 1;
                    }
                }
            }
        }
        
        piece_count > 0
    }
    
    /// TT score conversion helpers
    fn score_to_tt(&self, score: i32, ply: i32) -> i32 {
        let near_mate = MATE_SCORE - 1000;
        if score >= near_mate {
            (score + ply).min(MATE_SCORE)
        } else if score <= -near_mate {
            (score - ply).max(-MATE_SCORE)
        } else {
            score
        }
    }
    
    fn score_from_tt(&self, score: i32, ply: i32) -> i32 {
        let near_mate = MATE_SCORE - 1000;
        if score >= near_mate {
            (score - ply).clamp(-MATE_SCORE, MATE_SCORE)
        } else if score <= -near_mate {
            (score + ply).clamp(-MATE_SCORE, MATE_SCORE)
        } else {
            score
        }
    }
}