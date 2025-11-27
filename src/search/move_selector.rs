use crate::search::search_context::SearchContext;
use crate::transposition::transposition_table::{TranspositionTable, BoundType};
use crate::core::types::{Move, MoveList};
use crate::core::board::Board;
use crate::movegen::ordering::OrderingContext;
use crate::movegen::ordering::order_moves;

/// Helper for transposition table operations
pub struct TranspositionTableHelper;

impl TranspositionTableHelper {
    /// Probe TT and adjust alpha/beta bounds if we have a useful entry
    /// Returns (hash_move, adjusted_alpha, adjusted_beta, tt_cutoff_score, tt_stored_score)
    pub fn probe_and_adjust_bounds(
        tt: &mut TranspositionTable,
        hash: u64,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
    ) -> (Option<Move>, i32, i32, Option<i32>, Option<i32>) {
        if let Some(entry) = tt.probe(hash) {
            let stored_score = Some(entry.score);
            if entry.depth >= depth {
                match entry.bound_type {
                    BoundType::Exact => return (entry.best_move, alpha, beta, stored_score, stored_score),
                    BoundType::LowerBound => alpha = alpha.max(entry.score),
                    BoundType::UpperBound => beta = beta.min(entry.score),
                }
                if alpha >= beta {
                    return (entry.best_move, alpha, beta, stored_score, stored_score);
                }
            }
            return (entry.best_move, alpha, beta, None, stored_score);
        }
        (None, alpha, beta, None, None)
    }

    /// Store a search result in the transposition table
    pub fn store_result(tt: &mut TranspositionTable, hash: u64, depth: u32, score: i32, bound_type: BoundType, best_move: Option<Move>) {
        tt.store(hash, depth, score, bound_type, best_move);
    }

    /// Perform internal iterative deepening to find a good TT move
    pub fn internal_iterative_deepening(
        board: &mut Board,
        s_ctx: &mut SearchContext,
        hash: u64,
        depth: u32,
        alpha: i32,
        beta: i32,
    ) -> Option<Move> {
        use crate::search::algorithms::negamax;

        if depth >= 3 {
            // Need to adjust alpha/beta for the search; tt_cutoff_score will be ignored.
            let (_, adjusted_alpha, adjusted_beta, _, _) = TranspositionTableHelper::probe_and_adjust_bounds(s_ctx.tt, hash, depth, alpha, beta);
            let _ = negamax(board, s_ctx, depth - 2, adjusted_alpha, adjusted_beta);
            if let Some(entry) = s_ctx.tt.probe(hash) {
                return entry.best_move;
            }
        }
        None
    }
}

/// Abstraction for move generation, ordering, and iteration
pub struct MoveSelector {
    moves: MoveList,
    current_index: usize,
    stage: MoveStage,
    captures_end: usize,
    quiets_start: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum MoveStage {
    Captures,
    Quiets,
    Finished,
}

impl MoveSelector {
    pub fn new() -> Self {
        Self {
            moves: MoveList::new(),
            current_index: 0,
            stage: MoveStage::Captures,
            captures_end: 0,
            quiets_start: 0,
        }
    }

    /// Generate and order moves for the current position using staged approach
    pub fn generate_and_order(
        &mut self,
        board: &mut Board,
        ctx: &mut OrderingContext,
        depth: u32,
        hash_move: Option<Move>,
    ) {
        self.moves.clear();
        self.current_index = 0;
        self.stage = MoveStage::Captures;

        // Generate all moves
        board.generate_moves_into(&mut self.moves);

        // Separate captures and quiets
        let mut captures = Vec::new();
        let mut quiets = Vec::new();

        for m in &self.moves {
            if m.captured_piece.is_some() || m.promotion.is_some() {
                captures.push(*m);
            } else {
                quiets.push(*m);
            }
        }

        // Order captures by MVV-LVA and SEE
        captures.sort_by_key(|m| -crate::ordering::mvv_lva_score(m, board));
        captures.retain(|m| {
            let see = crate::see::see_capture(board, m);
            see >= 0
        });
        captures.sort_by_key(|m| -crate::see::see_capture(board, m));

        // Order quiets with full ordering logic
        order_moves(ctx, board, &mut quiets[..], depth as usize, hash_move);

        // Combine: captures first, then quiets
        self.moves.clear();
        self.moves.extend(captures);
        self.captures_end = self.moves.len();
        self.moves.extend(quiets);
        self.quiets_start = self.captures_end;

        // Ensure hash move is first in quiets if present
        if let Some(hm) = &hash_move {
            if hm.captured_piece.is_none() && hm.promotion.is_none() {
                // Hash move is a quiet
                if let Some(pos) = self.moves[self.quiets_start..].iter().position(|m| m == hm) {
                    let actual_pos = self.quiets_start + pos;
                    self.moves.swap(self.quiets_start, actual_pos);
                }
            }
        }
    }

    /// Get the next move in the staged order
    pub fn next(&mut self) -> Option<(usize, &Move)> {
        loop {
            match self.stage {
                MoveStage::Captures => {
                    if self.current_index < self.captures_end {
                        let mv = &self.moves[self.current_index];
                        let idx = self.current_index;
                        self.current_index += 1;
                        return Some((idx, mv));
                    } else {
                        self.stage = MoveStage::Quiets;
                        self.current_index = self.quiets_start;
                    }
                }
                MoveStage::Quiets => {
                    if self.current_index < self.moves.len() {
                        let mv = &self.moves[self.current_index];
                        let idx = self.current_index;
                        self.current_index += 1;
                        return Some((idx, mv));
                    } else {
                        self.stage = MoveStage::Finished;
                        return None;
                    }
                }
                MoveStage::Finished => return None,
            }
        }
    }

    /// Check if there are more moves
    pub fn has_more(&self) -> bool {
        self.stage != MoveStage::Finished
    }

    /// Get total number of moves
    pub fn len(&self) -> usize {
        self.moves.len()
    }

    /// Check if move list is empty
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
}