use crate::board::Board;

impl Board {
    pub fn perft(&mut self, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let moves = self.generate_moves();
        if depth == 1 {
            return moves.len() as u64;
        }

        let mut nodes = 0;
        for m in moves {
            let info = self.make_move(&m);
            nodes += self.perft(depth - 1);
            self.unmake_move(&m, info);
        }

        nodes
    }
}
