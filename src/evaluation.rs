use crate::bitboard;
use crate::board::Board;
use crate::types::*;

// MATE_SCORE is used for mate detection
pub const MATE_SCORE: i32 = 100000;

// Simple material values (used for old PST tables)
const PAWN_VALUE: i32 = 100;
const KNIGHT_VALUE: i32 = 320;
const BISHOP_VALUE: i32 = 330;
const ROOK_VALUE: i32 = 500;
const QUEEN_VALUE: i32 = 900;
const KING_VALUE: i32 = 20000;

// Old piece-square tables (not currently used in evaluate(), but kept for compatibility)
#[allow(dead_code)]
const PAWN_PST: [i32; 64] = [
    0,   0,   0,   0,   0,   0,   0,   0,
    50,  50,  50,  50,  50,  50,  50,  50,
    10,  10,  20,  30,  30,  20,  10,  10,
    5,   5,   10,  25,  25,  10,  5,   5,
    0,   0,   0,   20,  20,   0,   0,   0,
    5,   -5,  -10,  0,   0,   -10, -5,  5,
    5,   10,  10,  -20, -20,  10,  10,  5,
    0,   0,   0,   0,   0,   0,   0,   0
];

#[allow(dead_code)]
const KNIGHT_PST: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,  0,   0,   0,   0,   -20, -40,
    -30,  0,   10,  15,  15,  10,  0,   -30,
    -30,  5,   15,  20,  20,  15,  5,   -30,
    -30,  0,   15,  20,  20,  15,  0,   -30,
    -30,  5,   10,  15,  15,  10,  5,   -30,
    -40, -20,  0,   5,   5,   0,   -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50
];

#[allow(dead_code)]
const BISHOP_PST: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,  0,   0,   0,   0,   0,   0,   -10,
    -10,  0,   5,   10,  10,  5,   0,   -10,
    -10,  5,   5,   10,  10,  5,   5,   -10,
    -10,  0,   10,  10,  10,  10,  0,   -10,
    -10,  10,  10,  10,  10,  10,  10,  -10,
    -10,  5,   0,   0,   0,   0,   5,   -10,
    -20, -10, -10, -10, -10, -10, -10, -20
];

#[allow(dead_code)]
const ROOK_PST: [i32; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,
    5,  10, 10, 10, 10, 10, 10, 5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    -5, 0,  0,  0,  0,  0,  0,  -5,
    0,  0,  0,  5,  5,  0,  0,  0
];

#[allow(dead_code)]
const QUEEN_PST: [i32; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20,
    -10,  0,   0,   0,  0,   0,   0,   -10,
    -10,  0,   5,   5,  5,   5,   0,   -10,
    -5,   0,   5,   5,  5,   5,   0,   -5,
    0,    0,   5,   5,  5,   5,   0,   -5,
    -10,  5,   5,   5,  5,   5,   0,   -10,
    -10,  0,   5,   0,  0,   0,   0,   -10,
    -20, -10, -10, -5, -5, -10, -10, -20
];

#[allow(dead_code)]
const KING_PST: [i32; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -10, -20, -20, -20, -20, -20, -20, -10,
    20,  20,   0,   0,   0,   0,  20,  20,
    20,  30,  10,  0,   0,  10,  30,  20
];

/// Get simple material value of a piece
pub fn piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => PAWN_VALUE,
        Piece::Knight => KNIGHT_VALUE,
        Piece::Bishop => BISHOP_VALUE,
        Piece::Rook => ROOK_VALUE,
        Piece::Queen => QUEEN_VALUE,
        Piece::King => KING_VALUE,
    }
}

/// Most Valuable Victim - Least Valuable Attacker score for move ordering
pub fn mvv_lva_score(attacker: Piece, victim: Piece) -> i32 {
    piece_value(victim) - piece_value(attacker) / 100
}

