use crate::core::board::Board;

/// Performance test utilities
pub struct Perft;

impl Perft {
    /// Count the number of nodes at a given depth from the current position
    pub fn perft(board: &mut Board, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let mut mvbuf: crate::core::types::MoveList = crate::core::types::MoveList::new();
        board.generate_moves_into(&mut mvbuf);
        if depth == 1 {
            return mvbuf.len() as u64;
        }

        let mut nodes = 0;
        for m in mvbuf {
            let info = board.make_move(&m);
            nodes += Self::perft(board, depth - 1);
            board.unmake_move(&m, info);
        }

        nodes
    }
}