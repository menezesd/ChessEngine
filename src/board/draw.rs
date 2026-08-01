use super::{Board, Color, Piece};

const FIFTY_MOVE_RULE_HALFMOVES: u32 = 100;
const THREEFOLD_REPETITION_COUNT: u32 = 3;
const LIGHT_SQUARES: u64 = 0x55AA55AA55AA55AA;
const DARK_SQUARES: u64 = 0xAA55AA55AA55AA55;

impl Board {
    /// Return the knight side for a king-and-two-knights versus bare-king endgame.
    ///
    /// This material class is not an automatic draw: legal mate-in-one
    /// positions exist. Search uses the classification to apply the exact
    /// tablebase result after checking terminal and mate-in-one positions.
    #[must_use]
    pub(crate) fn two_knights_vs_bare_king_side(&self) -> Option<Color> {
        for knight_side in Color::BOTH {
            let bare_king_side = knight_side.opponent();
            let knight_side_has_only_two_knights = self.piece_count(knight_side, Piece::King) == 1
                && self.piece_count(knight_side, Piece::Knight) == 2
                && self.piece_count(knight_side, Piece::Pawn) == 0
                && self.piece_count(knight_side, Piece::Bishop) == 0
                && self.piece_count(knight_side, Piece::Rook) == 0
                && self.piece_count(knight_side, Piece::Queen) == 0;
            let bare_king_side_has_only_king = self.piece_count(bare_king_side, Piece::King) == 1
                && self.piece_count(bare_king_side, Piece::Pawn) == 0
                && self.piece_count(bare_king_side, Piece::Knight) == 0
                && self.piece_count(bare_king_side, Piece::Bishop) == 0
                && self.piece_count(bare_king_side, Piece::Rook) == 0
                && self.piece_count(bare_king_side, Piece::Queen) == 0;

            if knight_side_has_only_two_knights && bare_king_side_has_only_king {
                return Some(knight_side);
            }
        }
        None
    }

    #[must_use]
    pub fn is_draw(&self) -> bool {
        // A mating move takes precedence over the fifty-move claim. Terminal
        // move generation is mutable, so clone only for the rare boundary
        // case where the side to move is already in check; ordinary search
        // nodes retain the read-only fast path.
        if self.halfmove_clock >= FIFTY_MOVE_RULE_HALFMOVES {
            if !self.is_in_check(self.side_to_move()) {
                return true;
            }
            let mut position = self.clone();
            if position.has_legal_move() {
                return true;
            }
        }
        self.repetition_counts.get(self.hash) >= THREEFOLD_REPETITION_COUNT
    }

    #[must_use]
    pub fn is_theoretical_draw(&self) -> bool {
        self.is_draw() || self.is_insufficient_material()
    }

    /// Count pieces of a given type for both colors combined.
    fn total_piece_count(&self, piece: Piece) -> u32 {
        self.piece_count(Color::White, piece) + self.piece_count(Color::Black, piece)
    }

    pub(crate) fn is_insufficient_material(&self) -> bool {
        if self.total_piece_count(Piece::Pawn) > 0
            || self.total_piece_count(Piece::Rook) > 0
            || self.total_piece_count(Piece::Queen) > 0
        {
            return false;
        }

        let total_knights = self.total_piece_count(Piece::Knight);
        let total_bishops = self.total_piece_count(Piece::Bishop);
        let total_minors = total_knights + total_bishops;

        if total_minors <= 1 {
            return true;
        }

        // With no knights, bishops confined to a single colour complex can
        // never cover the opposite-coloured mating square. This remains true
        // with promoted bishops, so do not restrict the test to exactly two.
        if total_knights == 0 && total_bishops > 0 {
            let all_bishops = self.all_pieces_of_type(Piece::Bishop).0;
            return bishops_all_same_color(all_bishops);
        }

        false
    }
}

fn bishops_all_same_color(bishops: u64) -> bool {
    (bishops & LIGHT_SQUARES == 0) || (bishops & DARK_SQUARES == 0)
}
