//! Static Exchange Evaluation (SEE).
//!
//! Evaluates capture sequences on a single square to determine
//! if a capture is winning, losing, or equal.

mod attackers;
mod quiet;

use super::attack_tables::slider_attacks;
use super::state::Board;
use super::types::{Bitboard, Color, Piece, Square};

/// Piece values for SEE (simpler than eval values)
pub(super) const SEE_VALUES: [i32; 6] = [
    100,   // Pawn
    320,   // Knight
    330,   // Bishop
    500,   // Rook
    900,   // Queen
    20000, // King
];

fn exchange_piece(piece: Piece, color: Color, target: Square) -> (Piece, i32) {
    if piece == Piece::Pawn && target.rank() == color.pawn_promotion_rank() {
        (
            Piece::Queen,
            SEE_VALUES[Piece::Queen.index()] - SEE_VALUES[Piece::Pawn.index()],
        )
    } else {
        (piece, 0)
    }
}

impl Board {
    fn see_capture_is_legal(
        &self,
        from: Bitboard,
        piece: Piece,
        target: Square,
        side: Color,
        occupancy: u64,
    ) -> bool {
        let occupancy_after = occupancy & !from.0;
        let king_square = if piece == Piece::King {
            target
        } else {
            self.find_king(side)
        };

        // The original victim still exists in the board's static piece
        // bitboards at `target`, even though the virtual exchange has already
        // replaced it. Exclude that square when filtering enemy attackers.
        let target_bit = Bitboard::from_square(target).0;
        let enemy = side.opponent();
        let enemy_attackers = self.attackers_to(king_square, Bitboard(occupancy_after)).0
            & self.occupied_by(enemy).0
            & occupancy_after
            & !target_bit;

        enemy_attackers == 0
    }

    fn least_valuable_legal_attacker(
        &self,
        attackers: Bitboard,
        side: Color,
        target: Square,
        occupancy: u64,
    ) -> Option<(Piece, Bitboard)> {
        for piece in [
            Piece::Pawn,
            Piece::Knight,
            Piece::Bishop,
            Piece::Rook,
            Piece::Queen,
            Piece::King,
        ] {
            let mut candidates = attackers.0 & self.pieces_of(side, piece).0;
            while candidates != 0 {
                let bit = candidates & candidates.wrapping_neg();
                let from = Bitboard(bit);
                if self.see_capture_is_legal(from, piece, target, side, occupancy) {
                    return Some((piece, from));
                }
                candidates &= candidates - 1;
            }
        }
        None
    }

    /// Static Exchange Evaluation for a capture move.
    ///
    /// Returns the material balance after all exchanges on the target square.
    /// Pawn promotions are valued as queens because this coordinate-based
    /// API does not specify an underpromotion piece.
    /// Positive = winning for the side making the initial capture.
    /// Negative = losing.
    /// Zero = equal exchange.
    ///
    /// # Arguments
    /// * `from` - Source square of the capturing piece
    /// * `to` - Target square (where the capture happens)
    ///
    /// # Returns
    /// Material balance in centipawns from the perspective of the side to move.
    #[must_use]
    pub fn see(&self, from: Square, to: Square) -> i32 {
        let side_to_move = self.side_to_move();

        // Get the piece being captured
        let (captured, en_passant_capture_square) = match self.piece_at(to) {
            Some((color, piece)) if color == side_to_move.opponent() && piece != Piece::King => {
                (piece, None)
            }
            Some(_) => return 0,
            None => {
                // En passant - captured piece is a pawn
                if self.en_passant_target == Some(to) {
                    let capture_rank = if side_to_move == Color::White {
                        to.rank().checked_sub(1)
                    } else {
                        to.rank().checked_add(1).filter(|rank| *rank < 8)
                    };
                    let Some(capture_rank) = capture_rank else {
                        return 0;
                    };
                    let capture_square = Square::new(capture_rank, to.file());
                    if self.piece_at(capture_square) != Some((side_to_move.opponent(), Piece::Pawn))
                    {
                        return 0;
                    }
                    (Piece::Pawn, Some(capture_square))
                } else {
                    return 0; // No capture
                }
            }
        };

        // Get the attacking piece
        let Some((color, attacker)) = self.piece_at(from) else {
            return 0;
        };
        if color != side_to_move {
            return 0;
        }

        self.see_impl(from, to, attacker, captured, en_passant_capture_square)
    }

