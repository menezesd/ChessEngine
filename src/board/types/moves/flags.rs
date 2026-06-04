use crate::board::Piece;

// Move flags (4 bits, values 0-15).
pub(super) const QUIET: u16 = 0;
pub(super) const DOUBLE_PAWN: u16 = 1;
pub(super) const CASTLE_KINGSIDE: u16 = 2;
pub(super) const CASTLE_QUEENSIDE: u16 = 3;
pub(super) const CAPTURE: u16 = 4;
pub(super) const EN_PASSANT: u16 = 5;
// 6-7 reserved.
pub(super) const PROMO_KNIGHT: u16 = 8;
pub(super) const PROMO_BISHOP: u16 = 9;
pub(super) const PROMO_ROOK: u16 = 10;
pub(super) const PROMO_QUEEN: u16 = 11;
pub(super) const PROMO_CAPTURE_KNIGHT: u16 = 12;
pub(super) const PROMO_CAPTURE_BISHOP: u16 = 13;
pub(super) const PROMO_CAPTURE_ROOK: u16 = 14;
pub(super) const PROMO_CAPTURE_QUEEN: u16 = 15;

pub(super) const fn promotion(piece: Piece) -> u16 {
    match piece {
        Piece::Knight => PROMO_KNIGHT,
        Piece::Bishop => PROMO_BISHOP,
        Piece::Rook => PROMO_ROOK,
        _ => PROMO_QUEEN,
    }
}

pub(super) const fn promotion_capture(piece: Piece) -> u16 {
    match piece {
        Piece::Knight => PROMO_CAPTURE_KNIGHT,
        Piece::Bishop => PROMO_CAPTURE_BISHOP,
        Piece::Rook => PROMO_CAPTURE_ROOK,
        _ => PROMO_CAPTURE_QUEEN,
    }
}

pub(super) const fn promotion_piece(flag: u16) -> Option<Piece> {
    match flag {
        PROMO_KNIGHT | PROMO_CAPTURE_KNIGHT => Some(Piece::Knight),
        PROMO_BISHOP | PROMO_CAPTURE_BISHOP => Some(Piece::Bishop),
        PROMO_ROOK | PROMO_CAPTURE_ROOK => Some(Piece::Rook),
        PROMO_QUEEN | PROMO_CAPTURE_QUEEN => Some(Piece::Queen),
        _ => None,
    }
}
