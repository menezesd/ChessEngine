use once_cell::sync::Lazy;
use rand::Rng;

// --- Zobrist Hashing ---

pub static ZOBRIST_PIECE_SQUARE: Lazy<[[[u64; 64]; 6]; 2]> = Lazy::new(|| {
    let mut rng = rand::thread_rng();
    let mut table = [[[0u64; 64]; 6]; 2];
    for color in 0..2 {
        for piece in 0..6 {
            for sq in 0..64 {
                table[color][piece][sq] = rng.gen();
            }
        }
    }
    table
});

pub static ZOBRIST_CASTLING: Lazy<[u64; 16]> = Lazy::new(|| {
    let mut rng = rand::thread_rng();
    let mut table = [0u64; 16];
    for i in 0..16 {
        table[i] = rng.gen();
    }
    table
});

pub static ZOBRIST_EN_PASSANT: Lazy<[u64; 8]> = Lazy::new(|| {
    let mut rng = rand::thread_rng();
    let mut table = [0u64; 8];
    for i in 0..8 {
        table[i] = rng.gen();
    }
    table
});

pub static ZOBRIST_SIDE_TO_MOVE: Lazy<u64> = Lazy::new(|| rand::thread_rng().gen());

pub fn zobrist_piece(color: usize, piece: usize, sq: usize) -> u64 {
    ZOBRIST_PIECE_SQUARE[color][piece][sq]
}

pub fn zobrist_castling(castling_rights: u8) -> u64 {
    ZOBRIST_CASTLING[castling_rights as usize]
}

pub fn zobrist_en_passant(sq: Option<usize>) -> u64 {
    match sq {
        Some(sq) => ZOBRIST_EN_PASSANT[sq % 8],
        None => 0,
    }
}

pub fn zobrist_side_to_move(color: usize) -> u64 {
    if color == 0 {
        0
    } else {
        *ZOBRIST_SIDE_TO_MOVE
    }
}
