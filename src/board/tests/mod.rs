//! Board module tests.
//!
//! Tests are organized into separate files by category:
//! - `perft.rs` - Performance tests for move generation
//! - `draw.rs` - Draw detection (50-move, repetition, insufficient material)
//! - `make_unmake.rs` - Make/unmake move correctness
//! - `edge_cases.rs` - Special positions and edge cases
//! - `proptest.rs` - Property-based tests

mod draw;
mod edge_cases;
mod make_unmake;
mod perft;
mod proptest;
