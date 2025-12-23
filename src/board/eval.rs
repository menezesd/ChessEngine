use super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::{pop_lsb, square_from_index, Board, Color, Piece, Square};

impl Board {
    pub(crate) fn evaluate(&self) -> i32 {
        let mut score = 0;

        const MATERIAL_MG: [i32; 6] = [82, 337, 365, 477, 1025, 20000];
        const MATERIAL_EG: [i32; 6] = [94, 281, 297, 512, 936, 20000];

        const PST_MG: [[i32; 64]; 6] = [
            [
                0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3,
                3, 33, -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6,
                7, 26, 31, 65, 56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0,
                0,
            ],
            [
                -105, -21, -58, -33, -17, -28, -19, -23, -29, -53, -12, -3, -1, 18, -14, -19, -23,
                -9, 12, 10, 19, 17, 25, -16, -13, 4, 16, 13, 28, 19, 21, -8, -9, 17, 19, 53, 37,
                69, 18, 22, -47, 60, 37, 65, 84, 129, 73, 44, -73, -41, 72, 36, 23, 62, 7, -17,
                -167, -89, -34, -49, 61, -97, -15, -107,
            ],
            [
                -33, -3, -14, -21, -13, -12, -39, -21, 4, 15, 16, 0, 7, 21, 33, 1, 0, 15, 15, 15,
                14, 27, 18, 10, -6, 13, 13, 26, 34, 12, 10, 4, -4, 5, 19, 50, 37, 37, 7, -2, -16,
                37, 43, 40, 35, 50, 37, -2, -26, 16, -18, -13, 30, 59, 18, -47, -29, 4, -82, -37,
                -25, -42, 7, -8,
            ],
            [
                -19, -13, 1, 17, 16, 7, -37, -26, -44, -16, -20, -9, -1, 11, -6, -71, -45, -25,
                -16, -17, 3, 0, -5, -33, -36, -26, -12, -1, 9, -7, 6, -23, -24, -11, 7, 26, 24, 35,
                -8, -20, -5, 19, 26, 36, 17, 45, 61, 16, 27, 32, 58, 62, 80, 67, 26, 44, 32, 42,
                32, 51, 63, 9, 31, 43,
            ],
            [
                -1, -18, -9, 10, -15, -25, -31, -50, -35, -8, 11, 2, 8, 15, -3, 1, -14, 2, -11, -2,
                -5, 2, 14, 5, -9, -26, -9, -10, -2, -4, 3, -3, -27, -27, -16, -16, -1, 17, -2, 1,
                -13, -17, 7, 8, 29, 56, 47, 57, -24, -39, -5, 1, -16, 57, 28, 54, -28, 0, 29, 12,
                59, 44, 43, 45,
            ],
            [
                -15, 36, 12, -54, 8, -28, 34, 14, 1, 7, -8, -64, -43, -16, 9, 8, -14, -14, -22,
                -46, -44, -30, -15, -27, -49, -1, -27, -39, -46, -44, -33, -51, -17, -20, -12, -27,
                -30, -25, -14, -36, -9, 24, 2, -16, -20, 6, 22, -22, 29, -1, -20, -7, -8, -4, -38,
                -29, -65, 23, 16, -15, -56, -34, 2, 13,
            ],
        ];

        const PST_EG: [[i32; 64]; 6] = [
            [
                0, 0, 0, 0, 0, 0, 0, 0, 13, 8, 8, 10, 13, 0, 2, -7, 4, 7, -6, 1, 0, -5, -1, -8, 13,
                9, -3, -7, -7, -8, 3, -1, 32, 24, 13, 5, -2, 4, 17, 17, 94, 100, 85, 67, 56, 53,
                82, 84, 178, 173, 158, 134, 147, 132, 165, 187, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            [
                -29, -51, -23, -15, -22, -18, -50, -64, -42, -20, -10, -5, -2, -20, -23, -44, -23,
                -3, -1, 15, 10, -3, -20, -22, -18, -6, 16, 25, 16, 17, 4, -18, -17, 3, 22, 22, 22,
                11, 8, -18, -24, -20, 10, 9, -1, -9, -19, -41, -25, -8, -25, -2, -9, -25, -24, -52,
                -58, -38, -13, -28, -31, -27, -63, -99,
            ],
            [
                -23, -9, -23, -5, -9, -16, -5, -17, -14, -18, -7, -1, 4, -9, -15, -27, -12, -3, 8,
                10, 13, 3, -7, -15, -6, 3, 13, 19, 7, 10, -3, -9, -3, 9, 12, 9, 14, 10, 3, 2, 2,
                -8, 0, -1, -2, 6, 0, 4, -8, -4, 7, -12, -3, -13, -4, -14, -14, -21, -11, -8, -7,
                -9, -17, -24,
            ],
            [
                -9, 2, 3, -1, -5, -13, 4, -20, -6, -6, 0, 2, -9, -9, -11, -3, -4, 0, -5, -1, -7,
                -12, -8, -16, 3, 5, 8, 4, -5, -6, -8, -11, 4, 3, 13, 1, 2, 1, -1, 2, 7, 7, 7, 5, 4,
                -3, -5, -3, 11, 13, 13, 11, -3, 3, 8, 3, 13, 10, 18, 15, 12, 12, 8, 5,
            ],
            [
                -33, -28, -22, -43, -5, -32, -20, -41, -22, -23, -30, -16, -16, -23, -36, -32, -16,
                -27, 15, 6, 9, 17, 10, 5, -18, 28, 19, 47, 31, 34, 39, 23, 3, 22, 24, 45, 57, 40,
                57, 36, -20, 6, 9, 49, 47, 35, 19, 9, -17, 20, 32, 41, 58, 25, 30, 0, -9, 22, 22,
                27, 27, 19, 10, 20,
            ],
            [
                -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, -5, -17, -19, -3,
                11, 21, 23, 16, 7, -9, -18, -4, 21, 24, 27, 23, 9, -11, -8, 22, 24, 27, 26, 33, 26,
                3, 10, 17, 23, 15, 20, 45, 44, 13, -12, 17, 14, 17, 17, 38, 23, 11, -74, -35, -18,
                -18, -11, 15, 4, -17,
            ],
        ];

        fn square_to_index(rank: usize, file: usize) -> usize {
            rank * 8 + file
        }

        fn piece_to_index(piece: Piece) -> usize {
            match piece {
                Piece::Pawn => 0,
                Piece::Knight => 1,
                Piece::Bishop => 2,
                Piece::Rook => 3,
                Piece::Queen => 4,
                Piece::King => 5,
            }
        }

        let mut white_material_mg = 0;
        let mut black_material_mg = 0;
        let mut white_bishop_count = 0;
        let mut black_bishop_count = 0;
        let mut white_pawns_by_file = [0; 8];
        let mut black_pawns_by_file = [0; 8];

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.piece_at(Square(rank, file)) {
                    let piece_idx = piece_to_index(piece);

                    if color == Color::White {
                        if piece == Piece::Bishop {
                            white_bishop_count += 1;
                        } else if piece == Piece::Pawn {
                            white_pawns_by_file[file] += 1;
                        }

                        white_material_mg += MATERIAL_MG[piece_idx];
                    } else {
                        if piece == Piece::Bishop {
                            black_bishop_count += 1;
                        } else if piece == Piece::Pawn {
                            black_pawns_by_file[file] += 1;
                        }

                        black_material_mg += MATERIAL_MG[piece_idx];
                    }
                }
            }
        }

