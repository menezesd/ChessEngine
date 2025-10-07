use crate::board::Board;
use crate::types::{Bitboard, Color, Piece, Square};
use crate::zobrist::{color_to_zobrist_index, piece_to_zobrist_index};

// Material constants will be defined below (copied from original board::evaluate)

pub const PUB_PAWN_PST_MG: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 1, 0, -10, -14, -14, -18, -13, 10, 10, -20, -13, -14, -13, -8, -4, -1, 4,
    -15, -11, -8, -5, 3, 10, 2, -8, -16, 2, 7, 7, 11, 26, 20, 10, -7, 13, 17, 23, 16, 24, 45, 25,
    -2, 32, 34, 35, 25, 23, 29, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0,
];
pub const PUB_PAWN_PST_EG: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, -3, -4, 10, 12, 12, 10, -4, -3, -3, 0, -1, 0, 0, -1, 0, -3, 2, 2, -5,
    -14, -14, -5, 2, 2, 19, 11, 2, -13, -13, 2, 11, -7, 46, 40, 22, 5, 5, 22, 40, -2, 52, 63, 43,
    32, 32, 43, 63, 52, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub const P_SUPPORT: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 1, -1, 2, 5, 6, 4, 3, 0, 2, 3, 4, 2, 4, 6, 1, 1, 6, 17, 10, 5, 1,
    1, 3, 4, 8, 14, 15, 9, 4, 3, 7, 9, 10, 12, 12, 10, 9, 7, 9, 10, 12, 14, 14, 12, 10, 9, 0, 0, 0,
    0, 0, 0, 0, 0,
];

// Material values and piece-square tables
pub const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000];
pub const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000];

pub const PST_MG: [[i32; 64]; 6] = [
    [
        0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3, 3, 33,
        -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6, 7, 26, 31, 65,
        56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        -105, -21, -58, -33, -17, -28, -19, -23, -29, -53, -12, -3, -1, 18, -14, -19, -23, -9, 12,
        10, 19, 17, 25, -16, -13, 4, 16, 13, 28, 19, 21, -8, -9, 17, 19, 53, 37, 69, 18, 22, -47,
        60, 37, 65, 84, 129, 73, 44, -73, -41, 72, 36, 23, 62, 7, -17, -167, -89, -34, -49, 61,
        -97, -15, -107,
    ],
    [
        -33, -3, -14, -21, -13, -12, -39, -21, 4, 15, 16, 0, 7, 21, 33, 1, 0, 15, 15, 15, 14, 27,
        18, 10, -6, 13, 13, 26, 34, 12, 10, 4, -4, 5, 19, 50, 37, 37, 7, -2, -16, 37, 43, 40, 35,
        50, 37, -2, -26, 16, -18, -13, 30, 59, 18, -47, -29, 4, -82, -37, -25, -42, 7, -8,
    ],
    [
        -19, -13, 1, 17, 16, 7, -37, -26, -44, -16, -20, -9, -1, 11, -6, -71, -45, -25, -16, -17,
        3, 0, -5, -33, -36, -26, -12, -1, 9, -7, 6, -23, -24, -11, 7, 26, 24, 35, -8, -20, -5, 19,
        26, 36, 17, 45, 61, 16, 27, 32, 58, 62, 80, 67, 26, 44, 32, 42, 32, 51, 63, 9, 31, 43,
    ],
    [
        -1, -18, -9, 10, -15, -25, -31, -50, -35, -8, 11, 2, 8, 15, -3, 1, -14, 2, -11, -2, -5, 2,
        14, 5, -9, -26, -9, -10, -2, -4, 3, -3, -27, -27, -16, -16, -1, 17, -2, 1, -13, -17, 7, 8,
        29, 56, 47, 57, -24, -39, -5, 1, -16, 57, 28, 54, -28, 0, 29, 12, 59, 44, 43, 45,
    ],
    [
        -15, 36, 12, -54, 8, -28, 34, 14, 1, 7, -8, -64, -43, -16, 9, 8, -14, -14, -22, -46, -44,
        -30, -15, -27, -49, -1, -27, -39, -46, -44, -33, -51, -17, -20, -12, -27, -30, -25, -14,
        -36, -9, 24, 2, -16, -20, 6, 22, -22, 29, -1, -20, -7, -8, -4, -38, -29, -65, 23, 16, -15,
        -56, -34, 2, 13,
    ],
];

