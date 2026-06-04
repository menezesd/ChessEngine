//! Pre-computed bitboard masks for evaluation.
//!
//! Contains masks for pawn structure evaluation, king safety zones,
//! and attack unit conversion tables.

mod files;
mod king;
mod pawns;

pub use files::{ADJACENT_FILES, FILES};
#[allow(unused_imports)]
pub use king::KING_ZONE;
pub use king::{KING_ATTACK_TABLE, KING_ZONE_EXTENDED, PAWN_SHIELD_MASK, RANK_7TH};
pub use pawns::{
    fill_backward, fill_forward, relative_rank, PASSED_PAWN_BONUS_EG, PASSED_PAWN_BONUS_MG,
    PASSED_PAWN_MASK, PAWN_SUPPORT_MASK,
};
#[allow(unused_imports)]
pub use pawns::{fill_north, fill_south};

#[cfg(test)]
mod tests;
