//! Search constants and parameters.
//!
//! Contains all tuned constants used by the alpha-beta search.

// ============================================================================
// SEARCH LIMITS
// ============================================================================

/// Maximum quiescence search depth to prevent explosion
/// Higher values find more tactics but increase nodes searched
pub const MAX_QSEARCH_DEPTH: i32 = 12;

/// Scores with absolute value >= this are considered checkmate scores
pub const MATE_THRESHOLD: i32 = 28000;

/// Maximum score bound for alpha-beta window
pub const SCORE_INFINITE: i32 = 30000;

/// Safe score limit (for correction history clamping)
pub const SCORE_SAFE_MAX: i32 = 29000;

/// Threshold for considering a score "near mate" (skip certain pruning)
pub const SCORE_NEAR_MATE: i32 = 20000;

// ============================================================================
// MOVE ORDERING PRIORITIES
// ============================================================================
// Higher scores = tried earlier. Ordered: TT > killers > counter > captures > quiet

/// Hash move (from transposition table) - highest priority
pub const TT_MOVE_SCORE: i32 = 1 << 20;

/// Base score for captures (added to MVV-LVA to ensure captures > killers)
pub const CAPTURE_BASE_SCORE: i32 = 100000;

/// First killer move (quiet that caused beta cutoff at same ply)
pub const KILLER1_SCORE: i32 = 20000;

/// Second killer move (replaced killer)
pub const KILLER2_SCORE: i32 = 10000;

/// Third killer move
pub const KILLER3_SCORE: i32 = 7500;

/// Counter move (quiet that refuted opponent's previous move)
pub const COUNTER_SCORE: i32 = 5000;

/// Moves with score above this are exempt from late move reductions
pub const LMR_SCORE_THRESHOLD: i32 = 2500;

// ========================================================================
// REDUCTIONS
// ========================================================================

/// Base null-move reduction increment
pub const NULL_MOVE_BASE_REDUCTION: u32 = 1;

/// LMR starts after this many moves (moves with idx > `LMR_IDX_BASE` + `move_count/4`)
pub const LMR_IDX_BASE: usize = 3;

/// LMR reduction table dimensions (depth x move index buckets)
pub const LMR_TABLE_MAX_DEPTH: usize = 32;
pub const LMR_TABLE_MAX_IDX: usize = 256;

// ============================================================================
// EXTENSIONS
// ============================================================================

/// Pre-promotion rank for white pawns (0-indexed: rank 7 = index 6)
pub const PAWN_EXTENSION_RANK_WHITE: usize = 6;

/// Pre-promotion rank for black pawns (0-indexed: rank 2 = index 1)
pub const PAWN_EXTENSION_RANK_BLACK: usize = 1;

// No pruning margins are currently used.

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_score_bounds() {
        // SCORE_INFINITE should be the highest
        assert!(SCORE_INFINITE > SCORE_SAFE_MAX);
        assert!(SCORE_SAFE_MAX > MATE_THRESHOLD);
        // MATE_THRESHOLD is for detecting checkmate scores
        assert!(MATE_THRESHOLD > SCORE_NEAR_MATE);
    }

    #[test]
    fn test_mate_threshold() {
        // Mate threshold should distinguish mate from normal scores
        assert!(MATE_THRESHOLD > 10000);
        assert!(MATE_THRESHOLD < SCORE_INFINITE);
    }

    #[test]
    fn test_move_ordering_priorities() {
        // TT move should be highest priority
        assert!(TT_MOVE_SCORE > CAPTURE_BASE_SCORE);
        // Captures should beat killers
        assert!(CAPTURE_BASE_SCORE > KILLER1_SCORE);
        // Killer ordering
        assert!(KILLER1_SCORE > KILLER2_SCORE);
        assert!(KILLER2_SCORE > KILLER3_SCORE);
        assert!(KILLER3_SCORE > COUNTER_SCORE);
    }

    #[test]
    fn test_lmr_threshold() {
        // LMR threshold should be between counter and killer scores
        assert!(LMR_SCORE_THRESHOLD < COUNTER_SCORE);
        assert!(LMR_SCORE_THRESHOLD > 0);
    }

    #[test]
    fn test_lmr_table_dimensions() {
        // LMR table should have reasonable dimensions
        assert!(LMR_TABLE_MAX_DEPTH >= 32);
        assert!(LMR_TABLE_MAX_IDX >= 64);
    }

    #[test]
    fn test_pawn_extension_ranks() {
        // White pre-promotion rank is 7th rank (index 6)
        assert_eq!(PAWN_EXTENSION_RANK_WHITE, 6);
        // Black pre-promotion rank is 2nd rank (index 1)
        assert_eq!(PAWN_EXTENSION_RANK_BLACK, 1);
    }

    #[test]
    fn test_qsearch_depth() {
        // Qsearch should have reasonable max depth
        assert!(MAX_QSEARCH_DEPTH >= 8);
        assert!(MAX_QSEARCH_DEPTH <= 20);
    }
}