pub const PST_EG: [[i32; 64]; 6] = [
    [
        0, 0, 0, 0, 0, 0, 0, 0, 13, 8, 8, 10, 13, 0, 2, -7, 4, 7, -6, 1, 0, -5, -1, -8, 13, 9, -3,
        -7, -7, -8, 3, -1, 32, 24, 13, 5, -2, 4, 17, 17, 94, 100, 85, 67, 56, 53, 82, 84, 178, 173,
        158, 134, 147, 132, 165, 187, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        -29, -51, -23, -15, -22, -18, -50, -64, -42, -20, -10, -5, -2, -20, -23, -44, -23, -3, -1,
        15, 10, -3, -20, -22, -18, -6, 16, 25, 16, 17, 4, -18, -17, 3, 22, 22, 22, 11, 8, -18, -24,
        -20, 10, 9, -1, -9, -19, -41, -25, -8, -25, -2, -9, -25, -24, -52, -58, -38, -13, -28, -31,
        -27, -63, -99,
    ],
    [
        -23, -9, -23, -5, -9, -16, -5, -17, -14, -18, -7, -1, 4, -9, -15, -27, -12, -3, 8, 10, 13,
        3, -7, -15, -6, 3, 13, 19, 7, 10, -3, -9, -3, 9, 12, 9, 14, 10, 3, 2, 2, -8, 0, -1, -2, 6,
        0, 4, -8, -4, 7, -12, -3, -13, -4, -14, -14, -21, -11, -8, -7, -9, -17, -24,
    ],
    [
        -9, 2, 3, -1, -5, -13, 4, -20, -6, -6, 0, 2, -9, -9, -11, -3, -4, 0, -5, -1, -7, -12, -8,
        -16, 3, 5, 8, 4, -5, -6, -8, -11, 4, 3, 13, 1, 2, 1, -1, 2, 7, 7, 7, 5, 4, -3, -5, -3, 11,
        13, 13, 11, -3, 3, 8, 3, 13, 10, 18, 15, 12, 12, 8, 5,
    ],
    [
        -33, -28, -22, -43, -5, -32, -20, -41, -22, -23, -30, -16, -16, -23, -36, -32, -16, -27,
        15, 6, 9, 17, 10, 5, -18, 28, 19, 47, 31, 34, 39, 23, 3, 22, 24, 45, 57, 40, 57, 36, -20,
        6, 9, 49, 47, 35, 19, 9, -17, 20, 32, 41, 58, 25, 30, 0, -9, 22, 22, 27, 27, 19, 10, 20,
    ],
    [
        -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, -5, -17, -19, -3, 11, 21,
        23, 16, 7, -9, -18, -4, 21, 24, 27, 23, 9, -11, -8, 22, 24, 27, 26, 33, 26, 3, 10, 17, 23,
        15, 20, 45, 44, 13, -12, 17, 14, 17, 17, 38, 23, 11, -74, -35, -18, -18, -11, 15, 4, -17,
    ],
];

