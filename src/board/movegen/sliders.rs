use super::super::attack_tables::slider_attacks;
use super::super::{Bitboard, Board, MoveList, Square};

/// Type of sliding piece for move generation
#[derive(Clone, Copy)]
pub(crate) enum SliderType {
    Bishop,
    Rook,
    Queen,
}

impl Board {
    pub(crate) fn generate_slider_moves(&self, from: Square, slider: SliderType) -> MoveList {
        let mut moves = MoveList::new();
        let color = self.side_to_move();
        let own_occ = self.occupied_by(color).0;
        let from_idx = from.index();

        let targets_raw = match slider {
            SliderType::Bishop => slider_attacks(from_idx, self.all_occupied.0, true),
            SliderType::Rook => slider_attacks(from_idx, self.all_occupied.0, false),
            SliderType::Queen => {
                slider_attacks(from_idx, self.all_occupied.0, false)
                    | slider_attacks(from_idx, self.all_occupied.0, true)
            }
        } & !own_occ;

        for to_idx in Bitboard(targets_raw).iter() {
            let to_sq = to_idx;
            moves.push(self.create_simple_move(from, to_sq));
        }
        moves
    }
}
