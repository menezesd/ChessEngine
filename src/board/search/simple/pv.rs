use crate::board::{Move, EMPTY_MOVE, MAX_PLY};

use super::SimpleSearchContext;

impl SimpleSearchContext<'_> {
    /// Extract Principal Variation from TT.
    pub(super) fn extract_pv(&mut self, max_len: usize) -> Vec<Move> {
        self.extract_pv_with_first_move_opt(None, max_len)
    }

    /// Extract PV with a specific first move, then continue from TT.
    ///
    /// Used for `MultiPV` to ensure the PV starts with the correct best move.
    pub(super) fn extract_pv_with_first_move(
        &mut self,
        first_move: Move,
        max_len: usize,
    ) -> Vec<Move> {
        self.extract_pv_with_first_move_opt(Some(first_move), max_len)
    }

    fn extract_pv_with_first_move_opt(
        &mut self,
        first_move: Option<Move>,
        max_len: usize,
    ) -> Vec<Move> {
        let mut pv = Vec::with_capacity(max_len);
        let mut seen_hashes = [0u64; MAX_PLY];
        let mut unmake_infos = Vec::with_capacity(max_len);

        let effective_max = max_len.min(MAX_PLY);
        for (seen_count, _) in (0..effective_max).enumerate() {
            let hash = self.board.hash;
            if seen_hashes[..seen_count].contains(&hash) {
                break;
            }
            seen_hashes[seen_count] = hash;

            let mv = if let (0, Some(fm)) = (seen_count, first_move) {
                fm
            } else {
                let tt_move = self
                    .state
                    .tables
                    .tt
                    .probe(self.board.hash)
                    .and_then(|entry| entry.best_move());

                let Some(m) = tt_move else { break };
                if m == EMPTY_MOVE {
                    break;
                }
                m
            };

            if !self.board.is_legal_move(mv) {
                break;
            }

            pv.push(mv);
            let info = self.board.make_move(mv);
            unmake_infos.push((mv, info));
        }

        for (mv, info) in unmake_infos.into_iter().rev() {
            self.board.unmake_move(mv, info);
        }

        pv
    }

    /// Format PV moves as a space-separated string of UCI moves.
    pub(super) fn format_pv(pv: &[Move]) -> String {
        pv.iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }
}