// Public high-level eval: receives pawn MG/EG (from cache or computed) and returns final white-minus-black score
pub fn eval(board: &Board, pawn_mg: i32, pawn_eg: i32) -> i32 {
    let mut mg_score = pawn_mg;
    let mut eg_score = pawn_eg;

    // compute pawn files
    let mut white_pawns_by_file = [0u32; 8];
    let mut black_pawns_by_file = [0u32; 8];
    for file in 0..8 {
        let file_mask = Board::file_mask(file);
        let wp = board.bitboards[color_to_zobrist_index(Color::White)]
            [piece_to_zobrist_index(Piece::Pawn)]
            & file_mask;
        let bp = board.bitboards[color_to_zobrist_index(Color::Black)]
            [piece_to_zobrist_index(Piece::Pawn)]
            & file_mask;
        white_pawns_by_file[file] = wp.count_ones();
        black_pawns_by_file[file] = bp.count_ones();
    }

    // piece material + PST
    for color_idx in 0..2 {
        let color = if color_idx == 0 {
            Color::White
        } else {
            Color::Black
        };
        for piece_idx in 0..6 {
            if piece_idx == piece_to_zobrist_index(Piece::Pawn) {
                continue;
            }
            let mut bb = board.bitboards[color_idx][piece_idx];
            if bb == 0 {
                continue;
            }
            let piece_mg = MATERIAL_MG[piece_idx];
            let piece_eg = MATERIAL_EG[piece_idx];
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                let rank = sq / 8;
                let file = sq % 8;
                let pst_idx = if color == Color::White {
                    (7 - rank) * 8 + file
                } else {
                    rank * 8 + file
                };
                if color == Color::White {
                    mg_score += piece_mg + PST_MG[piece_idx][pst_idx];
                    eg_score += piece_eg + PST_EG[piece_idx][pst_idx];
                } else {
                    mg_score -= piece_mg + PST_MG[piece_idx][pst_idx];
                    eg_score -= piece_eg + PST_EG[piece_idx][pst_idx];
                }
            }
        }
    }

    // bishop pair
    let white_bishops = board.bitboards[color_to_zobrist_index(Color::White)]
        [piece_to_zobrist_index(Piece::Bishop)];
    let black_bishops = board.bitboards[color_to_zobrist_index(Color::Black)]
        [piece_to_zobrist_index(Piece::Bishop)];
    if white_bishops.count_ones() >= 2 {
        mg_score += 30;
    }
    if black_bishops.count_ones() >= 2 {
        mg_score -= 30;
    }

    // rook open/semi-open files and pawn-structure penalties (copied logic)
    for file in 0..8 {
        let fpawns = (white_pawns_by_file[file] + black_pawns_by_file[file]) as i32;
        let white_rooks = board.bitboards[color_to_zobrist_index(Color::White)]
            [piece_to_zobrist_index(Piece::Rook)];
        let black_rooks = board.bitboards[color_to_zobrist_index(Color::Black)]
            [piece_to_zobrist_index(Piece::Rook)];
        let file_mask = Board::file_mask(file);
        if white_rooks & file_mask != 0 {
            if fpawns == 0 {
                mg_score += 15;
            } else if black_pawns_by_file[file] == 0 {
                mg_score += 7;
            }
        }
        if black_rooks & file_mask != 0 {
            if fpawns == 0 {
                mg_score -= 15;
            } else if white_pawns_by_file[file] == 0 {
                mg_score -= 7;
            }
        }

        let wpf = white_pawns_by_file[file] as i32;
        let bpf = black_pawns_by_file[file] as i32;
        if wpf > 0 {
            let left = if file > 0 {
                white_pawns_by_file[file - 1]
            } else {
                0
            };
            let right = if file < 7 {
                white_pawns_by_file[file + 1]
            } else {
                0
            };
            if left == 0 && right == 0 {
                mg_score -= 12;
            }
            if wpf > 1 {
                mg_score -= 12 * (wpf - 1);
            }
        }
        if bpf > 0 {
            let left = if file > 0 {
                black_pawns_by_file[file - 1]
            } else {
                0
            };
            let right = if file < 7 {
                black_pawns_by_file[file + 1]
            } else {
                0
            };
            if left == 0 && right == 0 {
                mg_score += 12;
            }
            if bpf > 1 {
                mg_score += 12 * (bpf - 1);
            }
        }

        // passed pawn detection (approx)
        let mut wpawns_on_file = board.bitboards[color_to_zobrist_index(Color::White)]
            [piece_to_zobrist_index(Piece::Pawn)]
            & file_mask;
        while wpawns_on_file != 0 {
            let sq = wpawns_on_file.trailing_zeros() as usize;
            wpawns_on_file &= wpawns_on_file - 1;
            let rank = sq / 8;
            let mut is_passed = true;
            let file_adj_mask = Board::file_mask(file)
                | if file > 0 {
                    Board::file_mask(file - 1)
                } else {
                    0
                }
                | if file < 7 {
                    Board::file_mask(file + 1)
                } else {
                    0
                };
            let ahead_mask = if rank * 8 >= 64 {
                u64::MAX
            } else {
                (1u64 << (rank * 8)) - 1
            };
            let bb = board.bitboards[color_to_zobrist_index(Color::Black)]
                [piece_to_zobrist_index(Piece::Pawn)];
            if bb & file_adj_mask & ahead_mask != 0 {
                is_passed = false;
            }
            if is_passed {
                let bonus = 10 + (7 - rank as i32) * 7;
                mg_score += bonus;
            }
        }
        let mut bpawns_on_file = board.bitboards[color_to_zobrist_index(Color::Black)]
            [piece_to_zobrist_index(Piece::Pawn)]
            & file_mask;
        while bpawns_on_file != 0 {
            let sq = bpawns_on_file.trailing_zeros() as usize;
            bpawns_on_file &= bpawns_on_file - 1;
            let rank = sq / 8;
            let mut is_passed = true;
            let file_adj_mask = Board::file_mask(file)
                | if file > 0 {
                    Board::file_mask(file - 1)
                } else {
                    0
                }
                | if file < 7 {
                    Board::file_mask(file + 1)
                } else {
                    0
                };
            let ahead_mask = if (rank + 1) * 8 >= 64 {
                0u64
            } else {
                !((1u64 << ((rank + 1) * 8)) - 1)
            };
            let bb = board.bitboards[color_to_zobrist_index(Color::White)]
                [piece_to_zobrist_index(Piece::Pawn)];
            if bb & file_adj_mask & ahead_mask != 0 {
                is_passed = false;
            }
            if is_passed {
                let bonus = 10 + rank as i32 * 7;
                mg_score -= bonus;
            }
        }
    }

    // Collect mobility / king-attack info from piece evaluators and apply
    // additional mobility and king safety adjustments.
    let mut e = EvalData::new();
    // evaluate pieces for both colors to populate EvalData
    for color_idx in 0..2 {
        let color = if color_idx == 0 { Color::White } else { Color::Black };
        eval_knight(board, &mut e, color);
        eval_bishop(board, &mut e, color);
        eval_rook(board, &mut e, color);
        eval_queen(board, &mut e, color);
        eval_king(board, &mut e, color);
    }

    // Apply mobility scores aggregated from piece evaluators. Treat these as
    // middlegame adjustments.
    let w_idx = color_to_zobrist_index(Color::White);
    let b_idx = color_to_zobrist_index(Color::Black);
    mg_score += e.mobility_score[w_idx];
    mg_score -= e.mobility_score[b_idx];

    // King safety: compute a king safety penalty per side using
    // attack-units and pawn-shield information collected in EvalData.
    for color_idx in 0..2 {
        let (mg_pen, eg_pen) = compute_king_safety(board, &e, color_idx);
        if color_idx == w_idx {
            mg_score -= mg_pen;
            eg_score -= eg_pen;
        } else {
            mg_score += mg_pen;
            eg_score += eg_pen;
        }
    }

    // game phase interpolation
    let total_material_mg: i32 = {
        let mut sum = 0i32;
        for idx in 0..6 {
            let white_bb = board.bitboards[0][idx];
            let black_bb = board.bitboards[1][idx];
            let cnt = (white_bb.count_ones() + black_bb.count_ones()) as i32;
            sum += cnt * MATERIAL_MG[idx];
        }
        sum
    };
    let max_material = 2
        * (MATERIAL_MG[1] * 2
            + MATERIAL_MG[2] * 2
            + MATERIAL_MG[3] * 2
            + MATERIAL_MG[4]
            + MATERIAL_MG[0] * 8);
    let mut phase = (total_material_mg as f32) / (max_material as f32);
    phase = phase.clamp(0.0, 1.0);

    let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
    position_score
}

