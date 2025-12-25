mod attack_tables;
mod pst;
#[cfg(debug_assertions)]
mod debug;
mod eval;
mod fen;
mod make_unmake;
mod movegen;
mod search;
mod state;
mod types;

#[cfg(test)]
mod tests;

pub use search::{
    find_best_move, find_best_move_with_time, Position, SearchClock, SearchContext, SearchLimits,
    SearchParams, SearchState, SearchStats, SearchTables, TerminalState, DEFAULT_TT_MB,
};
pub use state::{Board, NullMoveInfo, UnmakeInfo};
pub use types::format_square;
pub use types::{Bitboard, Color, Move, MoveList, Piece, Square, SquareIdx};

pub(crate) use types::{
    bit_for_square, castle_bit, file_to_index, pop_lsb, rank_to_index,
    CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K,
    CASTLE_WHITE_Q, EMPTY_MOVE, MAX_PLY, PROMOTION_PIECES,
};
