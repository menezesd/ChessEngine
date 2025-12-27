//! Chess engine library implementing UCI protocol.
//!
//! Provides a complete chess engine with:
//! - Bitboard-based board representation
//! - Alpha-beta search with iterative deepening
//! - Transposition tables with Zobrist hashing
//! - UCI protocol support for GUI integration
//!
//! # Quick Start
//!
//! ```
//! use chess_engine::board::{Board, find_best_move, SearchState};
//! use std::sync::atomic::AtomicBool;
//!
//! // Create a new game from starting position
//! let mut board = Board::new();
//!
//! // Generate all legal moves
//! let moves = board.generate_moves();
//! println!("Available moves: {}", moves.len());
//!
//! // Find the best move (depth 4)
//! let mut state = SearchState::new(64);
//! let stop = AtomicBool::new(false);
//! if let Some(best) = find_best_move(&mut board, &mut state, 4, &stop) {
//!     println!("Best move: {}", best);
//! }
//! ```
//!
//! # Building Positions
//!
//! ```
//! use chess_engine::board::{Board, BoardBuilder, Color, Piece, Square};
//!
//! // From FEN notation
//! let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
//!
//! // Using the builder
//! let board = BoardBuilder::new()
//!     .piece(Square::new(0, 4), Color::White, Piece::King)
//!     .piece(Square::new(7, 4), Color::Black, Piece::King)
//!     .piece(Square::new(1, 0), Color::White, Piece::Pawn)
//!     .side_to_move(Color::White)
//!     .build();
//! ```
//!
//! # Making Moves
//!
//! ```
//! use chess_engine::board::Board;
//!
//! let mut board = Board::new();
//!
//! // Parse and make a UCI move
//! board.make_move_uci("e2e4").unwrap();
//! board.make_move_uci("e7e5").unwrap();
//!
//! // Check game state
//! assert!(!board.is_checkmate());
//! assert!(!board.is_stalemate());
//! ```
//!
//! # Features
//!
//! - `serde` - Enable serialization for `Piece`, `Color`, `Square`, `Move`, and `CastlingRights`
//! - `logging` - Enable optional debug logging via the `log` crate

// Enable pedantic lints with sensible domain-specific exceptions
#![warn(clippy::pedantic)]
// Bitboard hex literals are clearer without separators (bit patterns visible)
#![allow(clippy::unreadable_literal)]
// Chess engines have intentionally similar names (eval_mg/eval_eg, etc.)
#![allow(clippy::similar_names)]
// Index casts are ubiquitous and safe in chess (board indices, square indices)
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
// Large arrays are needed for move lists and attack tables
#![allow(clippy::large_stack_arrays)]
// Module-level documentation is sufficient for this codebase
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod board;
pub mod engine;
pub mod sync;
pub mod timer;
pub mod tt;
pub mod uci;
pub mod xboard;
pub mod zobrist;
