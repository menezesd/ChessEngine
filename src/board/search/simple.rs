//! Core search implementation.
//!
//! This module implements:
//! - Iterative deepening with aspiration windows
//! - Alpha-beta search with null move pruning and LMR
//! - Quiescence search with stand-pat
//! - Move ordering (TT move, killers, MVV-LVA, history)

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::tt::BoundType;

use super::{SearchState, MATE_SCORE};
use crate::board::{Board, Move, MoveList, EMPTY_MOVE, MAX_PLY};

/// Maximum quiescence depth
const MAX_QSEARCH_DEPTH: i32 = 3;

/// Mate score threshold
const MATE_THRESHOLD: i32 = 27000;

/// TT move priority in move ordering
const TT_MOVE_SCORE: i32 = 1 << 20;
/// First killer move priority
const KILLER1_SCORE: i32 = 20000;
/// Second killer move priority
const KILLER2_SCORE: i32 = 10000;
/// Minimum score to avoid LMR
const LMR_SCORE_THRESHOLD: i32 = 2500;

/// Search context for a single search
pub struct SimpleSearchContext<'a> {
    pub board: &'a mut Board,
    pub state: &'a mut SearchState,
    pub stop: &'a AtomicBool,
    pub start_time: Instant,
    pub time_limit_ms: u64,
    pub node_limit: u64,
    pub nodes: u64,
    pub initial_depth: u32,
}

