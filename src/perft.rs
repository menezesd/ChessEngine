use crate::board::Board;

/// Perft test: count nodes at given depth
pub fn perft(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 { return 1; }
    let moves = board.generate_moves();
    if depth == 1 { return moves.len() as u64; }
    let mut nodes = 0;
    for m in moves {
        let info = board.make_move(&m);
        nodes += perft(board, depth - 1);
        board.unmake_move(&m, info);
    }
    nodes
}
