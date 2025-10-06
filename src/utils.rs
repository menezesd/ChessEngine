use crate::board::Board;
use crate::types::*;

/// Format a square as UCI notation (e.g., e4)
pub fn format_square(sq: Square) -> String {
    format!(
        "{}{}",
        (b'a' + sq.1 as u8) as char,
        (b'1' + sq.0 as u8) as char
    )
}

/// Parse a UCI move string (e.g., "e2e4", "e7e8q") into a Move
pub fn parse_uci_move(board: &Board, move_str: &str) -> Option<Move> {
    if move_str.len() < 4 {
        return None;
    }

    let from_file = (move_str.as_bytes()[0] - b'a') as usize;
    let from_rank = (move_str.as_bytes()[1] - b'1') as usize;
    let to_file = (move_str.as_bytes()[2] - b'a') as usize;
    let to_rank = (move_str.as_bytes()[3] - b'1') as usize;

    let from = Square(from_rank, from_file);
    let to = Square(to_rank, to_file);

    let promotion = if move_str.len() == 5 {
        match move_str.as_bytes()[4] as char {
            'q' => Some(Piece::Queen),
            'r' => Some(Piece::Rook),
            'b' => Some(Piece::Bishop),
            'n' => Some(Piece::Knight),
            _ => None,
        }
    } else {
        None
    };

    Some(board.create_move(from, to, promotion, false, false))
}

/// Format a Move as UCI notation
pub fn move_to_uci(m: &Move) -> String {
    let from = format_square(m.from);
    let to = format_square(m.to);
    let mut result = format!("{}{}", from, to);
    if let Some(promo) = m.promotion {
        let promo_char = match promo {
            Piece::Queen => 'q',
            Piece::Rook => 'r',
            Piece::Bishop => 'b',
            Piece::Knight => 'n',
            _ => 'q',
        };
        result.push(promo_char);
    }
    result
}