pub const DOUBLED_PAWN_MG: i32 = -9;
pub const DOUBLED_PAWN_EG: i32 = -9;
pub const ISOLATED_PAWN_MG: i32 = -10;
pub const ISOLATED_PAWN_EG: i32 = -18;
pub const ISOLATED_OPEN_MG: i32 = -9;
pub const ISOLATED_OPEN_EG: i32 = 0;
pub const BACKWARD_PAWN_MG: i32 = -2;
pub const BACKWARD_PAWN_EG: i32 = -1;
pub const BACKWARD_OPEN_MG: i32 = -6;
pub const BACKWARD_OPEN_EG: i32 = 0;

// Mobility/misc tables required for piece eval
pub const KNIGHT_MOB: [i32; 9] = [-28, -6, -3, -2, 16, 17, 17, 20, 25];
pub const BISHOP_MOB: [i32; 15] = [-30, -29, -23, -11, -5, 2, 8, 12, 20, 19, 23, 28, 35, 44, 40];
pub const ROOK_MOB: [i32; 15] = [-14, -12, -13, -11, -8, -6, -8, -3, 2, 2, 7, 14, 17, 17, 10];
pub const QUEEN_MOB: [i32; 28] = [
    -14, -13, -12, -30, -28, -17, -11, -2, -6, 0, 3, 1, 1, 1, 4, 6, 5, 0, 3, 2, 7, 3, 6, 2, 15, 21,
    12, 14,
];

