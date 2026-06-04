use crate::board::Move;

pub(super) struct SearchProgress {
    previous_best_move: Option<Move>,
    previous_score: i32,
    stability_count: u32,
    prev_iter_nodes: u64,
}

impl SearchProgress {
    pub(super) fn new(initial_score: i32) -> Self {
        Self {
            previous_best_move: None,
            previous_score: initial_score,
            stability_count: 0,
            prev_iter_nodes: 0,
        }
    }

    pub(super) fn stability_count(&self) -> u32 {
        self.stability_count
    }

    pub(super) fn previous_score(&self) -> i32 {
        self.previous_score
    }

    pub(super) fn prev_iter_nodes(&self) -> u64 {
        self.prev_iter_nodes
    }

    pub(super) fn update(&mut self, best_move: Option<Move>, score: i32, iter_nodes: u64) {
        if best_move == self.previous_best_move && best_move.is_some() {
            self.stability_count = self.stability_count.saturating_add(1);
        } else {
            self.stability_count = 0;
        }
        self.previous_best_move = best_move;
        self.previous_score = score;
        self.prev_iter_nodes = iter_nodes;
    }
}
