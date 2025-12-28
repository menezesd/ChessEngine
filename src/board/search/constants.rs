//! Search constants and parameters.
//!
//! Contains all tuned constants used by the alpha-beta search.

// ============================================================================
// SEARCH LIMITS
// ============================================================================

/// Maximum quiescence search depth to prevent explosion
pub const MAX_QSEARCH_DEPTH: i32 = 4;

/// Scores with absolute value >= this are considered checkmate scores
pub const MATE_THRESHOLD: i32 = 28000;

// ============================================================================
// MOVE ORDERING PRIORITIES
// ============================================================================
// Higher scores = tried earlier. Ordered: TT > killers > counter > captures > quiet

/// Hash move (from transposition table) - highest priority
pub const TT_MOVE_SCORE: i32 = 1 << 20;

/// First killer move (quiet that caused beta cutoff at same ply)
pub const KILLER1_SCORE: i32 = 20000;

/// Second killer move (replaced killer)
pub const KILLER2_SCORE: i32 = 10000;

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

// No pruning margins are currently used.