// King safety tables / weights (conservative Publius-like approximations)
// weights per attacker piece type (index by piece_to_zobrist_index)
// index order: Pawn, Knight, Bishop, Rook, Queen, King
pub const KING_ATTACK_WEIGHTS: [i32; 6] = [0, 3, 3, 5, 9, 0];

// Distance multipliers for attacker squares relative to the king (Chebyshev)
// index = distance (0..4+)
pub const KING_DIST_MULT: [i32; 5] = [5, 3, 2, 1, 1];

// Attack units -> penalty tables (MG and EG) with more granularity
pub const KING_ATTACK_PEN_MG: [i32; 21] = [
    0, 4, 8, 12, 18, 26, 36, 48, 62, 78, 96, 116, 138, 162, 188, 216, 246, 278, 312, 348, 386,
];
pub const KING_ATTACK_PEN_EG: [i32; 21] = [
    0, 2, 4, 6, 9, 13, 18, 24, 31, 39, 48, 58, 69, 81, 94, 108, 123, 139, 156, 174, 193,
];

pub const SHIELD_MISSING_PEN_MG: i32 = 18;
pub const SHIELD_MISSING_PEN_EG: i32 = 6;

// Caps per piece type to prevent a single piece type from dominating attack units
pub const KING_ATTACK_CAPS: [i32; 6] = [0, 12, 12, 20, 30, 0];

// Compute king safety penalty for the given side (color_idx is 0 for white, 1 for black)
fn compute_king_safety(board: &Board, e: &EvalData, color_idx: usize) -> (i32, i32) {
    let king_idx = piece_to_zobrist_index(Piece::King);
    let kbb = board.bitboards[color_idx][king_idx];
    if kbb == 0 {
        return (0, 0);
    }
    let ks = kbb.trailing_zeros() as usize;
    let king_zone = Board::king_attacks(Square(ks / 8, ks % 8)) | (1u64 << ks);

    // Count attack units from opponent into king zone using distance weighting
    let opp = 1 - color_idx;
    let mut attack_units = 0i32;
    for piece_idx in 0..6 {
        if piece_idx == piece_to_zobrist_index(Piece::King) {
            continue;
        }
        let mut per_piece_units = 0i32;
        let mut attacks = e.control[opp][piece_idx] & king_zone;
        while attacks != 0 {
            let s = attacks.trailing_zeros() as usize;
            attacks &= attacks - 1;
            let dr = (ks as i32 / 8 - s as i32 / 8).abs() as usize;
            let df = (ks as i32 % 8 - s as i32 % 8).abs() as usize;
            let d = dr.max(df).min(4);
            per_piece_units += KING_ATTACK_WEIGHTS[piece_idx] * KING_DIST_MULT[d] as i32;
        }
        // cap per-piece contribution
        let cap = KING_ATTACK_CAPS[piece_idx];
        if cap > 0 && per_piece_units > cap {
            per_piece_units = cap;
        }
        attack_units += per_piece_units;
    }

    // Shield evaluation: count pawns on the 3 shield squares in front of the king
    let file = ks % 8;
    let rank = ks / 8;
    let pawn_idx = piece_to_zobrist_index(Piece::Pawn);
    let mut shield_mask = 0u64;
    if color_idx == color_to_zobrist_index(Color::White) {
        // shield squares: rank+1, files file-1..file+1
        if rank < 7 {
            let r = rank + 1;
            for f in file.saturating_sub(1)..=(file + 1).min(7) {
                shield_mask |= 1u64 << (r * 8 + f);
            }
        }
    } else {
        // black: shield squares are rank-1
        if rank > 0 {
            let r = rank - 1;
            for f in file.saturating_sub(1)..=(file + 1).min(7) {
                shield_mask |= 1u64 << (r * 8 + f);
            }
        }
    }
    let pawns_present = (board.bitboards[color_idx][pawn_idx] & shield_mask).count_ones() as i32;
    let missing = 3 - pawns_present;

    // Lookup attack penalty
    let idx = attack_units.clamp(0, (KING_ATTACK_PEN_MG.len() - 1) as i32) as usize;
    let mut mg_pen = KING_ATTACK_PEN_MG[idx];
    let mut eg_pen = KING_ATTACK_PEN_EG[idx];
    // Add shield missing penalty
    mg_pen += missing * SHIELD_MISSING_PEN_MG;
    eg_pen += missing * SHIELD_MISSING_PEN_EG;

    (mg_pen, eg_pen)
}