    /// SEE with known attacker and victim pieces.
    ///
    /// This avoids redundant `piece_at` lookups when the caller already knows the pieces.
    #[inline]
    #[must_use]
    pub fn see_with_pieces(&self, from: Square, to: Square, attacker: Piece, victim: Piece) -> i32 {
        self.see_impl(from, to, attacker, victim, None)
    }

    /// SEE implementation with known attacker and victim.
    fn see_impl(
        &self,
        from: Square,
        to: Square,
        attacker: Piece,
        victim: Piece,
        en_passant_capture_square: Option<Square>,
    ) -> i32 {
        // Maximum depth of exchanges (should never be exceeded)
        const MAX_DEPTH: usize = 32;

        // Score at each ply of the exchange
        let mut gain = [0i32; MAX_DEPTH];
        let mut depth = 0;

        // Track which side is moving
        let mut side_to_move = self.white_to_move;

        // Build occupancy that we'll modify as pieces are "removed"
        let mut occupancy = self.all_occupied.0;
        if let Some(capture_sq) = en_passant_capture_square {
            occupancy &= !Bitboard::from_square(capture_sq).0;
        }

        // Get all attackers to the target square
        let mut attackers = self.attackers_to(to, Bitboard(occupancy));

        let (mut current_attacker, promotion_gain) =
            exchange_piece(attacker, self.side_to_move(), to);
        gain[0] = SEE_VALUES[victim.index()] + promotion_gain;
        let mut from_bb = Bitboard::from_square(from);

        loop {
            // Remove the attacker from the board
            occupancy ^= from_bb.0;
            attackers = Bitboard(attackers.0 & !from_bb.0);

            // Check for x-ray attacks revealed by removing this piece
            // Only sliders can have x-ray attacks
            if current_attacker == Piece::Pawn
                || current_attacker == Piece::Bishop
                || current_attacker == Piece::Queen
            {
                // Diagonal x-ray
                let diag_attackers = self.diagonal_sliders();
                let new_diag =
                    Bitboard(slider_attacks(to.index(), occupancy, true) & diag_attackers.0);
                attackers = Bitboard(attackers.0 | (new_diag.0 & occupancy));
            }

            if current_attacker.attacks_straight() {
                // Straight x-ray
                let straight_attackers = self.straight_sliders();
                let new_straight =
                    Bitboard(slider_attacks(to.index(), occupancy, false) & straight_attackers.0);
                attackers = Bitboard(attackers.0 | (new_straight.0 & occupancy));
            }

            // Switch sides
            side_to_move = !side_to_move;

            // Find the least valuable attacker for the side to move
            let side_color = if side_to_move {
                Color::White
            } else {
                Color::Black
            };
            let side_pieces = self.occupied_by(side_color);

            // Filter attackers to only include pieces of the side to move
            let side_attackers = Bitboard(attackers.0 & side_pieces.0);

            // If no more attackers, we're done
            if side_attackers.is_empty() {
                break;
            }

            // Leave the last slot unused: the minimax pass below reads
            // `gain[depth + 1]`, so a sequence ending exactly at the array
            // boundary would read out of bounds.
            if depth + 1 >= MAX_DEPTH {
                break;
            }
            // Pinned pieces and pieces whose removal exposes a discovered
            // check cannot legally recapture. If the first attacker of a
            // given value is pinned, another attacker of the same type may
            // still be usable.
            let Some((lva_piece, lva_bb)) =
                self.least_valuable_legal_attacker(side_attackers, side_color, to, occupancy)
            else {
                break;
            };

            depth += 1;

            // Score from making this capture (negated, as it's opponent's gain)
            let (placed_piece, promotion_gain) = exchange_piece(lva_piece, side_color, to);
            gain[depth] = SEE_VALUES[current_attacker.index()] + promotion_gain - gain[depth - 1];

            current_attacker = placed_piece;
            from_bb = lva_bb;
        }

        // Minimax the gains back up
        while depth > 0 {
            depth -= 1;
            gain[depth] = -(-gain[depth]).max(gain[depth + 1]);
        }

        gain[0]
    }

    /// Quick SEE test: returns true if the capture is likely good (SEE >= threshold).
    ///
    /// This is a faster approximation that can be used for move ordering.
    #[inline]
    #[must_use]
    pub fn see_ge(&self, from: Square, to: Square, threshold: i32) -> bool {
        self.see(from, to) >= threshold
    }
}

#[cfg(test)]
mod tests;
