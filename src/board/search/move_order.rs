use super::super::{color_index, piece_index, square_index, Color, Piece, Square};
use super::{Board, Move, SearchState};

pub fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 20000,
    }
}

fn piece_on(board: &Board, square: Square) -> Option<Piece> {
    let idx = square_index(square).0 as u64;
    let bit = 1u64 << idx;
    for color in [Color::White, Color::Black] {
        let c = color_index(color);
        for piece in [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            let p = piece_index(piece);
            if board.pieces[c][p].0 & bit != 0 {
                return Some(piece);
            }
        }
    }
    None
}

pub fn mvv_lva_score(mv: &Move, board: &Board) -> i32 {
    let captured = match mv.captured_piece {
        Some(piece) => piece_value(piece),
        None => return 0,
    };
    let attacker = piece_on(board, mv.from).map(piece_value).unwrap_or(0);
    captured * 10 - attacker
}

pub fn is_bad_capture(board: &Board, mv: &Move) -> bool {
    let captured = match mv.captured_piece {
        Some(piece) => piece_value(piece),
        None => return false,
    };
    let attacker = piece_on(board, mv.from).map(piece_value).unwrap_or(0);
    captured + 50 < attacker
}

pub fn order_moves(
    board: &Board,
    state: &SearchState,
    moves: &mut super::MoveList,
    ply: u32,
    hash_move: Option<Move>,
    counter_move: Option<Move>,
) {
    let ply_usize = ply as usize;
    moves.as_mut_slice().sort_by_key(|m| {
        let mut score = 0i32;
        if Some(*m) == hash_move {
            score += 1_000_000;
        }
        if Some(*m) == counter_move {
            score += 50_000;
        }
        if state.is_killer(ply_usize, *m) {
            score += 40_000;
        }
        if m.captured_piece.is_some() {
            score += 10_000 + mvv_lva_score(m, board);
        }
        if m.promotion.is_some() {
            score += 9_000;
        }
        score += state.history_score(*m) / 16;
        -score
    });
}

pub fn order_root_moves(
    board: &Board,
    state: &SearchState,
    moves: &mut super::MoveList,
    hash_move: Option<Move>,
    pv_move: Option<Move>,
) {
    moves.as_mut_slice().sort_by_key(|m| {
        let mut score = 0i32;
        if Some(*m) == pv_move {
            score += 1_000_000;
        }
        if Some(*m) == hash_move {
            score += 900_000;
        }
        if m.captured_piece.is_some() {
            score += 10_000 + mvv_lva_score(m, board);
        }
        if m.promotion.is_some() {
            score += 9_000;
        }
        score += state.history_score(*m) / 16;
        -score
    });
}
