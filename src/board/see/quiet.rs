use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS, PAWN_ATTACKS};
use crate::board::{Board, Piece, Square};

use super::SEE_VALUES;

impl Board {
    /// Check if a quiet move is safe using SEE.
    ///
    /// Returns true if moving the piece to the destination is unlikely to lose material.
    /// This evaluates what happens if the opponent captures our piece after we move it.
    ///
    /// # Arguments
    /// * `from` - Source square of the moving piece
    /// * `to` - Target square (empty, since it's a quiet move)
    #[must_use]
    pub fn see_quiet_safe(&self, from: Square, to: Square) -> bool {
        let Some((color, piece)) = self.piece_at(from) else {
            return true;
        };

        let enemy_pawns = self.opponent_pieces(color, Piece::Pawn);
        let pawn_attacks_to_sq = PAWN_ATTACKS[color.index()][to.index()];
        if (pawn_attacks_to_sq & enemy_pawns.0) != 0 && piece != Piece::Pawn {
            return false;
        }

        let piece_value = SEE_VALUES[piece.index()];

        let enemy_knights = self.opponent_pieces(color, Piece::Knight);
        if (KNIGHT_ATTACKS[to.index()] & enemy_knights.0) != 0
            && SEE_VALUES[Piece::Knight.index()] < piece_value
        {
            return false;
        }

        if piece_value >= SEE_VALUES[Piece::Bishop.index()] {
            let occupancy = self.all_occupied.0;

            let enemy_bishops = self.opponent_pieces(color, Piece::Bishop);
            let enemy_queens = self.opponent_pieces(color, Piece::Queen);
            let diag_attacks = slider_attacks(to.index(), occupancy, true);
            if (diag_attacks & (enemy_bishops.0 | enemy_queens.0)) != 0
                && (diag_attacks & enemy_bishops.0) != 0
                && SEE_VALUES[Piece::Bishop.index()] < piece_value
            {
                return false;
            }

            if piece_value >= SEE_VALUES[Piece::Rook.index()] {
                let enemy_rooks = self.opponent_pieces(color, Piece::Rook);
                let straight_attacks = slider_attacks(to.index(), occupancy, false);
                if (straight_attacks & enemy_rooks.0) != 0 {
                    return false;
                }
            }
        }

        true
    }
}
