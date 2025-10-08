use crate::transposition::transposition_table::{TranspositionTable, BoundType};
use crate::core::types::{Move, MoveList};
use crate::core::board::Board;
use crate::movegen::ordering::OrderingContext;
use crate::core::config::evaluation::MATE_SCORE;

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
    tt: &mut TranspositionTable,
    depth: u32,
    hash_move: Option<Move>,
    current_hash: u64,
    moves_buf: &mut MoveList,
    ctx: &mut OrderingContext,
    ply: usize,
) -> u32 {
    if depth < 8 || hash_move.is_none() {
        return 0;
    }

    // Check if we have a TT entry that qualifies for singular extension
    if tt.probe(current_hash).is_none_or(|e| e.depth < depth - 3 || e.bound_type != BoundType::LowerBound) {
        return 0;
    }

    let tt_entry = tt.probe(current_hash).unwrap();
    let singular_beta = tt_entry.score - 50; // margin for singularity
    let mut excluded_score = -MATE_SCORE;

    // Search all moves except the TT move
    moves_buf.clear();
    board.generate_moves_into(moves_buf);
    crate::movegen::ordering::order_moves(ctx, board, &mut moves_buf[..], depth as usize, hash_move);

    let mut child_buf = std::mem::take(&mut ctx.child_buf);
    child_buf.clear();

    for m in moves_buf.iter().filter(|m| Some(**m) != hash_move) {
        let info = board.make_move(m);
        let score = -crate::search::algorithms::negamax(board, tt, depth - 3, -singular_beta - 1, -singular_beta, &mut child_buf, ctx, ply + 1);
        board.unmake_move(m, info);

        excluded_score = excluded_score.max(score);
        if excluded_score >= singular_beta {
            ctx.child_buf = child_buf;
            return 0; // Found a good alternative, no extension
        }
    }

    ctx.child_buf = child_buf;

    if excluded_score < singular_beta {
        1 // Extend this singular move
    } else {
        0
    }
}