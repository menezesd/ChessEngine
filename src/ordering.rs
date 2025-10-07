use crate::types::Piece;

fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000,
    }
}

/// Compute an MVV-LVA move ordering score from optional victim and attacker piece types.
///
/// Returns a higher score for captures that take a higher-value victim with a
/// lower-value attacker. Used by the move ordering heuristics.
pub fn mvv_lva_score_by_values(victim: Option<Piece>, attacker: Option<Piece>) -> i32 {
    if let Some(v) = victim {
        if let Some(a) = attacker {
            let victim_value = piece_value(v);
            let attacker_value = piece_value(a);
            return victim_value * 10 - attacker_value;
        }
        // If attacker unknown, just prioritize victim
        piece_value(v) * 10
    } else {
        0
    }
}

use crate::types::Move;
use crate::board::Board;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

static ORDERING_ENABLED: AtomicBool = AtomicBool::new(true);

/// Globally enable or disable move ordering heuristics. When disabled, `order_moves`
/// will be a no-op (moves remain in their original order).
pub fn set_ordering_enabled(enabled: bool) {
    ORDERING_ENABLED.store(enabled, AtomicOrdering::SeqCst);
}

pub fn ordering_enabled() -> bool {
    ORDERING_ENABLED.load(AtomicOrdering::SeqCst)
}

// History table dimensions: piece (6) x from_sq (64) x to_sq (64) = 24576 entries
const HISTORY_PIECES: usize = 6;
const HISTORY_SQUARES: usize = 64;
const HISTORY_SIZE: usize = HISTORY_PIECES * HISTORY_SQUARES * HISTORY_SQUARES;

/// Small ordering context holding per-depth killer moves and a global history table.
///
/// Create with `OrderingContext::new(max_depth)` and pass mutable reference to
/// search functions to accumulate ordering heuristics across searches.
pub struct OrderingContext {
    /// killer_table[depth][slot]
    pub killers: Vec<[Option<Move>; 2]>,
    /// history indexed by piece/from/to -> score
    pub history: Vec<i32>,
    pub max_depth: usize,
}

impl OrderingContext {
    pub fn new(max_depth: usize) -> Self {
        Self {
            killers: vec![[None, None]; max_depth + 1],
            history: vec![0i32; HISTORY_SIZE],
            max_depth,
        }
    }

    pub fn record_killer(&mut self, depth: usize, m: Move) {
        if depth > self.max_depth {
            return;
        }
        let slot = &mut self.killers[depth];
        // If the move is already killer, keep order. Otherwise replace second slot then rotate.
        if slot[0].is_none() {
            slot[0] = Some(m);
        } else if slot[0] != Some(m) {
            slot[1] = slot[0];
            slot[0] = Some(m);
        }
    }

    pub fn record_history(&mut self, piece: Piece, from: u8, to: u8, delta: i32) {
        let p_idx = match piece {
            crate::types::Piece::Pawn => 0usize,
            crate::types::Piece::Knight => 1usize,
            crate::types::Piece::Bishop => 2usize,
            crate::types::Piece::Rook => 3usize,
            crate::types::Piece::Queen => 4usize,
            crate::types::Piece::King => 5usize,
        };
        let idx = p_idx * HISTORY_SQUARES * HISTORY_SQUARES + (from as usize) * HISTORY_SQUARES + (to as usize);
        if idx < self.history.len() {
            self.history[idx] = self.history[idx].saturating_add(delta);
        }
    }
}

/// Assigns ordering scores to moves and sorts `moves` in-place.
///
/// The `tt_move` (transposition-table suggested move) is given highest priority
/// and placed first if present. `ctx` supplies killer/history heuristics.
pub fn order_moves(
    ctx: &mut OrderingContext,
    board: &Board,
    moves: &mut [Move],
    depth: usize,
    tt_move: Option<Move>,
) {
    if !ordering_enabled() {
        // Leave moves as-is when ordering is disabled
        return;
    }
    // Prepare killers for this depth
    let killers = if depth <= ctx.max_depth { Some(ctx.killers[depth]) } else { None };

    moves.sort_by_key(|m| {
        // Highest scores first, so negate for sort_by_key ascending
        let mut score = 0i32;
        // TT move gets very high priority
        if let Some(tm) = tt_move {
            if *m == tm {
                return -1_000_000_000i32;
            }
        }
        // Captures: MVV-LVA using victim and attacker
        if let Some(victim) = m.captured_piece {
            let attacker = board.piece_at(m.from).map(|(_c, p)| p);
            score += mvv_lva_score_by_values(Some(victim), attacker);
        }
        // Killer moves
        if let Some(k) = killers {
            if k[0] == Some(*m) {
                score += 5000;
            } else if k[1] == Some(*m) {
                score += 4000;
            }
        }
        // History heuristic
        if let Some((_p, from_sq, to_sq)) = board.piece_at(m.from).map(|(_c, p)| (p, m.from.0 as u8, m.to.0 as u8)) {
            // compute index
            let p_idx = board.piece_at(m.from).map(|(_c, p)| match p {
                crate::types::Piece::Pawn => 0usize,
                crate::types::Piece::Knight => 1usize,
                crate::types::Piece::Bishop => 2usize,
                crate::types::Piece::Rook => 3usize,
                crate::types::Piece::Queen => 4usize,
                crate::types::Piece::King => 5usize,
            }).unwrap_or(0usize);
            let idx = p_idx * HISTORY_SQUARES * HISTORY_SQUARES + (from_sq as usize) * HISTORY_SQUARES + (to_sq as usize);
            if idx < ctx.history.len() {
                score += ctx.history[idx];
            }
        }
        -score
    });
}


