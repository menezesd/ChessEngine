// Crate root - export modules
pub mod core;
pub mod evaluation;
pub mod magic;
pub mod movegen;
pub mod transposition;
pub mod search;
pub mod uci;
pub mod engine;
pub mod perft;
pub mod tablebase;

// Re-export commonly used types for convenience
pub use core::{Board, Move, Piece, Color, Square, MoveList, Bitboard, FenError};
pub use evaluation::*;
pub use magic::*;
pub use movegen::*;
pub use transposition::*;
pub use search::*;
pub use uci::*;
