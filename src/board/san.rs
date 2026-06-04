//! Standard Algebraic Notation (SAN) support.
//!
//! SAN is the standard human-readable chess notation used in scoresheets,
//! books, and GUIs. Examples: "e4", "Nf3", "Bxc6+", "O-O", "e8=Q#"
//!
//! # Examples
//! ```
//! use chess_engine::board::Board;
//!
//! let mut board = Board::new();
//! let mv = board.parse_san("e4").unwrap();
//! assert_eq!(board.move_to_san(&mv), "e4");
//! ```

mod format;
mod parse;

#[cfg(test)]
mod tests;
