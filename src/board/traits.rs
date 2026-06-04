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
        self.hash == other.hash
    }
}

impl Eq for Board {}
