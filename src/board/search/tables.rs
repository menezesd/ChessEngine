use std::sync::Arc;

use crate::board::nnue::NnueNetwork;
use crate::board::{Board, Move, Piece};
use crate::tt::TranspositionTable;

use super::constants;
use super::move_order;

mod heuristics;

const MVV_LVA_VICTIM_SCALE: i32 = 10;
const SEE_SCORE_DIVISOR: i32 = 10;
const CAPTURE_HISTORY_SCORE_DIVISOR: i32 = 100;

pub use heuristics::{
    CaptureHistory, ContinuationHistory, CorrectionHistory, CounterMoveTable, CountermoveHistory,
    HistoryTable, KillerTable,
};

/// Tables used during search (TT, killers, history, counter moves)
pub struct SearchTables {
    /// Shared transposition table (thread-safe, can be shared across workers)
    pub tt: Arc<TranspositionTable>,
    /// Shared pawn hash table (thread-safe, can be shared across workers)
    pub pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
    /// Shared NNUE network (optional, loaded from file)
    pub nnue: Option<Arc<NnueNetwork>>,
    /// Optional NNUE used only for qsearch/pruning static evaluation.
    pub static_nnue: Option<Arc<NnueNetwork>>,
    /// Per-thread killer move table
    pub killer_moves: KillerTable,
    /// Per-thread history heuristic table
    pub history: HistoryTable,
    /// Per-thread counter move table
    pub counter_moves: CounterMoveTable,
    /// Per-thread continuation history table
    pub continuation_history: ContinuationHistory,
    /// Per-thread countermove history table (response to opponent's move)
    pub countermove_history: CountermoveHistory,
    /// Per-thread capture history table
    pub capture_history: CaptureHistory,
    /// Per-thread correction history table
    pub correction_history: CorrectionHistory,
}

impl SearchTables {
    fn new_per_thread_tables() -> (
        KillerTable,
        HistoryTable,
        CounterMoveTable,
        ContinuationHistory,
        CountermoveHistory,
        CaptureHistory,
        CorrectionHistory,
    ) {
        (
            KillerTable::new(),
            HistoryTable::new(),
            CounterMoveTable::new(),
            ContinuationHistory::new(),
            CountermoveHistory::new(),
            CaptureHistory::new(),
            CorrectionHistory::new(),
        )
    }

    /// Create a new `SearchTables` with a fresh transposition table of the given size.
    #[must_use]
    pub fn new(tt_mb: usize) -> Self {
        let (
            killer_moves,
            history,
            counter_moves,
            continuation_history,
            countermove_history,
            capture_history,
            correction_history,
        ) = Self::new_per_thread_tables();

        SearchTables {
            tt: Arc::new(TranspositionTable::new(tt_mb)),
            pawn_hash: Arc::new(crate::pawn_hash::PawnHashTable::default()),
            nnue: None,
            static_nnue: None,
            killer_moves,
            history,
            counter_moves,
            continuation_history,
            countermove_history,
            capture_history,
            correction_history,
        }
    }

    /// Create a new `SearchTables` with shared TT, pawn hash, and NNUE.
    ///
    /// Used for SMP workers that share these tables but have separate per-thread tables.
    #[must_use]
    pub fn with_shared(
        tt: Arc<TranspositionTable>,
        pawn_hash: Arc<crate::pawn_hash::PawnHashTable>,
        nnue: Option<Arc<NnueNetwork>>,
        static_nnue: Option<Arc<NnueNetwork>>,
    ) -> Self {
        let (
            killer_moves,
            history,
            counter_moves,
            continuation_history,
            countermove_history,
            capture_history,
            correction_history,
        ) = Self::new_per_thread_tables();

        SearchTables {
            tt,
            pawn_hash,
            nnue,
            static_nnue,
            killer_moves,
            history,
            counter_moves,
            continuation_history,
            countermove_history,
            capture_history,
            correction_history,
        }
    }

    /// MVV-LVA score for a capture move, enhanced with capture history
    /// Prioritizes capturing high-value pieces with low-value attackers,
    /// with capture history as a secondary factor
    #[must_use]
    pub fn mvv_lva_score(&self, board: &Board, mv: &Move) -> i32 {
        if !mv.is_capture() {
            return 0;
        }

        let Some((_, attacker_piece)) = board.piece_at(mv.from()) else {
            return 0;
        };
        let attacker = move_order::piece_value(attacker_piece);

        if mv.is_en_passant() {
            let mvv_lva = mvv_lva_value(move_order::piece_value(Piece::Pawn), attacker);
            return capture_order_score(
                0,
                mvv_lva,
                0,
                self.capture_history_bonus(attacker_piece, Piece::Pawn),
            );
        }

        let Some((_, victim_piece)) = board.piece_at(mv.to()) else {
            return 0;
        };
        let captured = move_order::piece_value(victim_piece);
        let mvv_lva = mvv_lva_value(captured, attacker);

        let enemy_king_sq = board.find_king(board.side_to_move().opponent());
        let to_sq = mv.to();
        let rank_diff = (to_sq.rank() as i32 - enemy_king_sq.rank() as i32).abs();
        let file_diff = (to_sq.file() as i32 - enemy_king_sq.file() as i32).abs();
        let near_king = rank_diff <= 1 && file_diff <= 1;

        let see_score = if attacker > captured && !near_king {
            board.see_with_pieces(mv.from(), mv.to(), attacker_piece, victim_piece)
                / SEE_SCORE_DIVISOR
        } else {
            0
        };

        capture_order_score(
            constants::CAPTURE_BASE_SCORE,
            mvv_lva,
            see_score,
            self.capture_history_bonus(attacker_piece, victim_piece),
        )
    }

    fn capture_history_bonus(&self, attacker: Piece, victim: Piece) -> i32 {
        self.capture_history.score(attacker, victim) / CAPTURE_HISTORY_SCORE_DIVISOR
    }

    /// Get history score for a move
    #[must_use]
    pub fn history_score(&self, mv: &Move) -> i32 {
        self.history.score(mv)
    }

    /// Update history on beta cutoff with gravity
    pub fn update_history(&mut self, mv: &Move, depth: u32) {
        self.history.update(mv, depth);
    }

    /// Reset history table
    pub fn reset_history(&mut self) {
        self.history.reset();
    }

    /// Decay history table (preserves some information from previous searches)
    pub fn decay_history(&mut self) {
        self.history.decay();
    }
}

fn mvv_lva_value(captured: i32, attacker: i32) -> i32 {
    captured
        .saturating_mul(MVV_LVA_VICTIM_SCALE)
        .saturating_sub(attacker)
}

fn capture_order_score(base: i32, mvv_lva: i32, see_score: i32, history: i32) -> i32 {
    base.saturating_add(mvv_lva)
        .saturating_add(see_score)
        .saturating_add(history)
}

#[cfg(test)]
mod tests {
    use super::{capture_order_score, mvv_lva_value};

    #[test]
    fn mvv_lva_value_saturates_extreme_inputs() {
        assert_eq!(mvv_lva_value(i32::MAX, i32::MIN), i32::MAX);
        assert_eq!(mvv_lva_value(i32::MIN, i32::MAX), i32::MIN);
    }

    #[test]
    fn capture_order_score_saturates_extreme_inputs() {
        assert_eq!(
            capture_order_score(i32::MAX, i32::MAX, i32::MAX, i32::MAX),
            i32::MAX
        );
        assert_eq!(
            capture_order_score(i32::MIN, i32::MIN, i32::MIN, i32::MIN),
            i32::MIN
        );
    }
}
