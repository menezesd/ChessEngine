use crate::board::attack_tables::{slider_attacks, KNIGHT_ATTACKS, PAWN_ATTACKS};
use crate::board::{Bitboard, Board, Piece, Square};

use super::SEE_VALUES;

impl Board {
    /// Screen a quiet move for possible material loss.
    ///
    /// Returns true if moving the piece to the destination is unlikely to lose material.
    /// Search verifies flagged moves with a legal exchange after making the move.
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
            let diag_attacks = slider_attacks(to.index(), occupancy, true);
            if (diag_attacks & enemy_bishops.0) != 0
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

    /// Verify that the side to move can win material by capturing the
    /// piece that just moved to this square.
    pub(crate) fn can_win_exchange_on(&self, target: Square, victim: Piece) -> bool {
        let side = self.side_to_move();
        let occupancy = self.all_occupied;
        let attackers = self.attackers_to(target, occupancy).0 & self.occupied_by(side).0;
        let Some((attacker, from)) =
            self.least_valuable_legal_attacker(Bitboard(attackers), side, target, occupancy.0)
        else {
            return false;
        };
        let source = Square::from_index(from.0.trailing_zeros() as usize);
        self.see_with_pieces(source, target, attacker, victim) > 0
    }
}
