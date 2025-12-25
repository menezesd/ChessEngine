//! Prelude module for convenient imports.
//!
//! This module re-exports the most commonly used types and functions.
//!
//! # Example
//! ```
//! use chess_engine::board::prelude::*;
//! ```

pub use super::{
    find_best_move, find_best_move_with_time, Board, BoardBuilder, CastlingRights, Color, FenError,
    Move, MoveList, MoveParseError, Piece, SearchState, Square, SquareError,
};
