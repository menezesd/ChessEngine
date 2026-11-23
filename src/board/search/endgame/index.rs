pub const PAWN_POSITION_COUNT: usize = 6 * 4 * 64 * 64 * 2;
pub const MAJOR_POSITION_COUNT: usize = 4 * 4 * 64 * 64 * 2;
pub const THREE_PIECE_LENGTH: usize = PAWN_POSITION_COUNT * 2 + MAJOR_POSITION_COUNT * 2;
pub const KNIGHT_PAIR_DISTANCE_BITS: u32 = 7;
pub const BISHOP_KNIGHT_BLOCK_SIZE: usize = 64;
pub const BISHOP_PAIR_BLOCK_SIZE: usize = 512;
pub const KING_ANCHORS: [usize; 10] = [0, 8, 9, 16, 17, 18, 24, 25, 26, 27];
pub const ROOK_POSITION_COUNT: usize = KING_ANCHORS.len() * 64 * 64 * 64 * 2;
pub const ROOK_KNIGHT_BLOCK_SIZE: usize = 64;
pub const ROOK_BISHOP_BLOCK_SIZE: usize = 64;
pub const ROOK_ROOK_BLOCK_SIZE: usize = 256;
pub const ROOK_QUEEN_BLOCK_SIZE: usize = 128;
pub const ROOK_PAWN_BLOCK_SIZE: usize = 64;
pub const ROOK_PAWN_STATES_PER_RANK: usize = 4 * 64 * 64 * 64 * 2;
pub const ROOK_PAWN_POSITION_COUNT: usize = 6 * ROOK_PAWN_STATES_PER_RANK;
pub const NO_WIN: u8 = u8::MAX;
pub const CLOCK_STATE_SHIFT: usize = 14;
pub const CLOCK_DISTANCE_MASK: u32 = 0x7f;

pub const fn clock_entry(state: usize, budget: u8, mate: u8) -> u32 {
    ((state as u32) << CLOCK_STATE_SHIFT) | ((budget as u32) << 7) | mate as u32
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MinorMaterial {
    BishopKnight,
    BishopPair,
    KnightPair,
}

impl MinorMaterial {
    const fn positions_per_anchor(self) -> usize {
        let full_board = 64 * 64 * 64;
        match self {
            Self::BishopKnight | Self::KnightPair => full_board,
            Self::BishopPair => full_board / 4,
        }
    }

    pub const fn position_count(self) -> usize {
        KING_ANCHORS.len() * self.positions_per_anchor() * 2
    }
}

#[derive(Clone, Copy)]
pub struct MinorPosition {
    pub defender: usize,
    pub king: usize,
    pub first: usize,
    pub second: usize,
    pub attacking: bool,
}

pub const fn square_color(square: usize) -> usize {
    (square ^ (square / 8)) & 1
}

const fn transform(square: usize, flip: usize, transpose: bool) -> usize {
    let square = square ^ flip;
    if transpose {
        (square % 8) * 8 + square / 8
    } else {
        square
    }
}

pub const fn palette_width(count: u8) -> usize {
    if count <= 1 {
        0
    } else {
        (count - 1).ilog2() as usize + 1
    }
}

fn anchored_index(
    anchor: usize,
    positions_per_anchor: usize,
    pieces_index: impl Fn(usize, bool) -> usize,
) -> usize {
    let flip = (if anchor % 8 >= 4 { 7 } else { 0 }) | (if anchor / 8 >= 4 { 56 } else { 0 });
    let anchor = anchor ^ flip;
    let (rank, file) = (anchor / 8, anchor % 8);
    let high = rank.max(file);
    let anchor_index = high * (high + 1) / 2 + rank.min(file);
    let pieces = pieces_index(flip, file > rank);
    let pieces = if rank == file {
        pieces.min(pieces_index(flip, true))
    } else {
        pieces
    };
    anchor_index * positions_per_anchor + pieces
}

impl MinorPosition {
    /// Normalize the defending king into the triangle a1-d4. On its
    /// diagonal, use the diagonal reflection that packs smaller piece indices.
    pub fn index(self, material: MinorMaterial) -> usize {
        anchored_index(
            self.defender,
            material.positions_per_anchor(),
            |flip, transpose| self.pieces_index(material, flip, transpose),
        ) * 2
            + usize::from(self.attacking)
    }

    fn pieces_index(self, material: MinorMaterial, flip: usize, transpose: bool) -> usize {
        let king = transform(self.king, flip, transpose);
        let first = transform(self.first, flip, transpose);
        let second = transform(self.second, flip, transpose);
        if material == MinorMaterial::BishopPair {
            // Each bishop has 32 possible squares. Store the dark bishop first.
            let (dark, light) = if square_color(first) == 0 {
                (first, second)
            } else {
                (second, first)
            };
            (king * 32 + dark / 2) * 32 + light / 2
        } else if material == MinorMaterial::KnightPair {
            (king * 64 + first.min(second)) * 64 + first.max(second)
        } else {
            (king * 64 + first) * 64 + second
        }
    }
}

#[derive(Clone, Copy)]
pub struct RookPosition {
    /// The opposing side and the rook side occupy indices zero and one.
    pub kings: [usize; 2],
    pub pieces: [usize; 2],
    pub turn: usize,
}

impl RookPosition {
    pub fn index(self) -> usize {
        anchored_index(self.kings[0], 64 * 64 * 64, |flip, transpose| {
            let king = transform(self.kings[1], flip, transpose);
            let rook = transform(self.pieces[1], flip, transpose);
            let other = transform(self.pieces[0], flip, transpose);
            (king * 64 + rook) * 64 + other
        }) * 2
            + self.turn
    }
}

/// Squares are normalized to a black pawn. The pawn side is first.
#[derive(Clone, Copy)]
pub struct RookPawnPosition {
    pub kings: [usize; 2],
    pub pawn: usize,
    pub rook: usize,
    pub turn: usize,
}

impl RookPawnPosition {
    pub fn index(self) -> usize {
        let flip = if self.pawn % 8 >= 4 { 7 } else { 0 };
        let pawn = (self.pawn ^ flip) - 8;
        let pawn_slot = pawn / 8 * 4 + pawn % 8;
        (((pawn_slot * 64 + (self.kings[0] ^ flip)) * 64 + (self.kings[1] ^ flip)) * 64
            + (self.rook ^ flip))
            * 2
            + self.turn
    }
}

const fn index(piece_index: usize, king: usize, defender: usize, attacking: bool) -> usize {
    (((piece_index * 64) + king) * 64 + defender) * 2 + attacking as usize
}

/// Normalize to a white pawn on files a-d before indexing.
pub const fn pawn_index(king: usize, defender: usize, pawn: usize, attacking: bool) -> usize {
    let flip = if pawn % 8 >= 4 { 7 } else { 0 };
    let pawn = pawn ^ flip;
    let piece_index = (pawn / 8 - 1) * 4 + pawn % 8;
    index(piece_index, king ^ flip, defender ^ flip, attacking)
}

/// Normalize a rook or queen into the lower-left quadrant before indexing.
pub const fn major_index(king: usize, defender: usize, major: usize, attacking: bool) -> usize {
    let flip = (if major % 8 >= 4 { 7 } else { 0 }) | (if major / 8 >= 4 { 56 } else { 0 });
    let major = major ^ flip;
    let piece_index = (major / 8) * 4 + major % 8;
    index(piece_index, king ^ flip, defender ^ flip, attacking)
}