// Bitboard helpers local to eval module
fn fill_forward_bits(mut b: u64, color: Color) -> u64 {
    if color == Color::White {
        b |= b << 8;
        b |= b << 16;
        b |= b << 32;
        b << 8
    } else {
        b |= b >> 8;
        b |= b >> 16;
        b |= b >> 32;
        b >> 8
    }
}

pub fn pawn_eval(board: &Board) -> (i32, i32) {
    let mut pmg = 0i32;
    let mut peg = 0i32;

    let white_pawns =
        board.bitboards[color_to_zobrist_index(Color::White)][piece_to_zobrist_index(Piece::Pawn)];
    let black_pawns =
        board.bitboards[color_to_zobrist_index(Color::Black)][piece_to_zobrist_index(Piece::Pawn)];

    for color_idx in 0..2 {
        let color = if color_idx == 0 {
            Color::White
        } else {
            Color::Black
        };
        let own = if color == Color::White {
            white_pawns
        } else {
            black_pawns
        };
        let opp = if color == Color::White {
            black_pawns
        } else {
            white_pawns
        };

        let mut b = own;
        while b != 0 {
            let sq = b.trailing_zeros() as usize;
            b &= b - 1;
            let rank = sq / 8;
            let file = sq % 8;
            let pst_idx = if color == Color::White {
                (7 - rank) * 8 + file
            } else {
                rank * 8 + file
            };

            if color == Color::White {
                pmg += PUB_PAWN_PST_MG[pst_idx];
                peg += PUB_PAWN_PST_EG[pst_idx];
            } else {
                pmg -= PUB_PAWN_PST_MG[pst_idx];
                peg -= PUB_PAWN_PST_EG[pst_idx];
            }

            let paint = 1u64 << sq;
            let front_span = fill_forward_bits(paint, color);
            let is_open = (front_span & opp) == 0;

            if front_span & own != 0 {
                if color == Color::White {
                    pmg += DOUBLED_PAWN_MG;
                    peg += DOUBLED_PAWN_EG;
                } else {
                    pmg -= DOUBLED_PAWN_MG;
                    peg -= DOUBLED_PAWN_EG;
                }
            }

            let base = ((paint & 0xfefefefefefefefe) >> 1) | ((paint & 0x7f7f7f7f7f7f7f7f) << 1);
            let strong_mask = if color == Color::White {
                base | (base >> 8)
            } else {
                base | (base << 8)
            };
            if strong_mask & own != 0 {
                let support_val = P_SUPPORT[sq];
                if color == Color::White {
                    pmg += support_val;
                } else {
                    pmg -= support_val;
                }
            } else {
                let mut adj_mask = 0u64;
                if file > 0 {
                    adj_mask |= Board::file_mask(file - 1);
                }
                if file < 7 {
                    adj_mask |= Board::file_mask(file + 1);
                }
                if (adj_mask & own) == 0 {
                    if color == Color::White {
                        pmg += ISOLATED_PAWN_MG + (is_open as i32) * ISOLATED_OPEN_MG;
                        peg += ISOLATED_PAWN_EG + (is_open as i32) * ISOLATED_OPEN_EG;
                    } else {
                        pmg -= ISOLATED_PAWN_MG + (is_open as i32) * ISOLATED_OPEN_MG;
                        peg -= ISOLATED_PAWN_EG + (is_open as i32) * ISOLATED_OPEN_EG;
                    }
                } else {
                    let support_mask = if color == Color::White {
                        base | (base >> 8)
                    } else {
                        base | (base << 8)
                    };
                    if (support_mask & own) == 0 {
                        if color == Color::White {
                            pmg += BACKWARD_PAWN_MG + (is_open as i32) * BACKWARD_OPEN_MG;
                            peg += BACKWARD_PAWN_EG + (is_open as i32) * BACKWARD_OPEN_EG;
                        } else {
                            pmg -= BACKWARD_PAWN_MG + (is_open as i32) * BACKWARD_OPEN_MG;
                            peg -= BACKWARD_PAWN_EG + (is_open as i32) * BACKWARD_OPEN_EG;
                        }
                    }
                }
            }
        }
    }

    (pmg, peg)
}

// A small EvalData to collect control maps and king attack units
#[derive(Default)]
pub struct EvalData {
    pub control: [[Bitboard; 6]; 2],
    pub all_att: [Bitboard; 2],
    pub king_att_units: [i32; 2],
    pub mobility_score: [i32; 2],
}

