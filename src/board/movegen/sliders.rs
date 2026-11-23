use super::super::attack_tables::slider_attacks;
use super::super::{Board, Square};

/// Type of sliding piece for move generation
#[derive(Clone, Copy)]
pub(crate) enum SliderType {
    Bishop,
    Rook,
    Queen,
}

impl Board {
    #[inline]
    pub(super) fn slider_targets(&self, from: Square, slider: SliderType) -> u64 {
        let from_idx = from.index();
        match slider {
            SliderType::Bishop => slider_attacks(from_idx, self.all_occupied.0, true),
            SliderType::Rook => slider_attacks(from_idx, self.all_occupied.0, false),
            SliderType::Queen => {
                slider_attacks(from_idx, self.all_occupied.0, false)
                    | slider_attacks(from_idx, self.all_occupied.0, true)
            }
        }
    }
}
