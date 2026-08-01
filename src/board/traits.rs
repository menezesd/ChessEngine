use std::fmt;
use std::hash::{Hash, Hasher};

use super::{Board, Square};

impl Default for Board {
    fn default() -> Self {
        Board::new()
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  +---+---+---+---+---+---+---+---+")?;
        for rank in (0..8).rev() {
            write!(f, "{} |", rank + 1)?;
            for file in 0..8 {
                let sq = Square::new(rank, file);
                let piece_char = match self.piece_at(sq) {
                    Some((color, piece)) => piece.to_fen_char(color),
                    None => ' ',
                };
                write!(f, " {piece_char} |")?;
            }
            writeln!(f)?;
            writeln!(f, "  +---+---+---+---+---+---+---+---+")?;
        }
        writeln!(f, "    a   b   c   d   e   f   g   h")?;
        writeln!(f)?;
        write!(
            f,
            "Side to move: {}",
            if self.white_to_move { "White" } else { "Black" }
        )?;
        Ok(())
    }
}

impl Hash for Board {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl PartialEq for Board {
    fn eq(&self, other: &Self) -> bool {
        // Zobrist values are deliberately compact and can collide. Equality
        // must compare the actual position state rather than treating a hash
        // match as proof of identity. Keep en-passant canonical, matching the
        // repetition/hash convention: an uncapturable target is irrelevant.
        self.pieces == other.pieces
            && self.white_to_move == other.white_to_move
            && self.castling_rights == other.castling_rights
            && self.en_passant_target.map_or(0, |sq| {
                self.en_passant_hash_component(sq, self.side_to_move())
            }) == other.en_passant_target.map_or(0, |sq| {
                other.en_passant_hash_component(sq, other.side_to_move())
            })
    }
}

impl Eq for Board {}

#[cfg(test)]
mod tests {
    use crate::board::{Board, Color, Piece, Square};

    #[test]
    fn board_equality_does_not_trust_hash_alone() {
        let board = Board::new();
        let mut altered = board.clone();
        // Deliberately keep the cached hash unchanged to model the only
        // observable effect of a Zobrist collision.
        altered.pieces[Color::White.index()][Piece::Pawn.index()].0 ^=
            1_u64 << Square::new(1, 0).index();

        assert_eq!(board.hash(), altered.hash());
        assert_ne!(board, altered);
    }

    #[test]
    fn board_equality_keeps_uncapturable_en_passant_canonical() {
        let e_file = Board::from_fen("8/8/8/8/8/8/8/K1k5 b - e3 0 1");
        let f_file = Board::from_fen("8/8/8/8/8/8/8/K1k5 b - f3 0 1");

        assert_eq!(e_file, f_file);
    }

    #[test]
    fn board_equality_distinguishes_capturable_en_passant_targets() {
        let capturable = Board::from_fen("8/8/8/8/3pP3/8/8/K1k5 b - e3 0 1");
        let uncapturable = Board::from_fen("8/8/8/8/3pP3/8/8/K1k5 b - f3 0 1");

        assert_ne!(capturable, uncapturable);
    }
}