        let total_material_mg = white_material_mg + black_material_mg;
        let max_material = 2
            * (MATERIAL_MG[1] * 2
                + MATERIAL_MG[2] * 2
                + MATERIAL_MG[3] * 2
                + MATERIAL_MG[4]
                + MATERIAL_MG[0] * 8);
        let phase = (total_material_mg as f32) / (max_material as f32);
        let phase = phase.min(1.0).max(0.0);

        let mut mg_score = 0;
        let mut eg_score = 0;

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.piece_at(Square(rank, file)) {
                    let piece_idx = piece_to_index(piece);

                    let sq_idx = if color == Color::White {
                        square_to_index(7 - rank, file)
                    } else {
                        square_to_index(rank, file)
                    };

                    let mg_value = MATERIAL_MG[piece_idx] + PST_MG[piece_idx][sq_idx];
                    let eg_value = MATERIAL_EG[piece_idx] + PST_EG[piece_idx][sq_idx];

                    if color == Color::White {
                        mg_score += mg_value;
                        eg_score += eg_value;
                    } else {
                        mg_score -= mg_value;
                        eg_score -= eg_value;
                    }
                }
            }
        }

        let position_score = (phase * mg_score as f32 + (1.0 - phase) * eg_score as f32) as i32;
        score += position_score;

        let (white_mobility, black_mobility) = self.mobility_counts();
        let mobility_score = (white_mobility - black_mobility) * 2;
        score += (mobility_score as f32 * phase) as i32;

        if white_bishop_count >= 2 {
            score += 30;
        }
        if black_bishop_count >= 2 {
            score -= 30;
        }

        for file in 0..8 {
            for rank in 0..8 {
                if let Some((color, piece)) = self.piece_at(Square(rank, file)) {
                    if piece == Piece::Rook {
                        let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];

                        if file_pawns == 0 {
                            let bonus = 15;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        } else if (color == Color::White && black_pawns_by_file[file] == 0)
                            || (color == Color::Black && white_pawns_by_file[file] == 0)
                        {
                            let bonus = 7;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        }

                        if color == Color::White && rank == 6 {
                            score += 12;
                        }
                        if color == Color::Black && rank == 1 {
                            score -= 12;
                        }
                    }
                }
            }
        }

        for file in 0..8 {
            if white_pawns_by_file[file] > 0 {
                let left_file = if file > 0 {
                    white_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right_file = if file < 7 {
                    white_pawns_by_file[file + 1]
                } else {
                    0
                };

                if left_file == 0 && right_file == 0 {
                    score -= 12;
                }
            }

            if black_pawns_by_file[file] > 0 {
                let left_file = if file > 0 {
                    black_pawns_by_file[file - 1]
                } else {
                    0
                };
                let right_file = if file < 7 {
                    black_pawns_by_file[file + 1]
                } else {
                    0
                };

                if left_file == 0 && right_file == 0 {
                    score += 12;
                }
            }

            if white_pawns_by_file[file] > 1 {
                score -= 12 * (white_pawns_by_file[file] - 1);
            }

            if black_pawns_by_file[file] > 1 {
                score += 12 * (black_pawns_by_file[file] - 1);
            }

            for rank in 0..8 {
                let sq = Square(rank, file);
                if let Some((Color::White, Piece::Pawn)) = self.piece_at(sq) {
                    let mut is_passed = true;
                    let mut is_blocked = false;

                    for check_rank in 0..rank {
                        for check_file in file.saturating_sub(1)..=(file + 1).min(7) {
                            let check_sq = Square(check_rank, check_file);
                            if let Some((Color::Black, Piece::Pawn)) = self.piece_at(check_sq) {
                                is_passed = false;
                                break;
                            }
                        }
                        if !is_passed {
                            break;
                        }
                    }

                    if rank < 7 {
                        let ahead = Square(rank + 1, file);
                        if self.piece_at(ahead).is_some() {
                            is_blocked = true;
                        }
                    }

                    if is_passed {
                        let bonus = 10 + (7 - rank as i32) * 7;
                        score += bonus;
                        if is_blocked {
                            score -= 8;
                        }
                    }
                } else if let Some((Color::Black, Piece::Pawn)) = self.piece_at(sq) {
                    let mut is_passed = true;
                    let mut is_blocked = false;

                    for check_rank in (rank + 1)..8 {
                        for check_file in file.saturating_sub(1)..=(file + 1).min(7) {
                            let check_sq = Square(check_rank, check_file);
                            if let Some((Color::White, Piece::Pawn)) = self.piece_at(check_sq) {
                                is_passed = false;
                                break;
                            }
                        }
                        if !is_passed {
                            break;
                        }
                    }

                    if rank > 0 {
                        let ahead = Square(rank - 1, file);
                        if self.piece_at(ahead).is_some() {
                            is_blocked = true;
                        }
                    }

                    if is_passed {
                        let bonus = 10 + rank as i32 * 7;
                        score -= bonus;
                        if is_blocked {
                            score += 8;
                        }
                    }
                }
            }
        }

        for file in 0..8 {
            for rank in 0..8 {
                if let Some((color, Piece::Pawn)) = self.piece_at(Square(rank, file)) {
                    let forward = if color == Color::White { 1i32 } else { -1i32 };
                    let next_rank = rank as i32 + forward;
                    if !(0..=7).contains(&next_rank) {
                        continue;
                    }
                    let same_file_ahead = Square(next_rank as usize, file);
                    if self.piece_at(same_file_ahead).is_some() {
                        continue;
                    }

                    let left_file = if file > 0 { file - 1 } else { 8 };
                    let right_file = if file < 7 { file + 1 } else { 8 };
                    let mut has_support = false;

                    if left_file < 8 && self.piece_at(Square(rank, left_file)).is_some() {
                        if let Some((c, Piece::Pawn)) = self.piece_at(Square(rank, left_file)) {
                            if c == color {
                                has_support = true;
                            }
                        }
                    }
                    if right_file < 8 && self.piece_at(Square(rank, right_file)).is_some() {
                        if let Some((c, Piece::Pawn)) = self.piece_at(Square(rank, right_file)) {
                            if c == color {
                                has_support = true;
                            }
                        }
                    }

                    if !has_support {
                        let penalty = 8;
                        if color == Color::White {
                            score -= penalty;
                        } else {
                            score += penalty;
                        }
                    }
                }
            }
        }

        let king_safety = |color: Color| -> i32 {
            let c_idx = if color == Color::White { 0 } else { 1 };
            let mut bb = self.pieces[c_idx][piece_to_index(Piece::King)];
            if bb.0 == 0 {
                return 0;
            }
            let sq = square_from_index(pop_lsb(&mut bb));
            let mut attacks = 0;
            for dr in -1i32..=1 {
                for df in -1i32..=1 {
                    let rr = sq.0 as i32 + dr;
                    let ff = sq.1 as i32 + df;
                    if !(0..=7).contains(&rr) || !(0..=7).contains(&ff) {
                        continue;
                    }
                    let target = Square(rr as usize, ff as usize);
                    if self.is_square_attacked(target, self.opponent_color(color)) {
                        attacks += 1;
                    }
                }
            }
            attacks
        };

        let safety = (king_safety(Color::Black) - king_safety(Color::White)) * 8;
        score += (safety as f32 * phase) as i32;

        let king_attack_units = |attacker: Color| -> i32 {
            let defender = self.opponent_color(attacker);
            let king_sq = match self.find_king(defender) {
                Some(sq) => sq,
                None => return 0,
            };
            let king_idx = square_to_index(king_sq.0, king_sq.1);
            let zone = KING_ATTACKS[king_idx] | (1u64 << king_idx);
            let c_idx = if attacker == Color::White { 0 } else { 1 };
            let occ = self.all_occupied.0;
            let mut units = 0i32;

            let mut pawns = self.pieces[c_idx][piece_to_index(Piece::Pawn)].0;
            while pawns != 0 {
                let sq = pawns.trailing_zeros() as usize;
                pawns &= pawns - 1;
                let attacks = PAWN_ATTACKS[c_idx][sq] & zone;
                units += attacks.count_ones() as i32;
            }

            let mut knights = self.pieces[c_idx][piece_to_index(Piece::Knight)].0;
            while knights != 0 {
                let sq = knights.trailing_zeros() as usize;
                knights &= knights - 1;
                let attacks = KNIGHT_ATTACKS[sq] & zone;
                units += 2 * attacks.count_ones() as i32;
            }

            let mut bishops = self.pieces[c_idx][piece_to_index(Piece::Bishop)].0;
            while bishops != 0 {
                let sq = bishops.trailing_zeros() as usize;
                bishops &= bishops - 1;
                let attacks = slider_attacks(sq, occ, true) & zone;
                units += 2 * attacks.count_ones() as i32;
            }

            let mut rooks = self.pieces[c_idx][piece_to_index(Piece::Rook)].0;
            while rooks != 0 {
                let sq = rooks.trailing_zeros() as usize;
                rooks &= rooks - 1;
                let attacks = slider_attacks(sq, occ, false) & zone;
                units += 3 * attacks.count_ones() as i32;
            }

            let mut queens = self.pieces[c_idx][piece_to_index(Piece::Queen)].0;
            while queens != 0 {
                let sq = queens.trailing_zeros() as usize;
                queens &= queens - 1;
                let attacks = (slider_attacks(sq, occ, true) | slider_attacks(sq, occ, false)) & zone;
                units += 4 * attacks.count_ones() as i32;
            }

            units
        };

        let units_white = king_attack_units(Color::White);
        let units_black = king_attack_units(Color::Black);
        let safety_table: [i32; 21] = [
            0, 0, 2, 5, 9, 14, 20, 27, 35, 44, 54, 65, 77, 90, 104, 119, 135, 152, 170, 189,
            209,
        ];
        let idx_w = units_white.min(20) as usize;
        let idx_b = units_black.min(20) as usize;
        let attack_score = safety_table[idx_b] - safety_table[idx_w];
        score += (attack_score as f32 * phase) as i32;

        let minor_attack_count = |attacker: Color| -> i32 {
            let c_idx = if attacker == Color::White { 0 } else { 1 };
            let enemy_idx = if attacker == Color::White { 1 } else { 0 };
            let occ = self.all_occupied.0;
            let enemy_minors = self.pieces[enemy_idx][piece_to_index(Piece::Knight)].0
                | self.pieces[enemy_idx][piece_to_index(Piece::Bishop)].0;
            let mut count = 0i32;

            let mut knights = self.pieces[c_idx][piece_to_index(Piece::Knight)].0;
            while knights != 0 {
                let sq = knights.trailing_zeros() as usize;
                knights &= knights - 1;
                if KNIGHT_ATTACKS[sq] & enemy_minors != 0 {
                    count += 1;
                }
            }

            let mut bishops = self.pieces[c_idx][piece_to_index(Piece::Bishop)].0;
            while bishops != 0 {
                let sq = bishops.trailing_zeros() as usize;
                bishops &= bishops - 1;
                if slider_attacks(sq, occ, true) & enemy_minors != 0 {
                    count += 1;
                }
            }

            count
        };

        let minor_score = (minor_attack_count(Color::White) - minor_attack_count(Color::Black)) * 6;
        score += (minor_score as f32 * phase) as i32;

        let supported_pawn_bonus = |color: Color, rank: usize, file: usize| -> i32 {
            let forward = if color == Color::White { -1i32 } else { 1i32 };
            let defend_rank = rank as i32 + forward;
            let mut supported = false;
            if (0..=7).contains(&defend_rank) {
                if file > 0 {
                    if let Some((c, Piece::Pawn)) = self.piece_at(Square(defend_rank as usize, file - 1)) {
                        supported |= c == color;
                    }
                }
                if file < 7 {
                    if let Some((c, Piece::Pawn)) = self.piece_at(Square(defend_rank as usize, file + 1)) {
                        supported |= c == color;
                    }
                }
            }

            let mut phalanx = false;
            if file > 0 {
                if let Some((c, Piece::Pawn)) = self.piece_at(Square(rank, file - 1)) {
                    phalanx |= c == color;
                }
            }
            if file < 7 {
                if let Some((c, Piece::Pawn)) = self.piece_at(Square(rank, file + 1)) {
                    phalanx |= c == color;
                }
            }

            if supported || phalanx {
                10
            } else {
                0
            }
        };

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, Piece::Pawn)) = self.piece_at(Square(rank, file)) {
                    let bonus = supported_pawn_bonus(color, rank, file);
                    if color == Color::White {
                        score += bonus;
                    } else {
                        score -= bonus;
                    }
                }
            }
        }

        for rank in 0..8 {
            for file in 0..8 {
                if let Some((color, piece)) = self.piece_at(Square(rank, file)) {
                    if piece == Piece::King || piece == Piece::Pawn {
                        continue;
                    }
                    let sq = Square(rank, file);
                    let attacked = self.is_square_attacked(sq, self.opponent_color(color));
                    let defended = self.is_square_attacked(sq, color);
                    if attacked && !defended {
                        let penalty = match piece {
                            Piece::Knight | Piece::Bishop => 20,
                            Piece::Rook => 30,
                            Piece::Queen => 40,
                            _ => 0,
                        };
                        if color == Color::White {
                            score -= penalty;
                        } else {
                            score += penalty;
                        }
                    }
                }
            }
        }

        let white_king_sq = self.find_king(Color::White);
        let black_king_sq = self.find_king(Color::Black);
        if let Some(king) = white_king_sq {
            if king == Square(0, 4) {
                let rook_a = self.piece_at(Square(0, 0));
                let rook_h = self.piece_at(Square(0, 7));
                if rook_a == Some((Color::White, Piece::Rook)) {
                    score -= 10;
                }
                if rook_h == Some((Color::White, Piece::Rook)) {
                    score -= 10;
                }
            }
        }
        if let Some(king) = black_king_sq {
            if king == Square(7, 4) {
                let rook_a = self.piece_at(Square(7, 0));
                let rook_h = self.piece_at(Square(7, 7));
                if rook_a == Some((Color::Black, Piece::Rook)) {
                    score += 10;
                }
                if rook_h == Some((Color::Black, Piece::Rook)) {
                    score += 10;
                }
            }
        }

        let pawn_shield = |color: Color| -> i32 {
            let c_idx = if color == Color::White { 0 } else { 1 };
            let mut bb = self.pieces[c_idx][piece_to_index(Piece::King)];
            if bb.0 == 0 {
                return 0;
            }
            let king_sq = square_from_index(pop_lsb(&mut bb));
            let forward = if color == Color::White { 1i32 } else { -1i32 };
            let target_rank = king_sq.0 as i32 + forward;
            if !(0..=7).contains(&target_rank) {
                return 0;
            }
            let mut shield = 0;
            for df in -1i32..=1 {
                let file = king_sq.1 as i32 + df;
                if !(0..=7).contains(&file) {
                    continue;
                }
                let sq = Square(target_rank as usize, file as usize);
                if let Some((c, Piece::Pawn)) = self.piece_at(sq) {
                    if c == color {
                        shield += 1;
                    }
                }
            }
            shield
        };

        let shield_score = (pawn_shield(Color::White) - pawn_shield(Color::Black)) * 12;
        score += (shield_score as f32 * phase) as i32;

        let minor_only = white_material_mg + black_material_mg
            <= 2 * (MATERIAL_MG[1] * 2 + MATERIAL_MG[2] * 2);
        if minor_only {
            score = (score as f32 * 0.6) as i32;
        }
        if white_pawns_by_file.iter().sum::<i32>() == 0
            && black_pawns_by_file.iter().sum::<i32>() == 0
        {
            score = (score as f32 * 0.5) as i32;
        }

        score
    }

    pub(crate) fn eval_for_side(&self) -> i32 {
        if self.white_to_move {
            self.evaluate()
        } else {
            -self.evaluate()
        }
    }
}
