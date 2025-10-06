use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::time::Duration;
use crate::types::{Color, Move, Square};

/// Format a move into UCI string
pub fn format_uci_move(mv: &Move) -> String {
    fn square_to_uci(sq: Square) -> String {
        let file = (b'a' + (sq.1 as u8)) as char;
        let rank = (b'1' + (sq.0 as u8)) as char;
        format!("{}{}", file, rank)
    }
    let mut s = square_to_uci(mv.from);
    s.push_str(&square_to_uci(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::Rook => 'r',
            Piece::Queen => 'q',
            _ => panic!("Invalid promo piece"),
        });
    }
    s
}

/// Parse position command
pub fn parse_position_command(board: &mut crate::board::Board, parts: &[&str]) {
    // existing code...
}

/// Get the value of a piece for MVV-LVA and evaluation
pub fn piece_value(piece: crate::types::Piece) -> i32 {
    match piece {
        crate::types::Piece::Pawn => crate::types::PAWN_VALUE,
        crate::types::Piece::Knight => crate::types::KNIGHT_VALUE,
        crate::types::Piece::Bishop => crate::types::BISHOP_VALUE,
        crate::types::Piece::Rook => crate::types::ROOK_VALUE,
        crate::types::Piece::Queen => crate::types::QUEEN_VALUE,
        crate::types::Piece::King => crate::types::KING_VALUE,
    }
}

/// Calculate MVV-LVA (Most Valuable Victim - Least Valuable Attacker) score
pub fn mvv_lva_score(mv: &crate::types::Move, board: &crate::board::Board) -> i32 {
    if let Some(victim) = mv.captured_piece {
        let attacker = board.squares[mv.from.0][mv.from.1].unwrap().1;
        let victim_val = piece_value(victim);
        let attacker_val = piece_value(attacker);
        victim_val * 10 - attacker_val
    } else {
        0
    }
}
