//! Move types and move list.

use std::fmt;
use std::ops::Index;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::piece::Piece;
use super::square::Square;

// Move flags (4 bits, values 0-15)
const FLAG_QUIET: u16 = 0;
const FLAG_DOUBLE_PAWN: u16 = 1;
const FLAG_CASTLE_KINGSIDE: u16 = 2;
const FLAG_CASTLE_QUEENSIDE: u16 = 3;
const FLAG_CAPTURE: u16 = 4;
const FLAG_EN_PASSANT: u16 = 5;
// 6-7 reserved
const FLAG_PROMO_KNIGHT: u16 = 8;
const FLAG_PROMO_BISHOP: u16 = 9;
const FLAG_PROMO_ROOK: u16 = 10;
const FLAG_PROMO_QUEEN: u16 = 11;
const FLAG_PROMO_CAPTURE_KNIGHT: u16 = 12;
const FLAG_PROMO_CAPTURE_BISHOP: u16 = 13;
const FLAG_PROMO_CAPTURE_ROOK: u16 = 14;
const FLAG_PROMO_CAPTURE_QUEEN: u16 = 15;

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
        Move::with_flag(from, to, FLAG_QUIET)
    }

    /// Create a capture move
    #[inline]
    #[must_use]
    pub const fn capture(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_CAPTURE)
    }

    /// Create a double pawn push move
    #[inline]
    #[must_use]
    pub const fn double_pawn_push(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_DOUBLE_PAWN)
    }

    /// Create an en passant capture
    #[inline]
    #[must_use]
    pub const fn en_passant(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_EN_PASSANT)
    }

    /// Create a kingside castle move
    #[inline]
    #[must_use]
    pub const fn castle_kingside(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_CASTLE_KINGSIDE)
    }

    /// Create a queenside castle move
    #[inline]
    #[must_use]
    pub const fn castle_queenside(from: Square, to: Square) -> Self {
        Move::with_flag(from, to, FLAG_CASTLE_QUEENSIDE)
    }

    /// Create a promotion move (non-capture)
    #[inline]
    #[must_use]
    pub const fn new_promotion(from: Square, to: Square, piece: Piece) -> Self {
        let flag = match piece {
            Piece::Knight => FLAG_PROMO_KNIGHT,
            Piece::Bishop => FLAG_PROMO_BISHOP,
            Piece::Rook => FLAG_PROMO_ROOK,
            _ => FLAG_PROMO_QUEEN, // Default to queen for invalid pieces
        };
        Move::with_flag(from, to, flag)
    }

    /// Create a promotion capture move
    #[inline]
    #[must_use]
    pub const fn new_promotion_capture(from: Square, to: Square, piece: Piece) -> Self {
        let flag = match piece {
            Piece::Knight => FLAG_PROMO_CAPTURE_KNIGHT,
            Piece::Bishop => FLAG_PROMO_CAPTURE_BISHOP,
            Piece::Rook => FLAG_PROMO_CAPTURE_ROOK,
            _ => FLAG_PROMO_CAPTURE_QUEEN, // Default to queen for invalid pieces
        };
        Move::with_flag(from, to, flag)
    }

    /// Create a move with a specific flag
    #[inline]
    const fn with_flag(from: Square, to: Square, flag: u16) -> Self {
        let from_idx = from.as_index() as u16;
        let to_idx = to.as_index() as u16;
        Move(from_idx | (to_idx << 6) | (flag << 12))
    }

    /// Get the source square
    #[inline]
    #[must_use]
    pub const fn from(self) -> Square {
        let idx = (self.0 & 0x3F) as usize;
        Square::from_index(idx)
    }

    /// Get the destination square
    #[inline]
    #[must_use]
    pub const fn to(self) -> Square {
        let idx = ((self.0 >> 6) & 0x3F) as usize;
        Square::from_index(idx)
    }

    /// Get the flag bits
    #[inline]
    const fn flag(self) -> u16 {
        self.0 >> 12
    }

    /// Returns true if this move captures a piece (including en passant)
    #[inline]
    #[must_use]
    pub const fn is_capture(self) -> bool {
        let f = self.flag();
        f == FLAG_CAPTURE || f == FLAG_EN_PASSANT || f >= FLAG_PROMO_CAPTURE_KNIGHT
    }

    /// Returns true if this move is en passant
    #[inline]
    #[must_use]
    pub const fn is_en_passant(self) -> bool {
        self.flag() == FLAG_EN_PASSANT
    }

    /// Returns true if this move is castling (kingside or queenside)
    #[inline]
    #[must_use]
    pub const fn is_castling(self) -> bool {
        let f = self.flag();
        f == FLAG_CASTLE_KINGSIDE || f == FLAG_CASTLE_QUEENSIDE
    }

    /// Returns true if this is kingside castling (O-O)
    #[inline]
    #[must_use]
    pub const fn is_castle_kingside(self) -> bool {
        self.flag() == FLAG_CASTLE_KINGSIDE
    }

    /// Returns true if this is queenside castling (O-O-O)
    #[inline]
    #[must_use]
    pub const fn is_castle_queenside(self) -> bool {
        self.flag() == FLAG_CASTLE_QUEENSIDE
    }

    /// Returns true if this move is a double pawn push
    #[inline]
    #[must_use]
    pub const fn is_double_pawn_push(self) -> bool {
        self.flag() == FLAG_DOUBLE_PAWN
    }

    /// Returns true if this move is a pawn promotion
    #[inline]
    #[must_use]
    pub const fn is_promotion(self) -> bool {
        self.flag() >= FLAG_PROMO_KNIGHT
    }

    /// Get the promotion piece, if this is a promotion move
    #[inline]
    #[must_use]
    pub const fn promotion(self) -> Option<Piece> {
        match self.flag() {
            FLAG_PROMO_KNIGHT | FLAG_PROMO_CAPTURE_KNIGHT => Some(Piece::Knight),
            FLAG_PROMO_BISHOP | FLAG_PROMO_CAPTURE_BISHOP => Some(Piece::Bishop),
            FLAG_PROMO_ROOK | FLAG_PROMO_CAPTURE_ROOK => Some(Piece::Rook),
            FLAG_PROMO_QUEEN | FLAG_PROMO_CAPTURE_QUEEN => Some(Piece::Queen),
            _ => None,
        }
    }

    /// Returns true if this move is "quiet" (not a capture, promotion, or special move)
    #[inline]
    #[must_use]
    pub const fn is_quiet(self) -> bool {
        let f = self.flag();
        f == FLAG_QUIET || f == FLAG_DOUBLE_PAWN
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

pub(crate) const MAX_MOVES: usize = 256;
pub(crate) const MAX_PLY: usize = 128;
pub(crate) const EMPTY_MOVE: Move = Move::null();

/// List of moves with fixed-size backing array.
#[derive(Clone, Debug)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    pub(crate) fn new() -> Self {
        MoveList {
            moves: [EMPTY_MOVE; MAX_MOVES],
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, mv: Move) {
        self.moves[self.len] = mv;
        self.len += 1;
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub(crate) fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Move> {
        self.as_mut_slice().iter_mut()
    }

    #[must_use]
    pub fn get(&self, idx: usize) -> Option<Move> {
        if idx < self.len {
            Some(self.moves[idx])
        } else {
            None
        }
    }

    #[must_use]
    pub fn first(&self) -> Option<Move> {
        self.get(0)
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a mut MoveList {
    type Item = &'a mut Move;
    type IntoIter = std::slice::IterMut<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

impl Default for MoveList {
    fn default() -> Self {
        MoveList::new()
    }
}

/// Owning iterator over moves in a `MoveList`
pub struct MoveListIntoIter {
    list: MoveList,
    idx: usize,
}

impl Iterator for MoveListIntoIter {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.list.len {
            let mv = self.list.moves[self.idx];
            self.idx += 1;
            Some(mv)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.list.len - self.idx;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for MoveListIntoIter {}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = MoveListIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        MoveListIntoIter { list: self, idx: 0 }
    }
}

impl Index<usize> for MoveList {
    type Output = Move;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(
            idx < self.len,
            "MoveList index {} out of bounds (len {})",
            idx,
            self.len
        );
        &self.moves[idx]
    }
}

/// A scored move for move ordering.
#[derive(Clone, Copy, Debug)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// Fixed-size list of scored moves to avoid heap allocation.
#[derive(Clone, Debug)]
pub struct ScoredMoveList {
    moves: [ScoredMove; MAX_MOVES],
    len: usize,
}

impl ScoredMoveList {
    /// Create a new empty scored move list.
    #[must_use]
    pub fn new() -> Self {
        ScoredMoveList {
            moves: [ScoredMove {
                mv: EMPTY_MOVE,
                score: 0,
            }; MAX_MOVES],
            len: 0,
        }
    }

    /// Add a scored move to the list.
    #[inline]
    pub fn push(&mut self, mv: Move, score: i32) {
        self.moves[self.len] = ScoredMove { mv, score };
        self.len += 1;
    }

    /// Get the number of moves in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the list is empty.
    #[must_use]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a slice of the scored moves.
    #[must_use]
    pub fn as_slice(&self) -> &[ScoredMove] {
        &self.moves[..self.len]
    }

    /// Get a mutable slice of the scored moves.
    pub fn as_mut_slice(&mut self) -> &mut [ScoredMove] {
        &mut self.moves[..self.len]
    }

    /// Sort moves by score in descending order.
    pub fn sort_by_score_desc(&mut self) {
        self.as_mut_slice().sort_by(|a, b| b.score.cmp(&a.score));
    }

    /// Partial sort: find the best move from index `start` onwards and swap it to position `start`.
    /// Returns the move at position `start` after swapping (the best remaining move).
    /// This implements incremental selection sort - O(n-start) per call, but avoids sorting
    /// moves we'll never try due to early cutoffs.
    #[inline]
    pub fn pick_best(&mut self, start: usize) -> Option<&ScoredMove> {
        if start >= self.len {
            return None;
        }

        // Find index of best move from start onwards
        let mut best_idx = start;
        let mut best_score = self.moves[start].score;
        for i in (start + 1)..self.len {
            if self.moves[i].score > best_score {
                best_score = self.moves[i].score;
                best_idx = i;
            }
        }

        // Swap best to start position
        if best_idx != start {
            self.moves.swap(start, best_idx);
        }

        Some(&self.moves[start])
    }

    /// Iterate over scored moves.
    pub fn iter(&self) -> std::slice::Iter<'_, ScoredMove> {
        self.as_slice().iter()
    }
}

impl Default for ScoredMoveList {
    fn default() -> Self {
        ScoredMoveList::new()
    }
}
