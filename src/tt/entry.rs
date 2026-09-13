use crate::board::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Exact,
    LowerBound,
    UpperBound,
}

impl BoundType {
    pub(super) fn to_u8(self) -> u8 {
        match self {
            BoundType::Exact => 0,
            BoundType::LowerBound => 1,
            BoundType::UpperBound => 2,
        }
    }

    pub(super) fn from_u8(v: u8) -> Self {
        match v & BOUND_MASK {
            0 => BoundType::Exact,
            1 => BoundType::LowerBound,
            _ => BoundType::UpperBound,
        }
    }
}

pub(super) const BOUND_MASK: u8 = 0x3;
pub(super) const GENERATION_MASK: u8 = 0x3F;
pub(super) const GENERATION_SHIFT: u8 = 2;

const MOVE_MASK: u64 = 0xFFFF;
const SCORE_SHIFT: usize = 16;
pub(super) const DEPTH_SHIFT: usize = 32;
pub(super) const BOUND_GEN_SHIFT: usize = 40;
pub(super) const BYTE_MASK: u64 = 0xFF;
const HALFMOVE_SHIFT: usize = 48;
const HALFMOVE_MASK: u64 = 0x7F;
pub(super) const MAX_HALFMOVE_CLOCK: u32 = 100;
// Keep the all-zero word reserved for an empty TT slot.  The packed payload
// itself can otherwise legitimately be zero (depth-zero exact draw with no
// best move and generation zero), so it needs an explicit validity marker.
const VALID_BIT: u64 = 1 << 63;

/// Unpacked TT entry for reading.
#[derive(Clone, Debug)]
pub struct TTEntry {
    pub depth: u8,
    pub score: i16,
    pub bound_type: BoundType,
    pub best_move: Option<Move>,
    pub generation: u8,
    /// Fifty-move counter at the searched position, capped at 100.
    pub halfmove_clock: u8,
}

impl TTEntry {
    #[must_use]
    pub fn depth(&self) -> u32 {
        self.depth as u32
    }

    #[must_use]
    pub fn score(&self) -> i32 {
        self.score as i32
    }

    #[must_use]
    pub fn bound_type(&self) -> BoundType {
        self.bound_type
    }

    #[must_use]
    pub fn best_move(&self) -> Option<Move> {
        self.best_move
    }

    #[must_use]
    pub fn matches_halfmove_clock(&self, halfmove_clock: u32) -> bool {
        u32::from(self.halfmove_clock) == halfmove_clock.min(MAX_HALFMOVE_CLOCK)
    }
}

/// Packed entry format (fits in 64 bits):
/// - bits 0-15:  move (u16, 0 = no move)
/// - bits 16-31: score (i16 as u16)
/// - bits 32-39: depth (u8)
/// - bits 40-47: bound (2 bits) + generation (6 bits)
/// - bits 48-54: fifty-move counter (0-100)
/// - bit 63: entry-valid marker (keeps the all-zero word as the empty sentinel)
///
/// Total: 55 payload bits plus one validity bit; 8 bits remain spare.
pub(super) fn pack_entry(
    depth: u8,
    score: i16,
    bound_type: BoundType,
    best_move: Option<Move>,
    generation: u8,
    halfmove_clock: u8,
) -> u64 {
    let mv: u16 = best_move.map_or(0, Move::as_u16);
    let sc: u16 = score as u16;
    let bound_gen: u8 =
        (bound_type.to_u8() & BOUND_MASK) | ((generation & GENERATION_MASK) << GENERATION_SHIFT);

    VALID_BIT
        | (mv as u64)
        | ((sc as u64) << SCORE_SHIFT)
        | ((depth as u64) << DEPTH_SHIFT)
        | ((bound_gen as u64) << BOUND_GEN_SHIFT)
        | ((u64::from(halfmove_clock) & HALFMOVE_MASK) << HALFMOVE_SHIFT)
}

pub(super) fn unpack_entry(data: u64) -> TTEntry {
    let mv_bits = (data & MOVE_MASK) as u16;
    let score = ((data >> SCORE_SHIFT) & MOVE_MASK) as i16;
    let depth = ((data >> DEPTH_SHIFT) & BYTE_MASK) as u8;
    let bound_gen = ((data >> BOUND_GEN_SHIFT) & BYTE_MASK) as u8;

    let bound_type = BoundType::from_u8(bound_gen & BOUND_MASK);
    let generation = (bound_gen >> GENERATION_SHIFT) & GENERATION_MASK;
    let halfmove_clock = ((data >> HALFMOVE_SHIFT) & HALFMOVE_MASK) as u8;

    let best_move = if mv_bits == 0 {
        None
    } else {
        Some(Move::from_u16(mv_bits))
    };

    TTEntry {
        depth,
        score,
        bound_type,
        best_move,
        generation,
        halfmove_clock,
    }
}

#[cfg(test)]
mod tests;
