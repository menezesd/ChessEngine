//! Move types and move list.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::piece::Piece;
use super::square::Square;

mod flags;
mod list;
mod scored;

pub use list::{MoveList, MoveListIntoIter};
#[allow(unused_imports)]
pub use scored::ScoredMove;
pub use scored::ScoredMoveList;

const SQUARE_MASK: u16 = 0x3F;
const TO_SHIFT: u16 = 6;
const FLAG_SHIFT: u16 = 12;
const HISTORY_INDEX_STRIDE: usize = 64;

/// Compact 16-bit move representation.
///
/// Encoding:
/// - bits 0-5:   from square (0-63)
/// - bits 6-11:  to square (0-63)
/// - bits 12-15: flags (move type)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Move(u16);

impl Move {
    /// Create a null/empty move (used for initialization)
    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Move(0)
    }

    /// Create a quiet move (no capture, no special flags)
    #[inline]
    #[must_use]
    pub const fn quiet(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, flags::QUIET)
    }

    /// Create a capture move
    #[inline]
    #[must_use]
    pub const fn capture(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, flags::CAPTURE)
    }

    /// Create a double pawn push move
    #[inline]
    #[must_use]
    pub const fn double_pawn_push(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, flags::DOUBLE_PAWN)
    }

    /// Create an en passant capture
    #[inline]
    #[must_use]
    pub const fn en_passant(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, flags::EN_PASSANT)
    }

    /// Create a kingside castle move
    #[inline]
    #[must_use]
    pub const fn castle_kingside(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, flags::CASTLE_KINGSIDE)
    }

    /// Create a queenside castle move
    #[inline]
    #[must_use]
    pub const fn castle_queenside(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, flags::CASTLE_QUEENSIDE)
    }

    /// Create a promotion move (non-capture)
    #[inline]
    #[must_use]
    pub const fn new_promotion(from: Square, to: Square, piece: Piece) -> Self {
        Move::with_flag(from, to, flags::promotion(piece))
    }

    /// Create a promotion capture move
    #[inline]
    #[must_use]
    pub const fn new_promotion_capture(from: Square, to: Square, piece: Piece) -> Self {
        Move::with_flag(from, to, flags::promotion_capture(piece))
    }

    /// Create a move with a specific flag
    #[inline]
    const fn with_flag(from: Square, to: Square, flag: u16) -> Self {
        let from_idx = from.as_index() as u16;
        let to_idx = to.as_index() as u16;
        Move(from_idx | (to_idx << TO_SHIFT) | (flag << FLAG_SHIFT))
    }

    /// Get the source square
    #[inline]
    #[must_use]
    pub const fn from(self) -> Square {
        let idx = (self.0 & SQUARE_MASK) as usize;
        Square::from_index(idx)
    }

    /// Get the destination square
    #[inline]
    #[must_use]
    pub const fn to(self) -> Square {
        let idx = ((self.0 >> TO_SHIFT) & SQUARE_MASK) as usize;
        Square::from_index(idx)
    }

    /// Get the history table index for this move (from * 64 + to)
    #[inline]
    #[must_use]
    pub const fn history_index(self) -> usize {
        let from = (self.0 & SQUARE_MASK) as usize;
        let to = ((self.0 >> TO_SHIFT) & SQUARE_MASK) as usize;
        from * HISTORY_INDEX_STRIDE + to
    }

    /// Get the flag bits
    #[inline]
    const fn flag(self) -> u16 {
        self.0 >> FLAG_SHIFT
    }

    /// Returns true if this move captures a piece (including en passant)
    #[inline]
    #[must_use]
    pub const fn is_capture(self) -> bool {
        let f = self.flag();
        f == flags::CAPTURE || f == flags::EN_PASSANT || f >= flags::PROMO_CAPTURE_KNIGHT
    }

    /// Returns true if this move is en passant
    #[inline]
    #[must_use]
    pub const fn is_en_passant(self) -> bool {
        self.flag() == flags::EN_PASSANT
    }

    /// Returns true if this move is castling (kingside or queenside)
    #[inline]
    #[must_use]
    pub const fn is_castling(self) -> bool {
        let f = self.flag();
        f == flags::CASTLE_KINGSIDE || f == flags::CASTLE_QUEENSIDE
    }

    /// Returns true if this is kingside castling (O-O)
    #[inline]
    #[must_use]
    pub const fn is_castle_kingside(self) -> bool {
        self.flag() == flags::CASTLE_KINGSIDE
    }

    /// Returns true if this is queenside castling (O-O-O)
    #[inline]
    #[must_use]
    pub const fn is_castle_queenside(self) -> bool {
        self.flag() == flags::CASTLE_QUEENSIDE
    }

    /// Returns true if this move is a double pawn push
    #[inline]
    #[must_use]
    pub const fn is_double_pawn_push(self) -> bool {
        self.flag() == flags::DOUBLE_PAWN
    }

    /// Returns true if this move is a pawn promotion
    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.flag() >= flags::PROMO_KNIGHT
    }

    /// Get the promotion piece, if this is a promotion move
    #[inline]
    #[must_use]
    pub const fn promotion(self) -> Option<Piece> {
        flags::promotion_piece(self.flag())
    }

    /// Returns true if this move is "quiet" (not a capture, promotion, or special move)
    #[inline]
    #[must_use]
    pub const fn is_quiet(self) -> bool {
        let f = self.flag();
        f == flags::QUIET || f == flags::DOUBLE_PAWN
    }

    /// Returns true if this move is tactical (capture or promotion)
    #[inline]
    #[must_use]
    pub const fn is_tactical(self) -> bool {
        self.is_capture() || self.is_promotion()
    }

    /// Get the raw 16-bit value (for hashing/storage)
    #[inline]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Create from raw 16-bit value
    #[inline]
    #[must_use]
    pub const fn from_u16(value: u16) -> Self {
        Move(value)
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move({}{}", self.from(), self.to())?;
        if let Some(promo) = self.promotion() {
            write!(f, "={}", promo.to_char().to_ascii_uppercase())?;
        }
        if self.is_capture() {
            write!(f, " cap")?;
        }
        if self.is_castling() {
            write!(f, " castle")?;
        }
        if self.is_en_passant() {
            write!(f, " ep")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from(), self.to())?;
        if let Some(promo) = self.promotion() {
            write!(f, "{}", promo.to_char())?;
        }
        Ok(())
    }
}

/// Bound on legal moves in any position, with headroom above the
/// documented worst case. A single legal position is known to reach 271
/// legal moves ("QQQQQQBk/Q5RB/Q6Q/Q6Q/Q6Q/Q6Q/Q6Q/KQQQQQQQ w - - 0 1"),
/// far beyond the ~218-move ceiling for positions reachable from a real
/// game; 256 was not enough. `MoveList::push` also bounds-checks so any
/// future record, or an outright malformed FEN, degrades instead of
/// panicking.
pub(crate) const MAX_MOVES: usize = 512;
pub(crate) const MAX_PLY: usize = 128;
pub(crate) const EMPTY_MOVE: Move = Move::null();
