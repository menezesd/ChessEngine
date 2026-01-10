//! Evaluation constants and tables.
//!
//! Contains all tuned evaluation parameters used by the evaluation functions.

// ============================================================================
// MOBILITY TABLES
// ============================================================================

/// Knight mobility bonus (0-8 squares)
pub const KNIGHT_MOB_MG: [i32; 9] = [-28, -14, -2, 4, 8, 12, 17, 21, 25];
pub const KNIGHT_MOB_EG: [i32; 9] = [-28, -18, -8, 0, 6, 10, 14, 18, 22];

/// Bishop mobility bonus (0-13 squares)
pub const BISHOP_MOB_MG: [i32; 14] = [-30, -18, -8, 0, 6, 12, 17, 21, 24, 27, 29, 31, 33, 35];
pub const BISHOP_MOB_EG: [i32; 14] = [-30, -18, -8, 0, 6, 10, 14, 17, 20, 22, 24, 26, 28, 30];

/// Rook mobility bonus (0-14 squares)
pub const ROOK_MOB_MG: [i32; 15] = [-14, -8, -3, 0, 3, 6, 9, 12, 14, 16, 18, 20, 21, 22, 23];
pub const ROOK_MOB_EG: [i32; 15] = [-28, -16, -8, 0, 6, 12, 17, 21, 25, 28, 31, 34, 36, 38, 40];

/// Queen mobility bonus (0-27 squares)
pub const QUEEN_MOB_MG: [i32; 28] = [
    -14, -10, -6, -3, 0, 2, 4, 6, 8, 10, 11, 12, 13, 14, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20,
    20, 20, 21, 21,
];
pub const QUEEN_MOB_EG: [i32; 28] = [
    -28, -18, -10, -4, 0, 4, 8, 11, 14, 17, 19, 21, 23, 25, 26, 27, 28, 29, 30, 31, 32, 32, 33, 33,
    34, 34, 35, 35,
];

// ============================================================================
// PAWN STRUCTURE CONSTANTS
// ============================================================================

/// Doubled pawn penalty (Texel tuned v2)
pub const DOUBLED_PAWN_MG: i32 = -10;
pub const DOUBLED_PAWN_EG: i32 = 0;

/// Isolated pawn penalty (Texel tuned v2)
pub const ISOLATED_PAWN_MG: i32 = -7;
pub const ISOLATED_PAWN_EG: i32 = -9;

/// Extra penalty for isolated pawn on open file
pub const ISOLATED_OPEN_MG: i32 = -9;
pub const ISOLATED_OPEN_EG: i32 = 0;

/// Backward pawn penalty
pub const BACKWARD_PAWN_MG: i32 = -2;
pub const BACKWARD_PAWN_EG: i32 = -1;

/// Extra penalty for backward pawn on open file
pub const BACKWARD_OPEN_MG: i32 = -6;
pub const BACKWARD_OPEN_EG: i32 = 0;

/// Phalanx (side-by-side pawns) bonus by rank
pub const PHALANX_BONUS_MG: [i32; 8] = [0, 0, 3, 5, 12, 25, 50, 0];
pub const PHALANX_BONUS_EG: [i32; 8] = [0, 0, 2, 4, 8, 15, 30, 0];

/// Defended pawn bonus by rank
pub const DEFENDED_BONUS_MG: [i32; 8] = [0, 0, 5, 8, 12, 20, 35, 0];
pub const DEFENDED_BONUS_EG: [i32; 8] = [0, 0, 3, 5, 8, 12, 20, 0];

// ============================================================================
// KING SAFETY CONSTANTS
// ============================================================================

/// Attack weights per piece type [piece] = (strong, weak)
/// Strong = attack on undefended king zone square
/// Weak = attack on defended king zone square
pub const ATTACK_WEIGHTS: [(i32, i32); 6] = [
    (0, 0), // Pawn (don't count)
    (4, 3), // Knight
    (4, 3), // Bishop
    (6, 4), // Rook
    (7, 5), // Queen
    (0, 0), // King
];

/// Queen check threat bonus
pub const QUEEN_CHECK_BONUS: i32 = 2;

/// King shield bonus per pawn
pub const KING_SHIELD_BONUS_MG: i32 = 8;

/// Open file near king penalty
pub const KING_OPEN_FILE_MG: i32 = -25;
pub const KING_SEMI_OPEN_FILE_MG: i32 = -15;

// ============================================================================
// ROOK ACTIVITY CONSTANTS
// ============================================================================

/// Rook on open file (no pawns) (Texel tuned v2)
pub const ROOK_OPEN_FILE_MG: i32 = 50;
pub const ROOK_OPEN_FILE_EG: i32 = 22;

/// Rook on semi-open file (only enemy pawns) (Texel tuned v2)
pub const ROOK_SEMI_OPEN_MG: i32 = 14;
pub const ROOK_SEMI_OPEN_EG: i32 = 24;

/// Rook on 7th rank (Texel tuned v2)
pub const ROOK_7TH_MG: i32 = 29;
pub const ROOK_7TH_EG: i32 = 42;

/// Rook trapped by uncastled king
pub const TRAPPED_ROOK_MG: i32 = -40;

// ============================================================================
// HANGING PIECES CONSTANTS
// ============================================================================

/// Hanging piece penalty by type (attacked and undefended)
pub const HANGING_PENALTY: [i32; 6] = [
    10, // Pawn
    40, // Knight
    40, // Bishop
    60, // Rook
    80, // Queen
    0,  // King (can't be hanging)
];

/// Minor piece attacking minor piece bonus
pub const MINOR_ON_MINOR: i32 = 8;

// ============================================================================
// MINOR PIECE CONSTANTS
// ============================================================================

/// Knight outpost bonus (protected by pawn, can't be attacked by enemy pawns)
pub const KNIGHT_OUTPOST_MG: i32 = 20;
pub const KNIGHT_OUTPOST_EG: i32 = 15;

/// Bishop outpost bonus (smaller than knight - bishops prefer open diagonals)
pub const BISHOP_OUTPOST_MG: i32 = 10;
pub const BISHOP_OUTPOST_EG: i32 = 8;

/// Bad bishop penalty per blocking pawn (bishop blocked by own pawns on same color)
pub const BAD_BISHOP_MG: i32 = -5;
pub const BAD_BISHOP_EG: i32 = -8;

// ============================================================================
// CONNECTED ROOKS & ROOK BEHIND PASSED PAWN
// ============================================================================

/// Connected rooks bonus (rooks on same rank/file defending each other)
pub const CONNECTED_ROOKS_MG: i32 = 10;
pub const CONNECTED_ROOKS_EG: i32 = 8;

/// Rook behind passed pawn bonus (supporting or blocking)
pub const ROOK_BEHIND_PASSER_MG: i32 = 15;
pub const ROOK_BEHIND_PASSER_EG: i32 = 25;

// ============================================================================
// KING TROPISM (piece proximity to enemy king)
// ============================================================================

/// Queen tropism bonus per square closer to enemy king
pub const QUEEN_TROPISM_MG: i32 = 2;

/// Rook tropism bonus per square closer to enemy king
pub const ROOK_TROPISM_MG: i32 = 1;
