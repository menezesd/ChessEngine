use super::attack_tables::{slider_attacks, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS};
use super::eval_baseline::*;
use super::{pop_lsb, square_from_index, Board, Color, Piece, Square};
use std::env;

struct EvalTuning {
    material_mg: [i32; 6],
    material_eg: [i32; 6],
}

fn parse_material_env(key: &str, base: [i32; 6]) -> [i32; 6] {
    let Ok(raw) = env::var(key) else {
        return base;
    };
    let cleaned = raw.replace(',', " ");
    let vals: Vec<i32> = cleaned
        .split_whitespace()
        .filter_map(|part| part.parse::<i32>().ok())
        .collect();
    if vals.len() != 6 {
        return base;
    }
    match vals.try_into() {
        Ok(arr) => arr,
        Err(_) => base,
    }
}

impl EvalTuning {
    fn from_env(base_mg: [i32; 6], base_eg: [i32; 6]) -> Self {
        Self {
            material_mg: parse_material_env("EVAL_MATERIAL_MG", base_mg),
            material_eg: parse_material_env("EVAL_MATERIAL_EG", base_eg),
        }
    }
}

impl Board {
    pub(crate) fn evaluate(&self) -> i32 {
        let mut score = 0;

        let tuning = EvalTuning::from_env(BASELINE_MATERIAL_MG, BASELINE_MATERIAL_EG);
        let material_mg = &tuning.material_mg;
        let material_eg = &tuning.material_eg;
        let pst_mg = &BASELINE_PST_MG;
        let pst_eg = &BASELINE_PST_EG;

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

                        white_material_mg += material_mg[piece_idx];
                    } else {
                        if piece == Piece::Bishop {
                            black_bishop_count += 1;
                        } else if piece == Piece::Pawn {
                            black_pawns_by_file[file] += 1;
                        }

                        black_material_mg += material_mg[piece_idx];
                    }
                }
            }
        }

        let total_material_mg = white_material_mg + black_material_mg;
        let max_material = 2
            * (material_mg[1] * 2
                + material_mg[2] * 2
                + material_mg[3] * 2
                + material_mg[4]
                + material_mg[0] * 8);
        let phase = (total_material_mg as f32 / max_material as f32).clamp(0.0, 1.0);

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

                    let mg_value = material_mg[piece_idx] + pst_mg[piece_idx][sq_idx];
                    let eg_value = material_eg[piece_idx] + pst_eg[piece_idx][sq_idx];

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

        let blend = |mg: i32, eg: i32| -> i32 {
            (phase * mg as f32 + (1.0 - phase) * eg as f32) as i32
        };

        let position_score = blend(mg_score, eg_score);
        score += position_score;

        let (white_mobility, black_mobility) = self.mobility_counts();
        let mobility_score = (white_mobility - black_mobility) * BASELINE_MOBILITY_WEIGHT;
        score += (mobility_score as f32 * phase) as i32;

        let bishop_pair = BASELINE_BISHOP_PAIR;
        if white_bishop_count >= 2 {
            score += bishop_pair;
        }
        if black_bishop_count >= 2 {
            score -= bishop_pair;
        }

        for file in 0..8 {
            for rank in 0..8 {
                if let Some((color, piece)) = self.piece_at(Square(rank, file)) {
                    if piece == Piece::Rook {
                        let file_pawns = white_pawns_by_file[file] + black_pawns_by_file[file];

                        if file_pawns == 0 {
                            let bonus = BASELINE_ROOK_OPEN;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        } else if (color == Color::White && black_pawns_by_file[file] == 0)
                            || (color == Color::Black && white_pawns_by_file[file] == 0)
                        {
                            let bonus = BASELINE_ROOK_HALF;
                            if color == Color::White {
                                score += bonus;
                            } else {
                                score -= bonus;
                            }
                        }

                        if color == Color::White && rank == 6 {
                            score += BASELINE_ROOK_7TH;
                        }
                        if color == Color::Black && rank == 1 {
                            score -= BASELINE_ROOK_7TH;
                        }
                    }
                }
            }
        }

        let white_king_sq = self.find_king(Color::White);
        let black_king_sq = self.find_king(Color::Black);
        let king_dist = |a: Square, b: Square| -> i32 {
            let dr = if a.0 > b.0 { a.0 - b.0 } else { b.0 - a.0 };
            let df = if a.1 > b.1 { a.1 - b.1 } else { b.1 - a.1 };
            dr.max(df) as i32
        };

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
                    score -= BASELINE_ISOLATED;
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
                    score += BASELINE_ISOLATED;
                }
            }

            if white_pawns_by_file[file] > 1 {
                score -= BASELINE_DOUBLED * (white_pawns_by_file[file] - 1);
            }

            if black_pawns_by_file[file] > 1 {
                score += BASELINE_DOUBLED * (black_pawns_by_file[file] - 1);
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
                        let mut eg_adjust = 0;
                        if let Some(ksq) = white_king_sq {
                            if king_dist(ksq, Square(rank, file)) <= 2 {
                                eg_adjust += 6 + (7 - rank as i32) * 2;
                            }
                        }
                        if let Some(eksq) = black_king_sq {
                            if king_dist(eksq, Square(rank, file)) <= 2 && eksq.0 >= rank {
                                eg_adjust -= 6 + (7 - rank as i32);
                            }
                        }
                        score += blend(0, eg_adjust);
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
                        let mut eg_adjust = 0;
                        if let Some(ksq) = black_king_sq {
                            if king_dist(ksq, Square(rank, file)) <= 2 {
                                eg_adjust += 6 + rank as i32 * 2;
                            }
                        }
                        if let Some(eksq) = white_king_sq {
                            if king_dist(eksq, Square(rank, file)) <= 2 && eksq.0 <= rank {
                                eg_adjust -= 6 + rank as i32;
                            }
                        }
                        score -= blend(0, eg_adjust);
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
                let attacks =
                    (slider_attacks(sq, occ, true) | slider_attacks(sq, occ, false)) & zone;
                units += 4 * attacks.count_ones() as i32;
            }

            units
        };

        let units_white = king_attack_units(Color::White);
        let units_black = king_attack_units(Color::Black);
        let safety_table: [i32; 21] = [
            0, 0, 2, 5, 9, 14, 20, 27, 35, 44, 54, 65, 77, 90, 104, 119, 135, 152, 170, 189, 209,
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

        let minor_bonus = BASELINE_MINOR_ATTACK;
        let minor_score =
            (minor_attack_count(Color::White) - minor_attack_count(Color::Black)) * minor_bonus;
        score += minor_score;

        let supported_pawn_bonus = |color: Color, rank: usize, file: usize| -> i32 {
            let forward = if color == Color::White { -1i32 } else { 1i32 };
            let defend_rank = rank as i32 + forward;
            let mut supported = false;
            if (0..=7).contains(&defend_rank) {
                if file > 0 {
                    if let Some((c, Piece::Pawn)) =
                        self.piece_at(Square(defend_rank as usize, file - 1))
                    {
                        supported |= c == color;
                    }
                }
                if file < 7 {
                    if let Some((c, Piece::Pawn)) =
                        self.piece_at(Square(defend_rank as usize, file + 1))
                    {
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

        let minor_only =
            white_material_mg + black_material_mg <= 2 * (material_mg[1] * 2 + material_mg[2] * 2);
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
