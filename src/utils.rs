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
