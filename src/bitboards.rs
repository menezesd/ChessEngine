use once_cell::sync::Lazy;
use std::array::from_fn;

use crate::types::{square_index, Bitboard, Color, Square};

#[derive(Clone)]
struct MagicEntry {
    mask: Bitboard,
    magic: Bitboard,
    shift: u8,
    offset: usize,
}

struct MagicTable {
    entries: [MagicEntry; 64],
    attacks: Vec<Bitboard>,
}

impl MagicTable {
    fn attack(&self, square: usize, occupancy: Bitboard) -> Bitboard {
        let entry = &self.entries[square];
        let occ = occupancy & entry.mask;
        let index = if entry.shift == 64 {
            0
        } else {
            (occ.wrapping_mul(entry.magic) >> entry.shift) as usize
        };
        self.attacks[entry.offset + index]
    }
}

fn color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Black => 1,
    }
}

fn bishop_mask(square: usize) -> Bitboard {
    let rank = (square / 8) as i32;
    let file = (square % 8) as i32;
    let mut mask = 0u64;
    let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    for (dr, df) in directions {
        let mut r = rank + dr;
        let mut f = file + df;
        while r >= 0 && r < 8 && f >= 0 && f < 8 {
            if r == 0 || r == 7 || f == 0 || f == 7 {
                break;
            }
            mask |= 1u64 << square_index(Square(r as usize, f as usize));
            r += dr;
            f += df;
        }
    }
    mask
}

fn rook_mask(square: usize) -> Bitboard {
    let rank = (square / 8) as i32;
    let file = (square % 8) as i32;
    let mut mask = 0u64;
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for (dr, df) in directions {
        let mut r = rank + dr;
        let mut f = file + df;
        while r >= 0 && r < 8 && f >= 0 && f < 8 {
            if r == 0 || r == 7 || f == 0 || f == 7 {
                break;
            }
            mask |= 1u64 << square_index(Square(r as usize, f as usize));
            r += dr;
            f += df;
        }
    }
    mask
}