/// Get piece-square table value for a piece at a square (old tables, not currently used)
#[allow(dead_code)]
fn pst_value(piece: Piece, sq: usize, color: usize) -> i32 {
    let table_sq = if color == 0 { sq } else { sq ^ 56 }; // Flip for black
    match piece {
        Piece::Pawn => PAWN_PST[table_sq],
        Piece::Knight => KNIGHT_PST[table_sq],
        Piece::Bishop => BISHOP_PST[table_sq],
        Piece::Rook => ROOK_PST[table_sq],
        Piece::Queen => QUEEN_PST[table_sq],
        Piece::King => KING_PST[table_sq],
    }
}

/// Main evaluation function - returns score in centipawns from current player's perspective
pub fn evaluate(board: &mut Board) -> i32 {
    let mut score = 0;

    // Material values for middlegame and endgame (from d08a060)
    const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000]; // P, N, B, R, Q, K
    const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000]; // P, N, B, R, Q, K

    // Piece-square tables (middlegame) - from d08a060
    const PST_MG: [[i32; 64]; 6] = [
        // Pawn
        [
            0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3,
            3, 33, -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6,
            7, 26, 31, 65, 56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0,
            0,
        ],
        // Knight
        [
            -105, -21, -58, -33, -17, -28, -19, -23, -29, -53, -12, -3, -1, 18, -14, -19, -23,
            -9, 12, 10, 19, 17, 25, -16, -13, 4, 16, 13, 28, 19, 21, -8, -9, 17, 19, 53, 37,
            69, 18, 22, -47, 60, 37, 65, 84, 129, 73, 44, -73, -41, 72, 36, 23, 62, 7, -17,
            -167, -89, -34, -49, 61, -97, -15, -107,
        ],
        // Bishop
        [
            -33, -3, -14, -21, -13, -12, -39, -21, 4, 15, 16, 0, 7, 21, 33, 1, 0, 15, 15, 15,
            14, 27, 18, 10, -6, 13, 13, 26, 34, 12, 10, 4, -4, 5, 19, 50, 37, 37, 7, -2, -16,
            37, 43, 40, 35, 50, 37, -2, -26, 16, -18, -13, 30, 59, 18, -47, -29, 4, -82, -37,
            -25, -42, 7, -8,
        ],
        // Rook
        [
            -19, -13, 1, 17, 16, 7, -37, -26, -44, -16, -20, -9, -1, 11, -6, -71, -45, -25,
            -16, -17, 3, 0, -5, -33, -36, -26, -12, -1, 9, -7, 6, -23, -24, -11, 7, 26, 24, 35,
            -8, -20, -5, 19, 26, 36, 17, 45, 61, 16, 27, 32, 58, 62, 80, 67, 26, 44, 32, 42,
            32, 51, 63, 9, 31, 43,
        ],
        // Queen
        [
            -1, -18, -9, 10, -15, -25, -31, -50, -35, -8, 11, 2, 8, 15, -3, 1, -14, 2, -11, -2,
            -5, 2, 14, 5, -9, -26, -9, -10, -2, -4, 3, -3, -27, -27, -16, -16, -1, 17, -2, 1,
            -13, -17, 7, 8, 29, 56, 47, 57, -24, -39, -5, 1, -16, 57, 28, 54, -28, 0, 29, 12,
            59, 44, 43, 45,
        ],
        // King
        [
            -15, 36, 12, -54, 8, -28, 34, 14, 1, 7, -8, -64, -43, -16, 9, 8, -14, -14, -22,
            -46, -44, -30, -15, -27, -49, -1, -27, -39, -46, -44, -33, -51, -17, -20, -12, -27,
            -30, -25, -14, -36, -9, 24, 2, -16, -20, 6, 22, -22, 29, -1, -20, -7, -8, -4, -38,
            -29, -65, 23, 16, -15, -56, -34, 2, 13,
        ],
    ];

    // Piece-square tables (endgame) - from d08a060
    const PST_EG: [[i32; 64]; 6] = [
        // Pawn
        [
            0, 0, 0, 0, 0, 0, 0, 0, 13, 8, 8, 10, 13, 0, 2, -7, 4, 7, -6, 1, 0, -5, -1, -8, 13,
            9, -3, -7, -7, -8, 3, -1, 32, 24, 13, 5, -2, 4, 17, 17, 94, 100, 85, 67, 56, 53,
            82, 84, 178, 173, 158, 134, 147, 132, 165, 187, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        // Knight
        [
            -29, -51, -23, -15, -22, -18, -50, -64, -42, -20, -10, -5, -2, -20, -23, -44, -23,
            -3, -1, 15, 10, -3, -20, -22, -18, -6, 16, 25, 16, 17, 4, -18, -17, 3, 22, 22, 22,
            11, 8, -18, -24, -20, 10, 9, -1, -9, -19, -41, -25, -8, -25, -2, -9, -25, -24, -52,
            -58, -38, -13, -28, -31, -27, -63, -99,
        ],
        // Bishop
        [
            -23, -9, -23, -5, -9, -16, -5, -17, -14, -18, -7, -1, 4, -9, -15, -27, -12, -3, 8,
            10, 13, 3, -7, -15, -6, 3, 13, 19, 7, 10, -3, -9, -3, 9, 12, 9, 14, 10, 3, 2, 2,
            -8, 0, -1, -2, 6, 0, 4, -8, -4, 7, -12, -3, -13, -4, -14, -14, -21, -11, -8, -7,
            -9, -17, -24,
        ],
        // Rook
        [
            -9, 2, 3, -1, -5, -13, 4, -20, -6, -6, 0, 2, -9, -9, -11, -3, -4, 0, -5, -1, -7,
            -12, -8, -16, 3, 5, 8, 4, -5, -6, -8, -11, 4, 3, 13, 1, 2, 1, -1, 2, 7, 7, 7, 5, 4,
            -3, -5, -3, 11, 13, 13, 11, -3, 3, 8, 3, 13, 10, 18, 15, 12, 12, 8, 5,
        ],
        // Queen
        [
            -33, -28, -22, -43, -5, -32, -20, -41, -22, -23, -30, -16, -16, -23, -36, -32, -16,
            -27, 15, 6, 9, 17, 10, 5, -18, 28, 19, 47, 31, 34, 39, 23, 3, 22, 24, 45, 57, 40,
            57, 36, -20, 6, 9, 49, 47, 35, 19, 9, -17, 20, 32, 41, 58, 25, 30, 0, -9, 22, 22,
            27, 27, 19, 10, 20,
        ],
        // King
        [
            -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, -5, -17, -19, -3,
            11, 21, 23, 16, 7, -9, -18, -4, 21, 24, 27, 23, 9, -11, -8, 22, 24, 27, 26, 33, 26,
            3, 10, 17, 23, 15, 20, 45, 44, 13, -12, 17, 14, 17, 17, 38, 23, 11, -74, -35, -18,
            -18, -11, 15, 4, -17,
        ],
    ];

    // Count pieces for game phase detection and evaluation features
    let mut white_material_mg = 0;
    let mut black_material_mg = 0;
    let mut _white_material_eg = 0;
    let mut _black_material_eg = 0;
    let mut white_bishop_count = 0;
    let mut black_bishop_count = 0;
    let mut white_pawns_by_file = [0; 8];
    let mut black_pawns_by_file = [0; 8];
    let mut _white_king_pos = (0, 0);
    let mut _black_king_pos = (0, 0);

    // First pass: Count pieces and positions using bitboards
    for color_idx in 0..2 {
        for piece_idx in 0..6 {
            let piece_bb = board.pieces[color_idx][piece_idx];
            let piece = match piece_idx {
                0 => Piece::Pawn,
                1 => Piece::Knight,
                2 => Piece::Bishop,
                3 => Piece::Rook,
                4 => Piece::Queen,
                5 => Piece::King,
                _ => continue,
            };

            let mut bb = piece_bb;
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                let rank = sq / 8;
                let file = sq % 8;
                bb &= bb - 1;

                if color_idx == 0 {
                    if piece == Piece::Bishop {
                        white_bishop_count += 1;
                    } else if piece == Piece::King {
                        _white_king_pos = (rank, file);
                    } else if piece == Piece::Pawn {
                        white_pawns_by_file[file] += 1;
                    }

                    white_material_mg += MATERIAL_MG[piece_idx];
                    _white_material_eg += MATERIAL_EG[piece_idx];
                } else {
                    if piece == Piece::Bishop {
                        black_bishop_count += 1;
                    } else if piece == Piece::King {
                        _black_king_pos = (rank, file);
                    } else if piece == Piece::Pawn {
                        black_pawns_by_file[file] += 1;
                    }

                    black_material_mg += MATERIAL_MG[piece_idx];
                    _black_material_eg += MATERIAL_EG[piece_idx];
                }
            }
        }
    }

    // Calculate game phase based on remaining material
    let total_material_mg = white_material_mg + black_material_mg;
    let max_material = 2 * (MATERIAL_MG[1] * 2 + MATERIAL_MG[2] * 2 + MATERIAL_MG[3] * 2 + MATERIAL_MG[4] + MATERIAL_MG[0] * 8);
    let phase = (total_material_mg as f32) / (max_material as f32);
    let phase = phase.min(1.0).max(0.0);

    // Second pass: Evaluate pieces with position
    let mut mg_score = 0;
    let mut eg_score = 0;

    for color_idx in 0..2 {
        for piece_idx in 0..6 {
            let piece_bb = board.pieces[color_idx][piece_idx];
            let mut bb = piece_bb;
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;

                // Get 1D index for piece square tables
                let sq_idx = if color_idx == 0 {
                    sq ^ 56 // White pieces are flipped vertically (7-rank * 8 + file)
                } else {
                    sq // Black pieces use the table as-is
                };

                let mg_value = MATERIAL_MG[piece_idx] + PST_MG[piece_idx][sq_idx];
                let eg_value = MATERIAL_EG[piece_idx] + PST_EG[piece_idx][sq_idx];

                if color_idx == 0 {
                    mg_score += mg_value;
                    eg_score += eg_value;
                } else {
                    mg_score -= mg_value;
                    eg_score -= eg_value;
                }
            }
        }
    }

    // Interpolate between middlegame and endgame scores based on phase
    let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
    score += position_score;

    // Additional evaluation factors

    // 1. Bishop pair bonus
    if white_bishop_count >= 2 {
        score += 30;
    }
    if black_bishop_count >= 2 {
        score -= 30;
    }

    // 2. Rook on open files
    for file in 0..8 {
        let white_rooks_on_file = (board.pieces[0][3] & bitboard::file_mask(file)) != 0;
        let black_rooks_on_file = (board.pieces[1][3] & bitboard::file_mask(file)) != 0;

        if white_rooks_on_file || black_rooks_on_file {
            let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];

            if file_pawns == 0 {
                // Open file
                let bonus = 15;
                if white_rooks_on_file {
                    score += bonus;
                }
                if black_rooks_on_file {
                    score -= bonus;
                }
            } else if (white_rooks_on_file && black_pawns_by_file[file] == 0)
                || (black_rooks_on_file && white_pawns_by_file[file] == 0) {
                // Semi-open file
                let bonus = 7;
                if white_rooks_on_file {
                    score += bonus;
                }
                if black_rooks_on_file {
                    score -= bonus;
                }
            }
        }
    }

    // 3. Pawn structure
    for file in 0..8 {
        // Isolated pawns
        if white_pawns_by_file[file] > 0 {
            let left_file = if file > 0 { white_pawns_by_file[file - 1] } else { 0 };
            let right_file = if file < 7 { white_pawns_by_file[file + 1] } else { 0 };

            if left_file == 0 && right_file == 0 {
                score -= 12; // Isolated pawn penalty
            }
        }

        if black_pawns_by_file[file] > 0 {
            let left_file = if file > 0 { black_pawns_by_file[file - 1] } else { 0 };
            let right_file = if file < 7 { black_pawns_by_file[file + 1] } else { 0 };

            if left_file == 0 && right_file == 0 {
                score += 12; // Isolated pawn penalty for black
            }
        }

        // Doubled pawns penalty
        if white_pawns_by_file[file] > 1 {
            score -= 12 * (white_pawns_by_file[file] - 1);
        }
        if black_pawns_by_file[file] > 1 {
            score += 12 * (black_pawns_by_file[file] - 1);
        }

        // Passed pawns (simplified version)
        // This is a simplified implementation - the original had more sophisticated passed pawn detection
        for rank in 0..8 {
            let sq = rank * 8 + file;
            let white_pawn_here = (board.pieces[0][0] & (1u64 << sq)) != 0;
            let black_pawn_here = (board.pieces[1][0] & (1u64 << sq)) != 0;

            if white_pawn_here {
                let mut is_passed = true;
                // Check if there are any black pawns ahead on same or adjacent files
                for check_rank in 0..rank {
                    for df in -1i32..=1 {
                        let check_file = file as i32 + df;
                        if check_file >= 0 && check_file < 8 {
                            let check_sq = check_rank * 8 + check_file as usize;
                            if (board.pieces[1][0] & (1u64 << check_sq)) != 0 {
                                is_passed = false;
                                break;
                            }
                        }
                    }
                    if !is_passed { break; }
                }

                if is_passed {
                    let bonus = 10 + (7 - rank as i32) * 7;
                    score += bonus;
                }
            }

            if black_pawn_here {
                let mut is_passed = true;
                // Check if there are any white pawns ahead on same or adjacent files
                for check_rank in (rank + 1)..8 {
                    for df in -1i32..=1 {
                        let check_file = file as i32 + df;
                        if check_file >= 0 && check_file < 8 {
                            let check_sq = check_rank * 8 + check_file as usize;
                            if (board.pieces[0][0] & (1u64 << check_sq)) != 0 {
                                is_passed = false;
                                break;
                            }
                        }
                    }
                    if !is_passed { break; }
                }

                if is_passed {
                    let bonus = 10 + rank as i32 * 7;
                    score -= bonus;
                }
            }
        }
    }

    // Return score relative to the current player to move
    if board.white_to_move {
        score
    } else {
        -score
    }
}

