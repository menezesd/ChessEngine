use super::{Board, Color, Piece};

const FIFTY_MOVE_RULE_HALFMOVES: u32 = 100;
const THREEFOLD_REPETITION_COUNT: u32 = 3;
const LIGHT_SQUARES: u64 = 0x55AA55AA55AA55AA;
const DARK_SQUARES: u64 = 0xAA55AA55AA55AA55;

impl Board {
    #[must_use]
    pub fn is_draw(&self) -> bool {
        if self.halfmove_clock >= FIFTY_MOVE_RULE_HALFMOVES {
            return true;
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

    fn is_insufficient_material(&self) -> bool {
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

        if total_knights == 0 && total_bishops == 2 {
            let all_bishops = self.all_pieces_of_type(Piece::Bishop).0;
            return bishops_all_same_color(all_bishops);
        }

        false
    }
}

fn bishops_all_same_color(bishops: u64) -> bool {
    (bishops & LIGHT_SQUARES == 0) || (bishops & DARK_SQUARES == 0)
}