impl SimpleSearchContext<'_> {
    /// Check if we should stop searching
    #[inline]
    fn should_stop(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        if self.node_limit > 0 && self.nodes >= self.node_limit {
            return true;
        }
        if self.time_limit_ms > 0 {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if elapsed >= self.time_limit_ms {
                return true;
            }
        }
        false
    }

    /// Evaluate position from side-to-move's perspective
    #[inline]
    fn evaluate(&self) -> i32 {
        self.board.evaluate()
    }

    /// Check for repetition (returns true if position repeated)
    #[inline]
    fn is_repetition(&self) -> bool {
        self.board.repetition_counts.get(self.board.hash) > 1
    }

    /// Order moves for better pruning (TT move > killers > captures > history)
    fn order_moves(&self, moves: &MoveList, tt_move: Move, ply: usize) -> Vec<(Move, i32)> {
        moves
            .iter()
            .map(|m| {
                let score = if *m == tt_move {
                    TT_MOVE_SCORE
                } else if ply < MAX_PLY && *m == self.state.tables.killer_moves[ply][0] {
                    KILLER1_SCORE
                } else if ply < MAX_PLY && *m == self.state.tables.killer_moves[ply][1] {
                    KILLER2_SCORE
                } else if m.captured_piece.is_some() {
                    self.state.tables.mvv_lva_score(m)
                } else {
                    self.state.tables.history_score(m)
                };
                (*m, score)
            })
            .collect()
    }

    /// Try null move pruning, returns Some(score) if cutoff achieved
    fn try_null_move_pruning(&mut self, depth: u32, beta: i32, in_check: bool) -> Option<i32> {
        let dominated_phase = if self.board.white_to_move {
            self.board.game_phase[0]
        } else {
            self.board.game_phase[1]
        };

        if in_check || dominated_phase == 0 || depth <= 2 || depth >= self.initial_depth {
            return None;
        }

        let r = 1 + (depth + 1) / 3;
        let info = self.board.make_null_move();
        let score = -self.alphabeta(depth.saturating_sub(r + 1), -beta, -beta + 1, false);
        self.board.unmake_null_move(info);

        if score >= beta {
            Some(beta)
        } else {
            None
        }
    }

    /// Handle beta cutoff: update killers, history, and TT
    fn handle_beta_cutoff(&mut self, m: &Move, ply: usize, depth: u32, score: i32, best_move: Move) {
        // Update killers for quiet moves
        if m.captured_piece.is_none() && ply < MAX_PLY {
            let killers = &mut self.state.tables.killer_moves[ply];
            if killers[0] != *m {
                killers[1] = killers[0];
                killers[0] = *m;
            }
        }

        // Update history
        self.state.tables.update_history(m, depth);

        // Store in TT
        if !self.should_stop() && score.abs() < 29000 {
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

    /// Store position in transposition table
    fn store_tt(&mut self, depth: u32, score: i32, raised_alpha: bool, best_move: Move) {
        if self.should_stop() || score.abs() >= 29000 || best_move == EMPTY_MOVE {
            return;
        }
        let bound = if raised_alpha {
            BoundType::Exact
        } else {
            BoundType::UpperBound
        };
        self.state.tables.tt.store(
            self.board.hash,
            depth,
            score,
            bound,
            Some(best_move),
            self.state.generation,
        );
    }

    /// Quiescence search for tactical stability
    pub fn quiesce(&mut self, mut alpha: i32, beta: i32, qdepth: i32) -> i32 {
        let stand_pat = self.evaluate();

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

        // Sort captures by MVV-LVA
        let mut sorted_moves: Vec<(Move, i32)> = moves
            .into_iter()
            .map(|m| {
                let score = self.state.tables.mvv_lva_score(&m);
                (m, score)
            })
            .collect();
        sorted_moves.sort_by(|a, b| b.1.cmp(&a.1));

        for (m, _) in sorted_moves {
            self.nodes += 1;
            let info = self.board.make_move(&m);
            let score = -self.quiesce(-beta, -alpha, qdepth + 1);
            self.board.unmake_move(&m, info);

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

    /// Alpha-beta search with null move pruning and LMR
    pub fn alphabeta(
        &mut self,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        allow_null: bool,
    ) -> i32 {
        // Repetition check
        if self.is_repetition() {
            return 0;
        }

        // Quiescence at leaf
        if depth == 0 {
            return self.quiesce(alpha, beta, 0);
        }

        self.nodes += 1;

        // Check for missing king (illegal position)
        if self.board.find_king(self.board.current_color()).is_none() {
            return -29000;
        }

        let ply = (self.initial_depth - depth) as usize;
        let in_check = self.board.is_in_check(self.board.current_color());

        // Probe TT
        let tt_entry = self.state.tables.tt.probe(self.board.hash);
        let mut tt_move = EMPTY_MOVE;

        if let Some(entry) = tt_entry {
            if let Some(mv) = entry.best_move() {
                tt_move = mv;
            }
            if entry.depth() >= depth && !self.is_repetition() {
                let score = entry.score();
                match entry.bound_type() {
                    BoundType::Exact => return score,
                    BoundType::LowerBound => {
                        if score >= beta {
                            return score;
                        }
                    }
                    BoundType::UpperBound => {
                        if score <= alpha {
                            return score;
                        }
                    }
                }
            }
        }

        // Generate moves
        let moves = self.board.generate_moves();
        if moves.is_empty() {
            return if in_check {
                -(MATE_THRESHOLD + depth as i32) // Checkmate
            } else {
                0 // Stalemate
            };
        }

        // Move ordering: TT move, killers, captures, history
        let mut scored_moves = self.order_moves(&moves, tt_move, ply);
        if depth > 1 {
            scored_moves.sort_by(|a, b| b.1.cmp(&a.1));
        }

        let move_count = scored_moves.len();

        // Null move pruning
        if allow_null {
            if let Some(score) = self.try_null_move_pruning(depth, beta, in_check) {
                return score;
            }
        }

        let mut best_score = -30000i32;
        let mut best_move = EMPTY_MOVE;
        let mut raised_alpha = false;

        for (i, (m, move_score)) in scored_moves.iter().enumerate() {
            if self.should_stop() {
                break;
            }

            let info = self.board.make_move(m);

            let mut score: i32;

            // LMR: reduce late moves with low scores
            let lmr_ok = i > 3 + move_count / 4 && *move_score < LMR_SCORE_THRESHOLD && depth > 1;
            let reduction = if lmr_ok { 2 } else { 1 };

            if i > 0 {
                // PVS: null window search for non-first moves
                score = -self.alphabeta(depth.saturating_sub(reduction), -alpha - 1, -alpha, true);
                if score > alpha && score < beta {
                    score = -self.alphabeta(depth.saturating_sub(reduction), -beta, -alpha, true);
                }
            } else {
                score = -self.alphabeta(depth - 1, -beta, -alpha, true);
            }

            self.board.unmake_move(m, info);

            if score > best_score {
                best_score = score;
                best_move = *m;

                if score > alpha {
                    if score >= beta {
                        self.handle_beta_cutoff(m, ply, depth, score, best_move);
                        return score;
                    }
                    alpha = score;
                    raised_alpha = true;
                }
            }
        }

        self.store_tt(depth, best_score, raised_alpha, best_move);
        best_score
    }

    /// Iterative deepening with aspiration windows
    pub fn iterative_deepening(&mut self, max_depth: u32) -> Option<Move> {
        let mut best_move: Option<Move> = None;
        let mut score = self.evaluate();

        // Reset history at start of search
        self.state.tables.reset_history();

        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }

            self.initial_depth = depth;

            // Aspiration window
            let mut delta = 30;
            let mut alpha = score - delta;
            let mut beta = score + delta;

            loop {
                let new_score = self.alphabeta(depth, alpha, beta, true);

                if self.should_stop() {
                    break;
                }

                if new_score >= beta {
                    beta += delta;
                    delta += delta;
                } else if new_score <= alpha {
                    alpha -= delta;
                    delta += delta;
                } else {
                    score = new_score;
                    break;
                }
            }

            // Get best move from TT
            if let Some(entry) = self.state.tables.tt.probe(self.board.hash) {
                if let Some(mv) = entry.best_move() {
                    if mv != EMPTY_MOVE {
                        // Verify move is legal
                        let moves = self.board.generate_moves();
                        if moves.iter().any(|m| *m == mv) {
                            best_move = Some(mv);
                        }
                    }
                }
            }

            // Print info
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if score.abs() < MATE_THRESHOLD {
                println!(
                    "info depth {} nodes {} time {} score cp {}",
                    depth, self.nodes, elapsed, score
                );
            } else {
                let mate_in = if score > 0 {
                    (MATE_SCORE - score + 1) / 2
                } else {
                    -(MATE_SCORE + score + 1) / 2
                };
                println!(
                    "info depth {} nodes {} time {} score mate {}",
                    depth, self.nodes, elapsed, mate_in
                );
            }
        }

        best_move
    }
}

/// Run the main search algorithm
pub fn simple_search(
    board: &mut Board,
    state: &mut SearchState,
    max_depth: u32,
    time_limit_ms: u64,
    node_limit: u64,
    stop: &AtomicBool,
) -> Option<Move> {
    // Increment generation for TT aging
    state.generation = state.generation.wrapping_add(1);

    let mut ctx = SimpleSearchContext {
        board,
        state,
        stop,
        start_time: Instant::now(),
        time_limit_ms,
        node_limit,
        nodes: 0,
        initial_depth: 1,
    };

    // Check for single legal move
    let moves = ctx.board.generate_moves();
    if moves.is_empty() {
        return None;
    }
    if moves.len() == 1 {
        return moves.first();
    }

    ctx.iterative_deepening(max_depth)
}
