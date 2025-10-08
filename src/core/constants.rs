//! Core constants for array indexing and basic values
//!
//! This module contains fundamental constants used for array indexing and basic
//! game values. More complex configuration parameters are in the config module.

// Re-export config constants for backward compatibility
pub use crate::core::config::game::*;
pub use crate::core::config::evaluation::*;
pub use crate::core::config::search::*;

// Re-export key constants directly for easier access
pub use crate::core::config::evaluation::{MATE_SCORE, KING_VALUE};

/// Piece index constants for array access (for backward compatibility)
pub const PAWN_INDEX: usize = 0;
pub const KNIGHT_INDEX: usize = 1;
pub const BISHOP_INDEX: usize = 2;
pub const ROOK_INDEX: usize = 3;
pub const QUEEN_INDEX: usize = 4;
pub const KING_INDEX: usize = 5;

/// Color index constants for array access (for backward compatibility)
pub const WHITE_INDEX: usize = 0;
pub const BLACK_INDEX: usize = 1;