fn bishop_attacks_on_the_fly(square: usize, occupancy: Bitboard) -> Bitboard {
    let rank = (square / 8) as i32;
    let file = (square % 8) as i32;
    let mut attacks = 0u64;
    let directions = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    for (dr, df) in directions {
        let mut r = rank + dr;
        let mut f = file + df;
        while r >= 0 && r < 8 && f >= 0 && f < 8 {
            let sq = square_index(Square(r as usize, f as usize));
            attacks |= 1u64 << sq;
            if occupancy & (1u64 << sq) != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

fn rook_attacks_on_the_fly(square: usize, occupancy: Bitboard) -> Bitboard {
    let rank = (square / 8) as i32;
    let file = (square % 8) as i32;
    let mut attacks = 0u64;
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for (dr, df) in directions {
        let mut r = rank + dr;
        let mut f = file + df;
        while r >= 0 && r < 8 && f >= 0 && f < 8 {
            let sq = square_index(Square(r as usize, f as usize));
            attacks |= 1u64 << sq;
            if occupancy & (1u64 << sq) != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

fn set_occupancy(index: usize, bits: usize, mut mask: Bitboard) -> Bitboard {
    let mut occupancy = 0u64;
    for i in 0..bits {
        let sq = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        if (index & (1 << i)) != 0 {
            occupancy |= 1u64 << sq;
        }
    }
    occupancy
}

#[rustfmt::skip]
const BISHOP_MAGICS: [u64; 64] = [
    0x0040011a02220020, 0x041010020141c020, 0x0010808091010110, 0x0009040900a44040,
    0x0025104010008100, 0x0800825041040882, 0x080a080904120000, 0x9000420084200200,
    0x4001980810008202, 0x021004504c004091, 0x01081000a2104400, 0x0004040400808900,
    0x8048120210803001, 0x5000050120120a00, 0x01004200902c3040, 0x0202248208120210,
    0x04c020920c580082, 0x182000020c040084, 0x0008009002801011, 0x0008221404001348,
    0x5804008e10220603, 0x0204082602010400, 0x0401820108011004, 0x000884050401010f,
    0x00202000080b4420, 0x0028040402100202, 0x0401100081004202, 0x0081180009004100,
    0x0448104008044000, 0x420800800a416002, 0x0400860021a23002, 0x0020a4a321040201,
    0x450918224188a000, 0x81a8080c00080988, 0x0007104810040800, 0x1801400a00022200,
    0x1080c40400804100, 0x0002014100020080, 0x0201880200a10116, 0x2408084140050103,
    0x0501100210002288, 0x4241844402022001, 0x009200202800d420, 0x811c110141040801,
    0x5000603600800410, 0x1020600042810441, 0x00181001020040c0, 0x0030020043190840,
    0x0045093002200094, 0x4222021084050425, 0x802002484c100100, 0x0000141020a80900,
    0x2010104002820410, 0x0404400801010185, 0x00c0500a04d10002, 0x0125280801082082,
    0x0005010450048400, 0x1110120082088202, 0x4212100544040400, 0x4007010011048800,
    0x0c20980014208210, 0x0002004902880200, 0x0004102001040092, 0x5620020200440988,
];

#[rustfmt::skip]
const ROOK_MAGICS: [u64; 64] = [
    0x0081002000004868, 0x4041122014081010, 0x800809100c200a04, 0x0804100128208202,
    0x8104050048820802, 0x00250082220c00c0, 0x8001420100802010, 0x0022c07a00801100,
    0x4008200214214000, 0x90820049018200a0, 0x0001802008100180, 0x0000800800801000,
    0x004100106c480100, 0x00808064002e0080, 0x0801000453000200, 0x4042000b0008e001,
    0x00801a0800020004, 0x90400100410880a0, 0x812c1200228200c0, 0x0002220040100a00,
    0x0042020010040820, 0x8085010044000802, 0x0000240008011082, 0x80c4082010008000,
    0x00000202080001b0, 0x80400800a0007000, 0x4041100080200082, 0x20c1002100100008,
    0x0000080100100500, 0x0a00040080020080, 0x0201000100840200, 0x0240021200000420,
    0x1000100002008010, 0x1041008225004008, 0x1808a001010010c4, 0x04e2092042001200,
    0x1880800800800400, 0x8050820080804400, 0x80f0800300800200, 0x4000000022041060,
    0x0402500000220061, 0x0010004020104000, 0x0926c200a0820014, 0x0400080010008080,
    0x0010080004008080, 0x08820009b0020004, 0x000490080a0c0005, 0x0018042120820004,
    0x4004280004950820, 0x90010184402a0600, 0x0908408204502200, 0x0050880080100080,
    0x0020140080080180, 0x0010800200140080, 0x0014021089080400, 0x0008000000210200,
    0x80c1000000849000, 0x00c020d402100800, 0x0830012014080601, 0x0414021128101082,
    0x6001081c00408202, 0x0421440902220881, 0x8090801c20410200, 0x8008408001080090,
];

fn init_magic_table(is_bishop: bool) -> MagicTable {
    let numbers = if is_bishop {
        &BISHOP_MAGICS
    } else {
        &ROOK_MAGICS
    };

    let mut entries = from_fn(|square| {
        let mask = if is_bishop {
            bishop_mask(square)
        } else {
            rook_mask(square)
        };
        let relevant_bits = mask.count_ones() as u8;
        MagicEntry {
            mask,
            magic: numbers[square],
            shift: 64 - relevant_bits,
            offset: 0,
        }
    });

    let mut attacks: Vec<Bitboard> = Vec::new();
    let mut current_offset = 0usize;

    for square in 0..64 {
        let entry = &mut entries[square];
        entry.offset = current_offset;
        let relevant_bits = 64 - entry.shift as usize;
        let table_size = 1usize << relevant_bits;
        attacks.resize(current_offset + table_size, 0);

        for index in 0..table_size {
            let occupancy = set_occupancy(index, relevant_bits, entry.mask);
            let attack = if is_bishop {
                bishop_attacks_on_the_fly(square, occupancy)
            } else {
                rook_attacks_on_the_fly(square, occupancy)
            };
            let magic_index = if entry.shift == 64 {
                0
            } else {
                (occupancy.wrapping_mul(entry.magic) >> entry.shift) as usize
            };
            attacks[entry.offset + magic_index] = attack;
        }

        current_offset += table_size;
    }

    MagicTable { entries, attacks }
}

static BISHOP_TABLE: Lazy<MagicTable> = Lazy::new(|| init_magic_table(true));
static ROOK_TABLE: Lazy<MagicTable> = Lazy::new(|| init_magic_table(false));

static KNIGHT_ATTACKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut attacks = [0u64; 64];
    for square in 0..64 {
        let rank = (square / 8) as i32;
        let file = (square % 8) as i32;
        let mut attack = 0u64;
        for (dr, df) in [
            (2, 1),
            (1, 2),
            (-1, 2),
            (-2, 1),
            (-2, -1),
            (-1, -2),
            (1, -2),
            (2, -1),
        ] {
            let r = rank + dr;
            let f = file + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                attack |= 1u64 << square_index(Square(r as usize, f as usize));
            }
        }
        attacks[square] = attack;
    }
    attacks
});

static KING_ATTACKS: Lazy<[Bitboard; 64]> = Lazy::new(|| {
    let mut attacks = [0u64; 64];
    for square in 0..64 {
        let rank = (square / 8) as i32;
        let file = (square % 8) as i32;
        let mut attack = 0u64;
        for (dr, df) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            let r = rank + dr;
            let f = file + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                attack |= 1u64 << square_index(Square(r as usize, f as usize));
            }
        }
        attacks[square] = attack;
    }
    attacks
});

static PAWN_ATTACKS: Lazy<[[Bitboard; 64]; 2]> = Lazy::new(|| {
    let mut attacks = [[0u64; 64]; 2];
    for square in 0..64 {
        let rank = (square / 8) as i32;
        let file = (square % 8) as i32;

        // White pawns move towards increasing ranks
        let mut white_attack = 0u64;
        for (dr, df) in [(1, -1), (1, 1)] {
            let r = rank + dr;
            let f = file + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                white_attack |= 1u64 << square_index(Square(r as usize, f as usize));
            }
        }
        attacks[color_index(Color::White)][square] = white_attack;

        let mut black_attack = 0u64;
        for (dr, df) in [(-1, -1), (-1, 1)] {
            let r = rank + dr;
            let f = file + df;
            if (0..8).contains(&r) && (0..8).contains(&f) {
                black_attack |= 1u64 << square_index(Square(r as usize, f as usize));
            }
        }
        attacks[color_index(Color::Black)][square] = black_attack;
    }
    attacks
});

pub fn index_to_square(index: usize) -> Square {
    Square(index / 8, index % 8)
}

pub fn pawn_attacks(color: Color, square: usize) -> Bitboard {
    PAWN_ATTACKS[color_index(color)][square]
}

pub fn knight_attacks(square: usize) -> Bitboard {
    KNIGHT_ATTACKS[square]
}

pub fn king_attacks(square: usize) -> Bitboard {
    KING_ATTACKS[square]
}

pub fn bishop_attacks(square: usize, occupancy: Bitboard) -> Bitboard {
    BISHOP_TABLE.attack(square, occupancy)
}

pub fn rook_attacks(square: usize, occupancy: Bitboard) -> Bitboard {
    ROOK_TABLE.attack(square, occupancy)
}

pub fn queen_attacks(square: usize, occupancy: Bitboard) -> Bitboard {
    bishop_attacks(square, occupancy) | rook_attacks(square, occupancy)
}
