use crate::board::{Color, Piece, Square};

pub type BitBoard = u64;

#[inline]
pub(crate) fn sq_to_bb(sq: Square) -> BitBoard { 1u64 << (sq.0 * 8 + sq.1) }

#[inline]
fn in_bounds(r: isize, f: isize) -> bool { r >= 0 && r < 8 && f >= 0 && f < 8 }

#[inline]
fn rf_to_sq(r: isize, f: isize) -> Square { Square(r as usize, f as usize) }

#[inline]
fn idx_to_sq(idx: usize) -> Square { Square((idx / 8) as usize, (idx % 8) as usize) }

#[inline]
fn popcount(bb: BitBoard) -> u32 { bb.count_ones() }

#[inline]
fn lsb(bb: BitBoard) -> Option<usize> {
    if bb == 0 { None } else { Some(bb.trailing_zeros() as usize) }
}

static KNIGHT_DELTAS: [(isize, isize); 8] = [
    (2, 1), (1, 2), (-1, 2), (-2, 1), (-2, -1), (-1, -2), (1, -2), (2, -1),
];

static KING_DELTAS: [(isize, isize); 8] = [
    (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1),
];

#[inline]
pub(crate) fn knight_attacks_from(sq: Square) -> BitBoard {
    let (r, f) = (sq.0 as isize, sq.1 as isize);
    let mut bb = 0u64;
    for (dr, df) in KNIGHT_DELTAS { let nr = r + dr; let nf = f + df; if in_bounds(nr, nf) { bb |= sq_to_bb(rf_to_sq(nr, nf)); } }
    bb
}

#[inline]
pub(crate) fn king_attacks_from(sq: Square) -> BitBoard {
    let (r, f) = (sq.0 as isize, sq.1 as isize);
    let mut bb = 0u64;
    for (dr, df) in KING_DELTAS { let nr = r + dr; let nf = f + df; if in_bounds(nr, nf) { bb |= sq_to_bb(rf_to_sq(nr, nf)); } }
    bb
}

#[inline]
fn ray_attacks(sq: Square, occ: BitBoard, deltas: &[(isize, isize)]) -> BitBoard {
    let (r0, f0) = (sq.0 as isize, sq.1 as isize);
    let mut attacks = 0u64;
    for &(dr, df) in deltas {
        let mut r = r0 + dr; let mut f = f0 + df;
        while in_bounds(r, f) {
            let nsq = rf_to_sq(r, f);
            let nsq_bb = sq_to_bb(nsq);
            attacks |= nsq_bb;
            if (occ & nsq_bb) != 0 { break; }
            r += dr; f += df;
        }
    }
    attacks
}

#[inline]
pub(crate) fn bishop_attacks_from(sq: Square, occ: BitBoard) -> BitBoard {
    const DIAGS: &[(isize, isize)] = &[(1, 1), (1, -1), (-1, 1), (-1, -1)];
    ray_attacks(sq, occ, DIAGS)
}

#[inline]
pub(crate) fn rook_attacks_from(sq: Square, occ: BitBoard) -> BitBoard {
    const ORTHO: &[(isize, isize)] = &[(1, 0), (-1, 0), (0, 1), (0, -1)];
    ray_attacks(sq, occ, ORTHO)
}

pub fn is_square_attacked_bb(
    squares: &[[Option<(Color, Piece)>; 8]; 8],
    target: Square,
    attacker_color: Color,
) -> bool {
    // Build occupancy and attacker piece bitboards
    let mut occ: BitBoard = 0;
    let mut pawns: BitBoard = 0; let mut knights: BitBoard = 0; let mut bishops: BitBoard = 0; let mut rooks: BitBoard = 0; let mut queens: BitBoard = 0; let mut king: BitBoard = 0;

    for r in 0..8 { for f in 0..8 {
        if let Some((c, p)) = squares[r][f] {
            let bb = 1u64 << (r * 8 + f);
            occ |= bb;
            if c == attacker_color {
                match p {
                    Piece::Pawn => pawns |= bb,
                    Piece::Knight => knights |= bb,
                    Piece::Bishop => bishops |= bb,
                    Piece::Rook => rooks |= bb,
                    Piece::Queen => queens |= bb,
                    Piece::King => king |= bb,
                }
            }
        }
    }}

    let tgt_bb = sq_to_bb(target);

    // Pawn attacks
    // Note: Board uses Square(rank,file) with rank 0 = White back rank, White pawns increase rank by +1 when moving.
    // For attacker_color pawns, generate the squares they attack and test membership.
    let pawn_attacks = if attacker_color == Color::White {
        // White pawns attack (r+1, f±1)
        let mut attacks = 0u64;
        let mut wp = pawns;
        while let Some(idx) = lsb(wp) {
            wp &= wp - 1; // pop lsb
            let sq = idx_to_sq(idx);
            let r = sq.0 as isize; let f = sq.1 as isize;
            for df in [-1, 1] { let nr = r + 1; let nf = f + df; if in_bounds(nr, nf) { attacks |= sq_to_bb(rf_to_sq(nr, nf)); } }
        }
        attacks
    } else {
        // Black pawns attack (r-1, f±1)
        let mut attacks = 0u64;
        let mut bp = pawns;
        while let Some(idx) = lsb(bp) {
            bp &= bp - 1; // pop lsb
            let sq = idx_to_sq(idx);
            let r = sq.0 as isize; let f = sq.1 as isize;
            for df in [-1, 1] { let nr = r - 1; let nf = f + df; if in_bounds(nr, nf) { attacks |= sq_to_bb(rf_to_sq(nr, nf)); } }
        }
        attacks
    };
    if (pawn_attacks & tgt_bb) != 0 { return true; }

    // Knight attacks
    let mut kn = knights;
    while let Some(idx) = lsb(kn) {
        kn &= kn - 1;
        if (knight_attacks_from(idx_to_sq(idx)) & tgt_bb) != 0 { return true; }
    }

    // King attacks
    if king != 0 { let idx = lsb(king).unwrap(); if (king_attacks_from(idx_to_sq(idx)) & tgt_bb) != 0 { return true; } }

    // Sliding: rook/queen orthogonals
    let mut rq = rooks | queens;
    while let Some(idx) = lsb(rq) {
        rq &= rq - 1;
        if (rook_attacks_from(idx_to_sq(idx), occ) & tgt_bb) != 0 { return true; }
    }

    // Sliding: bishop/queen diagonals
    let mut bq = bishops | queens;
    while let Some(idx) = lsb(bq) {
        bq &= bq - 1;
        if (bishop_attacks_from(idx_to_sq(idx), occ) & tgt_bb) != 0 { return true; }
    }

    false
}

// Common file/rank masks for pawn move generation (LSB = a1)
pub(crate) const FILE_A: BitBoard = 0x0101_0101_0101_0101u64;
pub(crate) const FILE_H: BitBoard = 0x8080_8080_8080_8080u64;
pub(crate) const RANK1: BitBoard = 0x0000_0000_0000_00FFu64;
pub(crate) const RANK8: BitBoard = 0xFF00_0000_0000_0000u64;

// Helpers used by movegen to detect intermediate squares for double pushes
#[inline]
pub(crate) fn rank3_mask() -> BitBoard { 0x0000_0000_0000_FF00u64 << 8 } // rank 3
#[inline]
pub(crate) fn rank6_mask() -> BitBoard { 0x00FF_0000_0000_0000u64 >> 8 } // rank 6
