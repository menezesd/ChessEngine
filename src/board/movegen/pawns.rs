use super::super::attack_tables::PAWN_ATTACKS;
use super::super::{Bitboard, Board, Color, Move, MoveList, Piece, Square, PROMOTION_PIECES};

impl Board {
    /// Add promotion moves for a pawn reaching the back rank
    fn add_promotions(&self, from: Square, to: Square, moves: &mut MoveList) {
        for promo in PROMOTION_PIECES {
            moves.push(self.create_promotion_move(from, to, promo));
        }
    }

    /// Generate pawn capture moves (used by both regular and tactical move generation)
    fn generate_pawn_captures(
        &self,
        from: Square,
        color: Color,
        promotion_rank: usize,
        targets: u64,
        moves: &mut MoveList,
    ) {
        let enemy = self.occupied_by(color.opponent()).0;
        let en_passant = self
            .en_passant_target
            .map_or(0, |square| (1 << square.index()) & !self.all_occupied.0);
        // En passant can remove a checker from a square other than the target.
        let captures = PAWN_ATTACKS[color.index()][from.index()] & ((enemy & targets) | en_passant);
        for to in Bitboard(captures).iter() {
            if enemy & (1 << to.index()) != 0 {
                if to.rank() == promotion_rank {
                    self.add_promotions(from, to, moves);
                } else {
                    moves.push(Move::capture(from, to));
                }
            } else {
                // A FEN target is usable only with the captured pawn present.
                let mv = Move::en_passant(from, to);
                if self.piece_at(mv.capture_square()) == Some((color.opponent(), Piece::Pawn)) {
                    moves.push(mv);
                }
            }
        }
    }

    pub(super) fn generate_pawn_moves(&self, from: Square, moves: &mut MoveList) {
        self.generate_pawn_moves_to(from, u64::MAX, moves);
    }

    pub(super) fn generate_pawn_moves_to(&self, from: Square, targets: u64, moves: &mut MoveList) {
        let color = self.side_to_move();
        let white = color == Color::White;
        let promotion_rank = color.pawn_promotion_rank();

        if let Some(to) = from.forward(white).filter(|&to| self.is_empty(to)) {
            let single_allowed = targets & (1 << to.index()) != 0;
            if to.rank() == promotion_rank {
                if single_allowed {
                    self.add_promotions(from, to, moves);
                }
            } else {
                if single_allowed {
                    moves.push(Move::quiet(from, to));
                }
                if from.rank() == color.pawn_start_rank() {
                    let double = to
                        .forward(white)
                        .expect("pawn double push stays on the board");
                    if targets & (1 << double.index()) != 0 && self.is_empty(double) {
                        moves.push(Move::double_pawn_push(from, double));
                    }
                }
            }
        }

        // Captures (including en passant)
        self.generate_pawn_captures(from, color, promotion_rank, targets, moves);
    }

    pub(crate) fn generate_pawn_tactical_moves(&self, from: Square, moves: &mut MoveList) {
        let color = self.side_to_move();
        let promotion_rank = color.pawn_promotion_rank();
        if let Some(to) = from.forward(color == Color::White) {
            if to.rank() == promotion_rank && self.is_empty(to) {
                self.add_promotions(from, to, moves);
            }
        }

        // Captures (including en passant and capture-promotions)
        self.generate_pawn_captures(from, color, promotion_rank, u64::MAX, moves);
    }
}
