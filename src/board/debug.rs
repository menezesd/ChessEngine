use super::{Bitboard, Board, Color, Piece};

#[cfg(debug_assertions)]
impl Board {
    /// Debug helper to print all bitboard values
    pub fn debug_bitboards(&self) {
        let colors = [Color::White, Color::Black];
        let pieces = [
            (Piece::Pawn, "P"),
            (Piece::Knight, "N"),
            (Piece::Bishop, "B"),
            (Piece::Rook, "R"),
            (Piece::Queen, "Q"),
            (Piece::King, "K"),
        ];

        println!(
            "Side to move: {}",
            if self.white_to_move { "White" } else { "Black" }
        );
        println!("Castling mask: {:#06b}", self.castling_rights);
        if let Some(ep_target) = self.en_passant_target {
            println!("EP Target: {ep_target}");
        }
        println!("All occupied: {:#018x}", self.all_occupied.0);

        for color in colors {
            let label = if color == Color::White {
                "White"
            } else {
                "Black"
            };
            for (piece, name) in pieces {
                let bb = self.pieces_of(color, piece).0;
                println!("{label} {name}: {bb:#018x}");
            }
        }
        println!("------------------------------------");
    }

    pub fn print_bitboard_grid(&self, label: &str, bb: Bitboard) {
        println!("{} {:#018x}", label, bb.0);
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let idx = (rank * 8 + file) as u8;
                let ch = if (bb.0 >> idx) & 1 == 1 { '1' } else { '.' };
                print!(" {ch} |");
            }
            println!("\n  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!("------------------------------------");
    }
}
