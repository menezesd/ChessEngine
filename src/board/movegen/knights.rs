use super::super::attack_tables::KNIGHT_ATTACKS;
use super::super::{Bitboard, Board, MoveList, Square};

impl Board {
    pub(crate) fn generate_knight_moves(&self, from: Square) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.side_to_move();
        let from_idx = from.index();
        let own_occ = self.occupied_by(color).0;
        let targets = Bitboard(KNIGHT_ATTACKS[from_idx] & !own_occ);

        for to_idx in targets.iter() {
            let to_sq = to_idx;
            moves.push(self.create_simple_move(from, to_sq));
        }
        moves
    }
}
