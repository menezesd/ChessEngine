//! Search rules for reusing and storing transposition table scores.

use super::{SimpleSearchContext, PV_EXTENSION_PLIES};
use crate::board::draw::FIFTY_MOVE_RULE_HALFMOVES;
use crate::board::search::constants::{
    DEFAULT_MAX_DEPTH, MATE_THRESHOLD, MAX_QSEARCH_DEPTH, QSEARCH_CHECK_EXTENSION,
};
use crate::board::{Move, EMPTY_MOVE, MAX_PLY};
use crate::tt::BoundType;

// Upper bound on plies reached by any normal root search. Even if every
// move is reversible, a clock below this cutoff cannot reach fifty moves.
pub(super) const TT_EARLY_CLOCK_LIMIT: u32 = FIFTY_MOVE_RULE_HALFMOVES
    - DEFAULT_MAX_DEPTH
    - PV_EXTENSION_PLIES
    - (MAX_QSEARCH_DEPTH + QSEARCH_CHECK_EXTENSION) as u32;

/// Convert a root-relative mate score into a TT-relative score.
///
/// The same position can be reached at a different ply through a
/// transposition. Storing mate scores without this adjustment makes a mate
/// found deeper in one line appear equally distant when probed from another.
#[inline]
pub(super) fn score_to_tt(score: i32, ply: usize) -> i32 {
    let ply_score = ply.min(MAX_PLY) as i32;
    if score >= MATE_THRESHOLD {
        score.saturating_add(ply_score)
    } else if score <= -MATE_THRESHOLD {
        score.saturating_sub(ply_score)
    } else {
        score
    }
}

/// Convert a TT-relative mate score back into a root-relative score.
#[inline]
pub(super) fn score_from_tt(score: i32, ply: usize) -> i32 {
    let ply_score = ply.min(MAX_PLY) as i32;
    if score >= MATE_THRESHOLD {
        score.saturating_sub(ply_score)
    } else if score <= -MATE_THRESHOLD {
        score.saturating_add(ply_score)
    } else {
        score
    }
}

pub(super) struct TtProbe {
    pub(super) best_move: Move,
    pub(super) score: i32,
    pub(super) bound: BoundType,
    pub(super) depth: u32,
    pub(super) cutoff: Option<i32>,
}

impl TtProbe {
    fn ordering_hint(best_move: Move) -> Self {
        Self {
            best_move,
            score: 0,
            bound: BoundType::Exact,
            depth: 0,
            cutoff: None,
        }
    }
}

impl SimpleSearchContext<'_> {
    /// Store position in transposition table
    pub(super) fn store_tt(
        &mut self,
        depth: u32,
        score: i32,
        bound: BoundType,
        best_move: Move,
        ply: usize,
    ) {
        if self.should_stop() || best_move == EMPTY_MOVE {
            return;
        }
        if ply == 0 {
            self.root_best_move = Some(best_move);
            // A score obtained with root moves excluded is not the value
            // of the unrestricted position stored under this board hash.
            if self.root_moves_restricted {
                return;
            }
        }
        self.state.tables.tt.store_with_halfmove_clock(
            self.board.hash,
            depth,
            score_to_tt(score, ply),
            bound,
            Some(best_move),
            self.state.generation,
            self.board.halfmove_clock(),
        );
    }

    pub(super) fn probe_tt(
        &self,
        depth: u32,
        alpha: i32,
        beta: i32,
        ply: usize,
        is_pv: bool,
        excluded_move_active: bool,
    ) -> TtProbe {
        let Some(entry) = self.state.tables.tt.probe(self.board.hash) else {
            return TtProbe::ordering_hint(EMPTY_MOVE);
        };
        let best_move = entry.best_move().unwrap_or(EMPTY_MOVE);
        let clock = self.board.halfmove_clock();
        if !entry.matches_halfmove_clock(clock)
            && clock.max(u32::from(entry.halfmove_clock)) >= TT_EARLY_CLOCK_LIMIT
        {
            // The score may depend on the fifty-move rule. The move still
            // gives an ordering hint after it is checked for legality.
            return TtProbe::ordering_hint(best_move);
        }

        let score = score_from_tt(entry.score(), ply);
        let bound = entry.bound_type();
        let can_cut = !excluded_move_active
            && entry.depth() >= depth
            && match bound {
                BoundType::Exact => !is_pv || (score > alpha && score < beta),
                BoundType::LowerBound => score >= beta,
                BoundType::UpperBound => score <= alpha,
            };
        TtProbe {
            best_move,
            score,
            bound,
            depth: entry.depth(),
            cutoff: can_cut.then_some(score),
        }
    }
}
