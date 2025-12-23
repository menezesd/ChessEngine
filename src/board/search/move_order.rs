use crate::board::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use crate::board::{
    color_index, piece_index, square_index, Bitboard, Board, Color, Move, Piece, SearchState,
};

const HASH_SCORE: i32 = 1_000_000;
const PV_SCORE: i32 = 900_000;
const CAPTURE_BASE: i32 = 500_000;
const KILLER_SCORE: i32 = 400_000;
const COUNTER_SCORE: i32 = 300_000;

pub(crate) fn order_moves(
    board: &Board,
    state: &SearchState,
    moves: &mut crate::board::MoveList,
    ply: u32,
    hash_move: Option<Move>,
    counter_move: Option<Move>,
) {
    moves.as_mut_slice().sort_by_key(|m| {
        -score_move(board, state, m, ply, hash_move, counter_move, None)
    });
}

pub(crate) fn order_root_moves(
    board: &Board,
    state: &SearchState,
    moves: &mut crate::board::MoveList,
    hash_move: Option<Move>,
    pv_move: Option<Move>,
) {
    moves
        .as_mut_slice()
        .sort_by_key(|m| -score_move(board, state, m, 0, hash_move, None, pv_move));
}

pub(crate) fn is_bad_capture(board: &Board, m: &Move) -> bool {
    if m.captured_piece.is_none() {
        return false;
    }
    see_capture(board, m) < 0
}

pub(crate) fn mvv_lva_score(m: &Move, board: &Board) -> i32 {
    if let Some(victim) = m.captured_piece {
        let attacker = board.piece_at(m.from).unwrap().1;
        let victim_value = piece_value(victim);
        let attacker_value = piece_value(attacker);
        victim_value * 10 - attacker_value
    } else {
        0
    }
}

pub(crate) fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 300,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000,
    }
}

fn score_move(
    board: &Board,
    state: &SearchState,
    m: &Move,
    ply: u32,
    hash_move: Option<Move>,
    counter_move: Option<Move>,
    pv_move: Option<Move>,
) -> i32 {
    if let Some(hm) = hash_move {
        if *m == hm {
            return HASH_SCORE;
        }
    }

    if let Some(pv) = pv_move {
        if *m == pv {
            return PV_SCORE;
        }
    }

    if m.captured_piece.is_some() || m.is_en_passant {
        let see = see_capture(board, m);
        let mut score = CAPTURE_BASE + mvv_lva_score(m, board);
        if see < 0 {
            score -= 10_000;
        } else {
            score += see;
        }
        return score;
    }

    if state.is_killer(ply as usize, *m) {
        return KILLER_SCORE;
    }

    if let Some(cm) = counter_move {
        if *m == cm {
            return COUNTER_SCORE;
        }
    }

    state.history_score(*m)
}

fn see_capture(board: &Board, m: &Move) -> i32 {
    let captured = match m.captured_piece {
        Some(p) => p,
        None => return 0,
    };
    if m.is_en_passant {
        return 0;
    }
    let (moving_color, moving_piece) = match board.piece_at(m.from) {
        Some(info) => info,
        None => return 0,
    };
    let promotion_piece = m.promotion.unwrap_or(moving_piece);

    let mut pieces = board.pieces;
    let from_bb = 1u64 << square_index(m.from).0;
    let to_bb = 1u64 << square_index(m.to).0;
    let mover_idx = color_index(moving_color);
    let opp_idx = color_index(board.opponent_color(moving_color));

    pieces[opp_idx][piece_index(captured)].0 &= !to_bb;
    pieces[mover_idx][piece_index(moving_piece)].0 &= !from_bb;
    pieces[mover_idx][piece_index(promotion_piece)].0 |= to_bb;

    let mut occ = board.all_occupied.0;
    occ &= !from_bb;
    occ &= !to_bb;
    occ |= to_bb;

    let attackers_to = |color: Color, occ: u64, pieces: &[[Bitboard; 6]; 2]| -> u64 {
        let sq_idx = square_index(m.to).0 as usize;
        let c_idx = color_index(color);
        let pawns = if color == Color::White {
            pieces[c_idx][piece_index(Piece::Pawn)].0 & PAWN_ATTACKS[1][sq_idx]
        } else {
            pieces[c_idx][piece_index(Piece::Pawn)].0 & PAWN_ATTACKS[0][sq_idx]
        };
        let knights = pieces[c_idx][piece_index(Piece::Knight)].0 & KNIGHT_ATTACKS[sq_idx];
        let bishops =
            pieces[c_idx][piece_index(Piece::Bishop)].0 & slider_attacks(sq_idx, occ, true);
        let rooks =
            pieces[c_idx][piece_index(Piece::Rook)].0 & slider_attacks(sq_idx, occ, false);
        let queens = pieces[c_idx][piece_index(Piece::Queen)].0
            & (slider_attacks(sq_idx, occ, true) | slider_attacks(sq_idx, occ, false));
        let kings = pieces[c_idx][piece_index(Piece::King)].0 & KING_ATTACKS[sq_idx];
        pawns | knights | bishops | rooks | queens | kings
    };

    let mut gains = [0i32; 32];
    gains[0] = piece_value(captured);
    let mut depth = 0usize;
    let mut side = board.opponent_color(moving_color);

    loop {
        let attackers = attackers_to(side, occ, &pieces);
        if attackers == 0 {
            break;
        }

        let side_idx = color_index(side);
        let mut attacker_piece = None;
        let mut attacker_sq = 0u64;
        for piece in [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            let bb = pieces[side_idx][piece_index(piece)].0 & attackers;
            if bb != 0 {
                attacker_piece = Some(piece);
                attacker_sq = bb & (!bb + 1);
                break;
            }
        }
        let attacker_piece = match attacker_piece {
            Some(p) => p,
            None => break,
        };

        depth += 1;
        gains[depth] = piece_value(attacker_piece) - gains[depth - 1];
        if gains[depth].max(-gains[depth - 1]) < 0 {
            break;
        }

        pieces[side_idx][piece_index(attacker_piece)].0 &= !attacker_sq;
        pieces[side_idx][piece_index(attacker_piece)].0 |= to_bb;
        occ &= !attacker_sq;

        side = board.opponent_color(side);
    }

    while depth > 0 {
        let d = depth;
        gains[d - 1] = -std::cmp::max(-gains[d - 1], gains[d]);
        depth -= 1;
    }

    gains[0]
}
