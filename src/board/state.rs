use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::{Bitboard, Color, Piece, Square, ALL_CASTLING_RIGHTS};

mod repetition;

use repetition::RepetitionTable;

#[derive(Clone, Copy, Debug)]
pub struct UnmakeInfo {
    pub(crate) captured_piece_info: Option<(Color, Piece)>,
    pub(crate) previous_en_passant_target: Option<Square>,
    pub(crate) previous_castling_rights: u8,
    pub(crate) previous_hash: u64,
    pub(crate) previous_halfmove_clock: u32,
    pub(crate) previous_fullmove_number: u32,
    pub(crate) made_hash: u64,
    pub(crate) previous_repetition_count: u32,
    // Incremental eval state (for restoration)
    pub(crate) previous_eval_mg: [i32; 2],
    pub(crate) previous_eval_eg: [i32; 2],
    pub(crate) previous_game_phase: [i32; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct NullMoveInfo {
    pub(crate) previous_en_passant_target: Option<Square>,
    pub(crate) previous_hash: u64,
    pub(crate) fullmove_number: u32,
}

#[derive(Clone, Debug)]
pub struct Board {
    pub(crate) pieces: [[Bitboard; 6]; 2],
    pub(crate) occupied: [Bitboard; 2],
    pub(crate) all_occupied: Bitboard,
    pub(crate) white_to_move: bool,
    pub(crate) en_passant_target: Option<Square>,
    pub(crate) castling_rights: u8, // bitmask
    pub(crate) hash: u64,           // Zobrist hash
    pub(crate) halfmove_clock: u32,
    pub(crate) fullmove_number: u32,
    pub(crate) repetition_counts: RepetitionTable,
    // Incremental evaluation scores
    pub(crate) eval_mg: [i32; 2],    // [white, black] middlegame scores
    pub(crate) eval_eg: [i32; 2],    // [white, black] endgame scores
    pub(crate) game_phase: [i32; 2], // [white, black] phase contribution
    // Cached king squares for fast check detection [white, black]
    pub(crate) king_square: [Square; 2],
    // Mailbox for O(1) piece_at lookups (mirrors bitboard state)
    pub(crate) mailbox: [Option<(Color, Piece)>; 64],
}

impl Board {
    fn add_piece_to_eval(&mut self, sq: Square, color: Color, piece: Piece) {
        let mut eval = self.eval_state();
        eval.add_piece(color.index(), piece, sq.index(), color == Color::White);
        self.set_eval_state(eval);
    }

    fn remove_piece_from_eval(&mut self, sq: Square, color: Color, piece: Piece) {
        let mut eval = self.eval_state();
        eval.remove_piece(color.index(), piece, sq.index(), color == Color::White);
        self.set_eval_state(eval);
    }

    fn clear_incremental_eval(&mut self) {
        self.eval_mg = [0, 0];
        self.eval_eg = [0, 0];
        self.game_phase = [0, 0];
    }

    #[must_use]
    pub fn new() -> Self {
        let mut board = Board::empty();
        let back_rank = [
            Piece::Rook,
            Piece::Knight,
            Piece::Bishop,
            Piece::Queen,
            Piece::King,
            Piece::Bishop,
            Piece::Knight,
            Piece::Rook,
        ];
        for (i, piece) in back_rank.iter().enumerate() {
            board.set_piece(Square::new(0, i), Color::White, *piece);
            board.set_piece(Square::new(7, i), Color::Black, *piece);
            board.set_piece(Square::new(1, i), Color::White, Piece::Pawn);
            board.set_piece(Square::new(6, i), Color::Black, Piece::Pawn);
        }

        board.castling_rights = ALL_CASTLING_RIGHTS;
        board.white_to_move = true;
        board.hash = board.calculate_initial_hash();
        board.repetition_counts.set(board.hash, 1);
        board.recalculate_incremental_eval();
        board
    }

    /// Clear all pieces from the board (for edit mode)
    pub fn clear(&mut self) {
        self.pieces = [[Bitboard(0); 6]; 2];
        self.occupied = [Bitboard(0); 2];
        self.all_occupied = Bitboard(0);
        self.white_to_move = true;
        self.castling_rights = 0;
        self.en_passant_target = None;
        self.halfmove_clock = 0;
        self.fullmove_number = 1;
        self.clear_incremental_eval();
        self.hash = 0;
        self.mailbox = [None; 64];
        // Keep the cached coordinates consistent with `Board::empty()` while
        // an editor rebuilds a position.  Leaving the previous king squares
        // here can make check/evaluation helpers observe stale coordinates.
        self.king_square = [Square::new(0, 4), Square::new(7, 4)];
        self.reset_repetition_history();
    }

    /// Start a new repetition history at the current position.
    ///
    /// Position editors mutate a board outside normal make/unmake flow, so
    /// their completed position must not inherit repetition entries from the
    /// game that was edited.
    pub(crate) fn reset_repetition_history(&mut self) {
        self.repetition_counts.clear();
        self.repetition_counts.set(self.hash, 1);
    }

    /// Flip the side to move (for edit mode)
    pub fn flip_side_to_move(&mut self) {
        use crate::zobrist::ZOBRIST;
        self.white_to_move = !self.white_to_move;
        self.hash ^= ZOBRIST.black_to_move_key;
        // This operation is used by position editors rather than normal move
        // play. The edited position must not inherit repetition counts from
        // the opposite side-to-move state.
        self.reset_repetition_history();
    }

    /// Resync the king-square cache after removing a King through the
    /// single-square editing API.
    ///
    /// `remove_piece` clears the bitboard/mailbox but, unlike `set_piece`,
    /// never touches `king_square` -- it is a hot path shared with
    /// `make_move`/`unmake_move`, where a King relocation always pairs a
    /// `remove_piece` with a `set_piece` that fixes the cache back up
    /// within the same operation (castling unmake even orders them
    /// `set_piece` then `remove_piece`, so resetting unconditionally on
    /// every King removal would clobber the value it just restored).  The
    /// editing API has no such pairing guarantee -- a caller can remove a
    /// King and never place one, or overwrite its square with an unrelated
    /// piece afterward -- so it must resync explicitly instead.
    fn sync_king_square_after_removal(&mut self, color: Color, removed_piece: Piece) {
        if removed_piece != Piece::King {
            return;
        }
        let c_idx = color.index();
        // Falls back to the standard back-rank square, matching `clear()`,
        // when no King of this color remains on the board.
        let default_sq = if color == Color::White {
            Square::new(0, 4)
        } else {
            Square::new(7, 4)
        };
        self.king_square[c_idx] = self
            .pieces_of(color, Piece::King)
            .iter()
            .next()
            .unwrap_or(default_sq);
    }

    /// Place a piece on the board (for edit mode)
    /// This updates bitboards, hash, and incremental eval
    pub fn place_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        use crate::zobrist::ZOBRIST;

        // First remove any existing piece at this square
        if let Some((old_color, old_piece)) = self.piece_at(sq) {
            self.remove_piece(sq, old_color, old_piece);
            self.hash ^= ZOBRIST.piece_keys[old_piece.index()][old_color.index()][sq.index()];
            self.remove_piece_from_eval(sq, old_color, old_piece);
            self.sync_king_square_after_removal(old_color, old_piece);
        }

        // Now add the new piece
        self.set_piece(sq, color, piece);
        self.hash ^= ZOBRIST.piece_keys[piece.index()][color.index()][sq.index()];
        self.add_piece_to_eval(sq, color, piece);
        self.reset_repetition_history();
    }

    /// Remove a piece from the board by square (for edit mode)
    /// This updates bitboards, hash, and incremental eval
    pub fn remove_piece_at(&mut self, sq: Square) {
        use crate::zobrist::ZOBRIST;

        if let Some((color, piece)) = self.piece_at(sq) {
            self.remove_piece(sq, color, piece);
            self.hash ^= ZOBRIST.piece_keys[piece.index()][color.index()][sq.index()];
            self.remove_piece_from_eval(sq, color, piece);
            self.sync_king_square_after_removal(color, piece);
            self.reset_repetition_history();
        }
    }

    /// Recalculate incremental evaluation from scratch (used after FEN parsing or initialization)
    pub(crate) fn recalculate_incremental_eval(&mut self) {
        self.clear_incremental_eval();

        for color in Color::BOTH {
            let c_idx = color.index();
            for piece in Piece::ALL {
                let p_idx = piece.index();
                for sq_idx in self.pieces_of(color, piece).iter() {
                    let sq = sq_idx.index();
                    // PST square: flip for white (tables are from black's perspective)
                    let pst_sq = if color == Color::White {
                        sq
                    } else {
                        sq ^ 0b11_1000
                    };

                    self.eval_mg[c_idx] += MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
                    self.eval_eg[c_idx] += MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
                    self.game_phase[c_idx] += PHASE_WEIGHTS[p_idx];
                }
            }
        }
    }

    pub(crate) fn empty() -> Self {
        Board {
            pieces: [[Bitboard(0); 6]; 2],
            occupied: [Bitboard(0); 2],
            all_occupied: Bitboard(0),
            white_to_move: true,
            en_passant_target: None,
            castling_rights: 0,
            hash: 0,
            halfmove_clock: 0,
            fullmove_number: 1,
            repetition_counts: RepetitionTable::new(),
            eval_mg: [0, 0],
            eval_eg: [0, 0],
            game_phase: [0, 0],
            // Will be set when kings are placed
            king_square: [Square::new(0, 4), Square::new(7, 4)],
            mailbox: [None; 64],
        }
    }

    #[must_use]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Compute a Zobrist hash of only the pawn positions.
    /// Used for pawn hash table lookups and correction history.
    #[must_use]
    pub fn pawn_hash(&self) -> u64 {
        use crate::zobrist::ZOBRIST;

        let mut hash = 0u64;

        for color in Color::BOTH {
            for sq in self.pieces_of(color, Piece::Pawn).iter() {
                hash ^= ZOBRIST.piece_keys[Piece::Pawn.index()][color.index()][sq.index()];
            }
        }

        hash
    }

    #[must_use]
    pub fn white_to_move(&self) -> bool {
        self.white_to_move
    }

    /// Get the side to move
    #[must_use]
    pub fn side_to_move(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    /// Get all pieces of a given type regardless of color
    #[must_use]
    pub fn all_pieces_of_type(&self, piece: Piece) -> Bitboard {
        Bitboard(self.pieces[0][piece.index()].0 | self.pieces[1][piece.index()].0)
    }

    /// Get the king square index (0-63) for a color
    #[inline]
    #[must_use]
    pub fn king_square_index(&self, color: Color) -> usize {
        self.king_square[color.index()].index()
    }

    #[must_use]
    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    /// FEN fullmove number (starts at one and increments after Black moves).
    #[must_use]
    pub fn fullmove_number(&self) -> u32 {
        self.fullmove_number
    }

    /// Count pieces of a given type for a color
    pub(crate) fn piece_count(&self, color: Color, piece: Piece) -> u32 {
        self.pieces_of(color, piece).popcount()
    }
}

#[cfg(test)]
mod tests {
    use super::{Board, Color, Piece, Square};

    #[test]
    fn removing_a_relocated_king_does_not_leave_a_stale_square() {
        // Move White's king away from its default square first, the way an
        // edit-mode GUI would when correcting a position: king on g1, not e1.
        let mut board = Board::empty();
        board.place_piece(Square::new(0, 6), Color::White, Piece::King);
        board.place_piece(Square::new(7, 4), Color::Black, Piece::King);
        assert_eq!(board.find_king(Color::White), Square::new(0, 6));

        // Remove the king without immediately replacing it, then place an
        // unrelated piece on the now-vacated g1. Before this fix,
        // king_square[White] stayed at g1, so find_king would return a
        // square that now holds a Black rook instead of White's king.
        board.remove_piece_at(Square::new(0, 6));
        board.place_piece(Square::new(0, 6), Color::Black, Piece::Rook);

        let cached = board.find_king(Color::White);
        assert_ne!(
            cached,
            Square::new(0, 6),
            "king_square must not still point at the square now holding a Black rook"
        );
        assert_ne!(
            board.piece_at(cached),
            Some((Color::Black, Piece::Rook)),
            "find_king(White) must never resolve to a piece_at() that isn't White's king"
        );
    }

    #[test]
    fn relocating_a_king_via_remove_then_place_updates_the_cache() {
        let mut board = Board::empty();
        board.place_piece(Square::new(0, 4), Color::White, Piece::King);
        board.place_piece(Square::new(7, 4), Color::Black, Piece::King);

        board.remove_piece_at(Square::new(0, 4));
        board.place_piece(Square::new(0, 2), Color::White, Piece::King);

        assert_eq!(board.find_king(Color::White), Square::new(0, 2));
        assert_eq!(
            board.piece_at(board.find_king(Color::White)),
            Some((Color::White, Piece::King))
        );
    }

    #[test]
    fn overwriting_a_king_square_with_a_new_piece_resyncs_the_cache() {
        // place_piece's "remove the old occupant" branch is exercised when a
        // king square is directly overwritten rather than cleared first.
        let mut board = Board::empty();
        board.place_piece(Square::new(0, 6), Color::White, Piece::King);
        board.place_piece(Square::new(7, 4), Color::Black, Piece::King);

        board.place_piece(Square::new(0, 6), Color::Black, Piece::Queen);

        assert_ne!(board.find_king(Color::White), Square::new(0, 6));
        assert_ne!(
            board.piece_at(board.find_king(Color::White)),
            Some((Color::Black, Piece::Queen))
        );
    }
}
