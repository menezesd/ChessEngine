use super::super::attack_tables::KNIGHT_ATTACKS;
use super::super::{
    color_index, pop_lsb, square_from_index, square_index, Bitboard, Board, MoveList, Square,
};

impl Board {
    pub(crate) fn generate_knight_moves(&self, from: Square) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.current_color();
        let from_idx = square_index(from).as_usize();
        let own_occ = self.occupied[color_index(color)].0;
        let mut targets = Bitboard(KNIGHT_ATTACKS[from_idx] & !own_occ);

        while targets.0 != 0 {
            let to_idx = pop_lsb(&mut targets);
            let to_sq = square_from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false));
        }
        moves
    }
}
