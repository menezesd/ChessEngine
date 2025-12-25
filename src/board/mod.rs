//! Chess board representation and game logic.
//!
//! Uses bitboards for efficient move generation and position evaluation.
//! Supports full chess rules including castling, en passant, and promotions.
//!
//! # Example
//! ```
//! use chess_engine::board::{Board, Color, Piece};
//!
//! let mut board = Board::new();
//! let moves = board.generate_moves();
//! println!("Starting position has {} legal moves", moves.len());
//! ```

mod attack_tables;
mod builder;
mod pst;
#[cfg(debug_assertions)]
mod debug;
mod error;
mod eval;
mod fen;
mod make_unmake;
mod movegen;
pub mod prelude;
mod san;
mod search;
mod state;
mod types;

#[cfg(test)]
mod tests;

// Public API - types users need
pub use builder::BoardBuilder;
pub use error::{FenError, MoveParseError, SanError, SquareError};
pub use state::Board;
pub use types::{Bitboard, CastlingRights, Color, Move, MoveList, MoveListIntoIter, Piece, Square};

// Public API - search functions and configuration
pub use search::{
    find_best_move, find_best_move_with_ponder, find_best_move_with_time,
    find_best_move_with_time_and_ponder, SearchClock, SearchLimits, SearchResult, SearchState,
    DEFAULT_TT_MB,
};

// Internal types exposed for advanced usage (but not in prelude)
pub use state::{NullMoveInfo, UnmakeInfo};
pub use types::SquareIdx;

// Re-export search internals for users who need fine-grained control
pub use search::{SearchParams, SearchStats, SearchTables};

pub(crate) use types::{
    bit_for_square, castle_bit, file_to_index, rank_to_index,
    CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K,
    CASTLE_WHITE_Q, EMPTY_MOVE, MAX_PLY, PROMOTION_PIECES,
};
