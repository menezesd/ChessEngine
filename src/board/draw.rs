use super::{Bitboard, Board, Piece, Square};

pub(super) const FIFTY_MOVE_RULE_HALFMOVES: u32 = 100;
pub(super) const THREEFOLD_REPETITION_COUNT: u32 = 3;
const SEVENTY_FIVE_MOVE_RULE_HALFMOVES: u32 = 150;
const FIVEFOLD_REPETITION_COUNT: u32 = 5;
const LIGHT_SQUARES: u64 = 0x55AA55AA55AA55AA;
const DARK_SQUARES: u64 = 0xAA55AA55AA55AA55;

impl Board {
    #[must_use]
    pub fn is_draw(&self) -> bool {
        self.is_fifty_move_draw()
            || self.repetition_counts.get(self.hash) >= THREEFOLD_REPETITION_COUNT
    }

    pub(crate) fn is_fifty_move_draw(&self) -> bool {
        // A mating move takes precedence over the fifty-move claim.
        self.halfmove_clock >= FIFTY_MOVE_RULE_HALFMOVES
            && (!self.is_in_check(self.side_to_move()) || self.has_legal_move())
    }

    #[must_use]
    pub fn is_theoretical_draw(&self) -> bool {
        self.is_draw() || self.is_insufficient_material()
    }

    /// Whether a root already scored as a draw can still play a winning move
    /// instead of claiming. Automatic draws and dead positions end the game.
    pub(crate) fn can_play_on_after_draw_claim(&self) -> bool {
        self.halfmove_clock < SEVENTY_FIVE_MOVE_RULE_HALFMOVES
            && self.repetition_counts.get(self.hash) < FIVEFOLD_REPETITION_COUNT
            && !self.is_insufficient_material()
    }

    pub(crate) fn is_insufficient_material(&self) -> bool {
        self.is_insufficient_material_with_mask(u64::MAX)
    }

    /// A legal ordinary capture removes material without changing the
    /// mover's material class or a moving bishop's square color.
    pub(crate) fn is_insufficient_material_without(&self, captured: Square) -> bool {
        self.is_insufficient_material_with_mask(!Bitboard::from_square(captured).0)
    }

    fn is_insufficient_material_with_mask(&self, remaining: u64) -> bool {
        if self.all_pieces_of_type(Piece::Pawn).0 & remaining != 0
            || self.all_pieces_of_type(Piece::Rook).0 & remaining != 0
            || self.all_pieces_of_type(Piece::Queen).0 & remaining != 0
        {
            return false;
        }

        let knights = self.all_pieces_of_type(Piece::Knight).0 & remaining;
        let bishops = self.all_pieces_of_type(Piece::Bishop).0 & remaining;
        if knights == 0 {
            // Any number of bishops confined to one color complex is insufficient.
            bishops_all_same_color(bishops)
        } else {
            bishops == 0 && knights.is_power_of_two()
        }
    }
}

fn bishops_all_same_color(bishops: u64) -> bool {
    (bishops & LIGHT_SQUARES == 0) || (bishops & DARK_SQUARES == 0)
}
