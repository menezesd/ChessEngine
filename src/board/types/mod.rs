//! Core chess types.
//!
//! This module contains the fundamental types used throughout the chess engine:
//! - `Piece` and `Color` - chess piece types and colors
//! - `Square` - compact board square representation (u8)
//! - `Bitboard` - 64-bit board representation
//! - `Move` and `MoveList` - move representation
//! - `CastlingRights` - castling state

mod bitboard;
mod castling;
mod indices;
mod moves;
mod piece;
mod square;

// Re-export all public types
pub use bitboard::Bitboard;
pub use castling::CastlingRights;
#[allow(unused_imports)]
pub use indices::{ColorIndex, PieceIndex};
pub(crate) use moves::ScoredMoveList;
pub use moves::{Move, MoveList, MoveListIntoIter};
pub use piece::{Color, Piece};
pub use square::Square;

// Re-export internal utilities
pub(crate) use bitboard::bit_for_square;
pub(crate) use castling::{
    castle_bit, ALL_CASTLING_RIGHTS, CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q,
};
pub(crate) use moves::{EMPTY_MOVE, MAX_PLY};
pub(crate) use piece::PROMOTION_PIECES;
pub(crate) use square::{file_to_index, rank_to_index};
