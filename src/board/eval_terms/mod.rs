//! Advanced evaluation terms.
//!
//! Contains evaluation functions for:
//! - Mobility (piece movement options)
//! - Pawn structure (passed, doubled, isolated, backward pawns)
//! - King safety (attack units, pawn shield)
//! - Rook activity (open files, 7th rank)
//! - Hanging pieces
//! - Drawish endgame detection
//! - Piece coordination (batteries, clusters)
//! - Advanced pawn features (storm, levers, chains)
//! - Weak squares (holes, color complexes)
//! - King danger refinements
//! - Endgame patterns (fortress, wrong bishop)
//! - Space control
//! - Advanced threats (forks, pins, skewers)
//! - Piece quality (active/passive, trapped)
//! - Imbalances (knight vs bishop, etc.)
//! - Initiative

mod combined;
mod coordination;
mod drawish;
mod endgame_patterns;
mod hanging;
pub mod helpers;
mod imbalances;
mod initiative;
mod king_danger;
mod king_safety;
mod minor_pieces;
mod mobility;
mod passed_pawns;
mod pawn_advanced;
mod pawn_structure;
mod piece_quality;
mod rooks;
mod space_control;
pub mod tables;
mod threats_advanced;
mod tropism;
mod weak_squares;

#[cfg(test)]
mod tests;