impl EvalData {
    pub fn new() -> Self {
        EvalData {
            control: [[0; 6]; 2],
            all_att: [0; 2],
            king_att_units: [0; 2],
            mobility_score: [0; 2],
        }
    }
}

// Compute knight mobility count
fn knight_mobility_count(square: usize, occ: Bitboard) -> usize {
    let sq = Square(square / 8, square % 8);
    let attacks = crate::board::Board::knight_attacks(sq);
    let mobility = attacks & !occ;
    mobility.count_ones() as usize
}

pub fn eval_knight(board: &Board, e: &mut EvalData, color: Color) {
    let color_idx = crate::zobrist::color_to_zobrist_index(color);
    let mut b = board.bitboards[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Knight)];
    let occ = board.all_occupancy;
    while b != 0 {
        let sq = b.trailing_zeros() as usize;
        b &= b - 1;
        // Add basic PST values: reuse board's PST via evaluate? For now use a small material baseline
    // Mobility
    let cnt = knight_mobility_count(sq, occ);
    let add = KNIGHT_MOB[cnt.min(8)];
        // Update control map
        let attacks = crate::board::Board::knight_attacks(Square(sq / 8, sq % 8));
        e.control[crate::zobrist::color_to_zobrist_index(color)]
            [crate::zobrist::piece_to_zobrist_index(Piece::Knight)] |= attacks;
        // King zone attacks handling will be done by external aggregation; here we count attacks to be used later
        e.king_att_units[crate::zobrist::color_to_zobrist_index(color)] +=
            (attacks & e.all_att[crate::zobrist::color_to_zobrist_index(color)]).count_ones()
                as i32;
        e.all_att[crate::zobrist::color_to_zobrist_index(color)] |= attacks;
        // mobility contribution
        e.mobility_score[crate::zobrist::color_to_zobrist_index(color)] += add as i32;
    }
}

// Bishop evaluator: mobility + control accumulation
pub fn eval_bishop(board: &Board, e: &mut EvalData, color: Color) {
    let color_idx = crate::zobrist::color_to_zobrist_index(color);
    let mut b = board.bitboards[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Bishop)];
    let occ = board.all_occupancy;
    while b != 0 {
        let sq = b.trailing_zeros() as usize;
        b &= b - 1;
        let attacks = crate::board::Board::bishop_attacks(Square(sq / 8, sq % 8), occ);
        let mobility = (attacks & !occ).count_ones() as usize;
        let add = BISHOP_MOB[mobility.min(14)];
        e.control[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Bishop)] |= attacks;
        e.all_att[color_idx] |= attacks;
        e.mobility_score[color_idx] += add as i32;
    }
}

