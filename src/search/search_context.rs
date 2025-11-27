use crate::transposition::transposition_table::TranspositionTable;
use crate::core::types::MoveList;
use crate::movegen::ordering::OrderingContext;
use crate::evaluation::pawn_hash::PawnHashTable;

pub struct SearchContext<'a> {
    pub tt: &'a mut TranspositionTable,
    pub moves_buf: &'a mut MoveList,
    pub ordering_ctx: &'a mut OrderingContext,
    pub ply: usize,
    pub pawn_hash_table: &'a mut PawnHashTable,
}
