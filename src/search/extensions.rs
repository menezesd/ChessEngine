use crate::transposition::transposition_table::BoundType;
use crate::core::types::Move;
use crate::core::board::Board;
use crate::core::config::evaluation::MATE_SCORE;
use crate::search::search_context::SearchContext;

/// Extension types and their values
#[derive(Debug, Clone, Copy)]
pub struct Extensions {
    pub check: u32,
    pub singular: u32,
}

impl Extensions {
    pub fn new() -> Self {
        Self {
            check: 0,
            singular: 0,
        }
    }

    pub fn total(&self) -> u32 {
        self.check + self.singular
    }
}

/// Calculate check extension
pub fn check_extension(board: &Board, move_index: usize, depth: u32) -> u32 {
    let gives_check = board.is_in_check(board.current_color());
    if gives_check && (move_index == 0 || depth < 4) {
        1
    } else {
        0
    }
}

/// Calculate singular extension for a TT move
pub fn singular_extension(
    board: &mut Board,
    s_ctx: &mut SearchContext,
    depth: u32,
    hash_move: Option<Move>,
    current_hash: u64,
) -> u32 {
    if depth < 8 || hash_move.is_none() {
        return 0;
    }

    // Check if we have a TT entry that qualifies for singular extension
    let (_, _, _, _, tt_stored_score) = crate::search::move_selector::TranspositionTableHelper::probe_and_adjust_bounds(s_ctx.tt, current_hash, depth, 0, 0);

    if tt_stored_score.is_none() {
        return 0;
    }

    let tt_entry = s_ctx.tt.probe(current_hash).unwrap();
    if tt_entry.depth < depth - 3 || tt_entry.bound_type != BoundType::LowerBound {
        return 0;
    }
    let singular_beta = tt_entry.score - 50; // margin for singularity
    let mut excluded_score = -MATE_SCORE;

    // Search all moves except the TT move
    s_ctx.moves_buf.clear();
    board.generate_moves_into(s_ctx.moves_buf);
    crate::movegen::ordering::order_moves(s_ctx.ordering_ctx, board, &mut s_ctx.moves_buf[..], depth as usize, hash_move);

    let mut child_buf = std::mem::take(&mut s_ctx.ordering_ctx.child_buf);
    child_buf.clear();

    for m in s_ctx.moves_buf.iter().filter(|m| Some(**m) != hash_move) {
        let info = board.make_move(m);
        let mut child_s_ctx = SearchContext {
            tt: s_ctx.tt,
            moves_buf: &mut child_buf,
            ordering_ctx: s_ctx.ordering_ctx,
            ply: s_ctx.ply + 1,
            pawn_hash_table: s_ctx.pawn_hash_table,
        };
        let score = -crate::search::algorithms::negamax(board, &mut child_s_ctx, depth - 3, -singular_beta - 1, -singular_beta);
        board.unmake_move(m, info);

        excluded_score = excluded_score.max(score);
        if excluded_score >= singular_beta {
            s_ctx.ordering_ctx.child_buf = child_buf;
            return 0; // Found a good alternative, no extension
        }
    }

    s_ctx.ordering_ctx.child_buf = child_buf;

    if excluded_score < singular_beta {
        1 // Extend this singular move
    } else {
        0
    }
}

/// Calculate recapture extension
pub fn recapture_extension(board: &Board, m: &Move) -> u32 {
    if let Some(last_move) = board.last_move_made {
        if m.captured_piece.is_some() && last_move.captured_piece.is_some() && m.to == last_move.to {
            return 1;
        }
    }
    0
}