// Rook evaluator: mobility + control and simple 7th-rank logic
pub fn eval_rook(board: &Board, e: &mut EvalData, color: Color) {
    let color_idx = crate::zobrist::color_to_zobrist_index(color);
    let mut b = board.bitboards[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Rook)];
    let occ = board.all_occupancy;
    while b != 0 {
        let sq = b.trailing_zeros() as usize;
        b &= b - 1;
        let attacks = crate::board::Board::rook_attacks(Square(sq / 8, sq % 8), occ);
        let mobility = (attacks & !occ).count_ones() as usize;
        let add = ROOK_MOB[mobility.min(14)];
        e.control[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Rook)] |= attacks;
        e.all_att[color_idx] |= attacks;
        e.mobility_score[color_idx] += add as i32;
        // per-rook file bonuses: open / semi-open and 7th-rank activity
        let file = sq % 8;
        let pawn_idx = crate::zobrist::piece_to_zobrist_index(Piece::Pawn);
        let w_pawns_on_file = board.bitboards[color_to_zobrist_index(Color::White)][pawn_idx] & Board::file_mask(file);
        let b_pawns_on_file = board.bitboards[color_to_zobrist_index(Color::Black)][pawn_idx] & Board::file_mask(file);
        // open file bonus
        if w_pawns_on_file == 0 && b_pawns_on_file == 0 {
            // stronger for rooks on open files
            e.mobility_score[color_idx] += 15;
        } else {
            // semi-open: no enemy pawns on file
            if (color == Color::White && b_pawns_on_file == 0) || (color == Color::Black && w_pawns_on_file == 0) {
                e.mobility_score[color_idx] += 7;
            }
        }
        // doubled rooks on same file
        let rooks_on_file = board.bitboards[color_to_zobrist_index(color)][crate::zobrist::piece_to_zobrist_index(Piece::Rook)] & Board::file_mask(file);
        if (rooks_on_file.count_ones() as i32) >= 2 {
            e.mobility_score[color_idx] += 25; // bonus for doubled rooks on file
        }
        // rooks behind passed pawns: if there's a passed pawn of same color on this file and rook is behind it, give bonus
        let mut passed_bonus = 0;
        let mut pawns_on_file = if color == Color::White { w_pawns_on_file } else { b_pawns_on_file };
        while pawns_on_file != 0 {
            let psq = pawns_on_file.trailing_zeros() as usize;
            pawns_on_file &= pawns_on_file - 1;
            // determine if pawn is passed (no opposing pawns ahead on adjacent files)
            let adj = Board::file_mask(file) | if file > 0 { Board::file_mask(file - 1) } else { 0 } | if file < 7 { Board::file_mask(file + 1) } else { 0 };
            let opp_pawns = if color == Color::White {
                board.bitboards[color_to_zobrist_index(Color::Black)][pawn_idx]
            } else {
                board.bitboards[color_to_zobrist_index(Color::White)][pawn_idx]
            };
            let ahead_mask = if color == Color::White {
                if psq / 8 >= 7 { u64::MAX } else { (1u64 << ((psq/8) * 8)) - 1 }
            } else {
                if (psq/8) == 0 { 0u64 } else { !((1u64 << (((psq/8)+1) * 8)) - 1) }
            };
            let is_passed = (opp_pawns & adj & ahead_mask) == 0;
            if is_passed {
                // if rook is behind pawn (closer to own side than pawn), award bonus
                if (color == Color::White && sq / 8 > psq / 8) || (color == Color::Black && sq / 8 < psq / 8) {
                    passed_bonus += 30;
                }
            }
        }
        e.mobility_score[color_idx] += passed_bonus;
        // file depth control: count number of controlled squares on this file in opponent half (rook control)
        let mut depth = 0i32;
        let rook_idx = crate::zobrist::piece_to_zobrist_index(Piece::Rook);
        let file_mask_bits = Board::file_mask(file) & e.control[color_idx][rook_idx];
        let mut fm = file_mask_bits;
        while fm != 0 {
            let s = fm.trailing_zeros() as usize;
            fm &= fm - 1;
            let r = s / 8;
            if (color == Color::White && r >= 4) || (color == Color::Black && r <= 3) {
                depth += 1;
            }
        }
        e.mobility_score[color_idx] += depth * 3;
        // 7th rank bonus when penetrating opponent camp
        let rank = sq / 8;
        if (color == Color::White && rank == 6) || (color == Color::Black && rank == 1) {
            // only award if opponent has few pawns on that file (already checked above), give medium bonus
            e.mobility_score[color_idx] += 20;
        }
    }
}

// Queen evaluator: combined rook+bishop mobility
pub fn eval_queen(board: &Board, e: &mut EvalData, color: Color) {
    let color_idx = crate::zobrist::color_to_zobrist_index(color);
    let mut b = board.bitboards[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Queen)];
    let occ = board.all_occupancy;
    while b != 0 {
        let sq = b.trailing_zeros() as usize;
        b &= b - 1;
        let rook_att = crate::board::Board::rook_attacks(Square(sq / 8, sq % 8), occ);
        let bish_att = crate::board::Board::bishop_attacks(Square(sq / 8, sq % 8), occ);
        let attacks = rook_att | bish_att;
        let mobility = (attacks & !occ).count_ones() as usize;
        let add = QUEEN_MOB[mobility.min(27)];
        e.control[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::Queen)] |= attacks;
        e.all_att[color_idx] |= attacks;
        e.mobility_score[color_idx] += add as i32;
    }
}

// King evaluator: PST + pawn shield and safety heuristics placeholder
pub fn eval_king(board: &Board, e: &mut EvalData, color: Color) {
    let color_idx = crate::zobrist::color_to_zobrist_index(color);
    let mut b = board.bitboards[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::King)];
    while b != 0 {
        let sq = b.trailing_zeros() as usize;
        b &= b - 1;
        // count king attacks control and accumulate
        let attacks = crate::board::Board::king_attacks(Square(sq / 8, sq % 8));
        e.control[color_idx][crate::zobrist::piece_to_zobrist_index(Piece::King)] |= attacks;
        e.all_att[color_idx] |= attacks;
        // placeholder: small penalty for being in center early on
        e.king_att_units[color_idx] -= 5;
    }
}