/// Quiescence search - only searches captures and promotions to avoid horizon effect
pub fn quiescence(board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
    let stand_pat = evaluate(board);

    // Stand pat: if current position is good enough, return it
    if stand_pat >= beta {
        return beta;
    }
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    let moves = board.generate_moves();
    let mut captures = Vec::new();

    // Collect only captures and promotions
    for m in moves {
        if m.captured_piece.is_some() || m.promotion.is_some() {
            captures.push(m);
        }
    }

    // Sort captures by MVV-LVA for better ordering
    captures.sort_by(|a, b| {
        let a_score = if let Some(victim) = a.captured_piece {
            if let Some(attacker) = board.piece_at(a.from.0 * 8 + a.from.1) {
                mvv_lva_score(attacker.1, victim)
            } else {
                0
            }
        } else {
            0
        };

        let b_score = if let Some(victim) = b.captured_piece {
            if let Some(attacker) = board.piece_at(b.from.0 * 8 + b.from.1) {
                mvv_lva_score(attacker.1, victim)
            } else {
                0
            }
        } else {
            0
        };

        b_score.cmp(&a_score) // Sort descending
    });

    for m in captures {
        let info = board.make_move(&m);
        let score = -quiescence(board, -beta, -alpha);
        board.unmake_move(&m, info);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
