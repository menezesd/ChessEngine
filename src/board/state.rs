use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use super::pst::{MATERIAL_EG, MATERIAL_MG, PHASE_WEIGHTS, PST_EG, PST_MG};
use super::{
    Bitboard, Color, Piece, Square, CASTLE_BLACK_K, CASTLE_BLACK_Q, CASTLE_WHITE_K, CASTLE_WHITE_Q,
};

#[derive(Clone, Copy, Debug)]
pub struct UnmakeInfo {
    pub(crate) captured_piece_info: Option<(Color, Piece)>,
    pub(crate) previous_en_passant_target: Option<Square>,
    pub(crate) previous_castling_rights: u8,
    pub(crate) previous_hash: u64,
    pub(crate) previous_halfmove_clock: u32,
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
}

#[derive(Clone, Debug)]
pub(crate) struct RepetitionTable {
    counts: HashMap<u64, u32>,
}

impl RepetitionTable {
    pub(crate) fn new() -> Self {
        RepetitionTable {
            counts: HashMap::new(),
        }
    }

    pub(crate) fn get(&self, hash: u64) -> u32 {
        self.counts.get(&hash).copied().unwrap_or(0)
    }

    pub(crate) fn set(&mut self, hash: u64, count: u32) {
        if count == 0 {
            self.counts.remove(&hash);
        } else {
            self.counts.insert(hash, count);
        }
    }

    pub(crate) fn increment(&mut self, hash: u64) -> u32 {
        let next = self.get(hash).saturating_add(1);
        self.set(hash, next);
        next
    }
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
    pub(crate) repetition_counts: RepetitionTable,
    // Incremental evaluation scores
    pub(crate) eval_mg: [i32; 2],    // [white, black] middlegame scores
    pub(crate) eval_eg: [i32; 2],    // [white, black] endgame scores
    pub(crate) game_phase: [i32; 2], // [white, black] phase contribution
}

impl Board {
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

