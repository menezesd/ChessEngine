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
        let color = self.current_color();
        let own_occ = self.occupied[color.index()].0;
        let from_idx = from.index().as_usize();

        let targets_raw = match slider {
            SliderType::Bishop => slider_attacks(from_idx, self.all_occupied.0, true),
            SliderType::Rook => slider_attacks(from_idx, self.all_occupied.0, false),
            SliderType::Queen => {
                slider_attacks(from_idx, self.all_occupied.0, false)
                    | slider_attacks(from_idx, self.all_occupied.0, true)
            }
        } & !own_occ;

        for to_idx in Bitboard(targets_raw).iter() {
            let to_sq = Square::from_index(to_idx);
            moves.push(self.create_move(from, to_sq, None, false, false));
        }
        moves
    }
}
