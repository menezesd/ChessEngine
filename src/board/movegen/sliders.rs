use super::super::attack_tables::slider_attacks;
use super::super::{pop_lsb, Bitboard, Board, MoveList, Square};

impl Board {
    pub(crate) fn generate_sliding_moves(
        &self,
        from: Square,
        directions: &[(isize, isize)],
    ) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.current_color();
        let own_occ = self.occupied[color.index()].0;
        let from_idx = from.index().as_usize();

        let targets = if directions.len() == 4 && directions[0].0 != 0 && directions[0].1 != 0 {
            slider_attacks(from_idx, self.all_occupied.0, true)
        } else if directions.len() == 4 {
            slider_attacks(from_idx, self.all_occupied.0, false)
        } else {
            slider_attacks(from_idx, self.all_occupied.0, false)
                | slider_attacks(from_idx, self.all_occupied.0, true)
        } & !own_occ;

        let mut targets = Bitboard(targets);
        while targets.0 != 0 {
            let to_idx = pop_lsb(&mut targets);
            let to_sq = Square::from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false));
        }
        moves
    }
}