        board.castling_rights = CASTLE_WHITE_K | CASTLE_WHITE_Q | CASTLE_BLACK_K | CASTLE_BLACK_Q;
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
        self.castling_rights = 0;
        self.en_passant_target = None;
        self.halfmove_clock = 0;
        self.eval_mg = [0, 0];
        self.eval_eg = [0, 0];
        self.game_phase = [0, 0];
        self.hash = 0;
    }

    /// Flip the side to move (for edit mode)
    pub fn flip_side_to_move(&mut self) {
        use crate::zobrist::ZOBRIST;
        self.white_to_move = !self.white_to_move;
        self.hash ^= ZOBRIST.black_to_move_key;
    }

    /// Place a piece on the board (for edit mode)
    /// This updates bitboards, hash, and incremental eval
    pub fn place_piece(&mut self, sq: Square, color: Color, piece: Piece) {
        use crate::zobrist::ZOBRIST;

        // First remove any existing piece at this square
        if let Some((old_color, old_piece)) = self.piece_at(sq) {
            self.remove_piece(sq, old_color, old_piece);
            self.hash ^= ZOBRIST.piece_keys[old_piece.index()][old_color.index()][sq.index()];
            // Update incremental eval for removed piece
            let c_idx = old_color.index();
            let p_idx = old_piece.index();
            let pst_sq = if old_color == Color::White {
                sq.index()
            } else {
                sq.index() ^ 56
            };
            self.eval_mg[c_idx] -= MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
            self.eval_eg[c_idx] -= MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
            self.game_phase[c_idx] -= PHASE_WEIGHTS[p_idx];
        }

        // Now add the new piece
        self.set_piece(sq, color, piece);
        self.hash ^= ZOBRIST.piece_keys[piece.index()][color.index()][sq.index()];

        // Update incremental eval
        let c_idx = color.index();
        let p_idx = piece.index();
        let pst_sq = if color == Color::White {
            sq.index()
        } else {
            sq.index() ^ 56
        };
        self.eval_mg[c_idx] += MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
        self.eval_eg[c_idx] += MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
        self.game_phase[c_idx] += PHASE_WEIGHTS[p_idx];
    }

    /// Remove a piece from the board by square (for edit mode)
    /// This updates bitboards, hash, and incremental eval
    pub fn remove_piece_at(&mut self, sq: Square) {
        use crate::zobrist::ZOBRIST;

        if let Some((color, piece)) = self.piece_at(sq) {
            self.remove_piece(sq, color, piece);
            self.hash ^= ZOBRIST.piece_keys[piece.index()][color.index()][sq.index()];

            // Update incremental eval
            let c_idx = color.index();
            let p_idx = piece.index();
            let pst_sq = if color == Color::White {
                sq.index()
            } else {
                sq.index() ^ 56
            };
            self.eval_mg[c_idx] -= MATERIAL_MG[p_idx] + PST_MG[p_idx][pst_sq];
            self.eval_eg[c_idx] -= MATERIAL_EG[p_idx] + PST_EG[p_idx][pst_sq];
            self.game_phase[c_idx] -= PHASE_WEIGHTS[p_idx];
        }
    }

    /// Recalculate incremental evaluation from scratch (used after FEN parsing or initialization)
    pub(crate) fn recalculate_incremental_eval(&mut self) {
        self.eval_mg = [0, 0];
        self.eval_eg = [0, 0];
        self.game_phase = [0, 0];

        for color in [Color::White, Color::Black] {
            let c_idx = color.index();
            for piece_type in 0..6 {
                for sq_idx in self.pieces[c_idx][piece_type].iter() {
                    let sq = sq_idx.index();
                    // PST square: flip for white (tables are from black's perspective)
                    let pst_sq = if color == Color::White { sq } else { sq ^ 56 };

                    self.eval_mg[c_idx] += MATERIAL_MG[piece_type] + PST_MG[piece_type][pst_sq];
                    self.eval_eg[c_idx] += MATERIAL_EG[piece_type] + PST_EG[piece_type][pst_sq];
                    self.game_phase[c_idx] += PHASE_WEIGHTS[piece_type];
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
            repetition_counts: RepetitionTable::new(),
            eval_mg: [0, 0],
            eval_eg: [0, 0],
            game_phase: [0, 0],
        }
    }

    #[must_use]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Compute a Zobrist hash of only the pawn positions.
    /// Used for pawn hash table lookups.
    #[must_use]
    pub fn pawn_hash(&self) -> u64 {
        use crate::zobrist::ZOBRIST;

        let mut hash = 0u64;

        // Hash white pawns
        for sq in self.pieces[0][Piece::Pawn.index()].iter() {
            hash ^= ZOBRIST.piece_keys[Piece::Pawn.index()][0][sq.index()];
        }

        // Hash black pawns
        for sq in self.pieces[1][Piece::Pawn.index()].iter() {
            hash ^= ZOBRIST.piece_keys[Piece::Pawn.index()][1][sq.index()];
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

    #[must_use]
    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    #[must_use]
    pub fn is_draw(&self) -> bool {
        if self.halfmove_clock >= 100 {
            return true;
        }
        self.repetition_counts.get(self.hash) >= 3
    }

    #[must_use]
    pub fn is_theoretical_draw(&self) -> bool {
        self.is_draw() || self.is_insufficient_material()
    }

    /// Count pieces of a given type for a color
    fn piece_count(&self, color: Color, piece: Piece) -> u32 {
        self.pieces[color.index()][piece.index()].0.count_ones()
    }

    /// Count pieces of a given type for both colors combined
    fn total_piece_count(&self, piece: Piece) -> u32 {
        self.piece_count(Color::White, piece) + self.piece_count(Color::Black, piece)
    }

    fn is_insufficient_material(&self) -> bool {
        // Any pawns, rooks, or queens means sufficient material
        if self.total_piece_count(Piece::Pawn) > 0
            || self.total_piece_count(Piece::Rook) > 0
            || self.total_piece_count(Piece::Queen) > 0
        {
            return false;
        }

        let total_knights = self.total_piece_count(Piece::Knight);
        let total_bishops = self.total_piece_count(Piece::Bishop);
        let total_minors = total_knights + total_bishops;

        // K vs K, or K+minor vs K
        if total_minors <= 1 {
            return true;
        }

        // K+B vs K+B with same-colored bishops
        if total_knights == 0 && total_bishops == 2 {
            let all_bishops = self.pieces[Color::White.index()][Piece::Bishop.index()].0
                | self.pieces[Color::Black.index()][Piece::Bishop.index()].0;
            return bishops_all_same_color(all_bishops);
        }

        false
    }
}

impl Default for Board {
    fn default() -> Self {
        Board::new()
    }
}

fn bishops_all_same_color(bishops: u64) -> bool {
    let light_squares: u64 = 0x55AA55AA55AA55AA;
    let dark_squares: u64 = 0xAA55AA55AA55AA55;

    (bishops & light_squares == 0) || (bishops & dark_squares == 0)
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  +---+---+---+---+---+---+---+---+")?;
        for rank in (0..8).rev() {
            write!(f, "{} |", rank + 1)?;
            for file in 0..8 {
                let sq = Square::new(rank, file);
                let piece_char = match self.piece_at(sq) {
                    Some((color, piece)) => piece.to_fen_char(color),
                    None => ' ',
                };
                write!(f, " {piece_char} |")?;
            }
            writeln!(f)?;
            writeln!(f, "  +---+---+---+---+---+---+---+---+")?;
        }
        writeln!(f, "    a   b   c   d   e   f   g   h")?;
        writeln!(f)?;
        write!(
            f,
            "Side to move: {}",
            if self.white_to_move { "White" } else { "Black" }
        )?;
        Ok(())
    }
}

impl Hash for Board {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use the precomputed Zobrist hash
        self.hash.hash(state);
    }
}

impl PartialEq for Board {
    fn eq(&self, other: &Self) -> bool {
        // Two positions are equal if they have the same Zobrist hash
        // This is technically not 100% collision-free but practically sufficient
        self.hash == other.hash
    }
}

impl Eq for Board {}
