//! Retrograde solver for two minor pieces against a bare king.
use std::collections::VecDeque;

use super::indexing::{
    MinorMaterial, MinorPosition, BISHOP_KNIGHT_BLOCK_SIZE, BISHOP_PAIR_BLOCK_SIZE, KING_ANCHORS,
    KNIGHT_PAIR_DISTANCE_BITS, NO_WIN,
};
use super::{bishop_attacks, bit, knight_attacks, squares};

impl MinorPosition {
    fn from_index(index: usize, material: MinorMaterial) -> Self {
        let attacking = index & 1 != 0;
        let index = index >> 1;
        let (second, first, king, defender) = match material {
            MinorMaterial::BishopKnight | MinorMaterial::KnightPair => (
                index & 63,
                (index >> 6) & 63,
                (index >> 12) & 63,
                KING_ANCHORS[index >> 18],
            ),
            MinorMaterial::BishopPair => {
                let light = (index & 31) * 2;
                let dark = ((index >> 5) & 31) * 2;
                (
                    light | (((light / 8) & 1) ^ 1),
                    dark | ((dark / 8) & 1),
                    (index >> 10) & 63,
                    KING_ANCHORS[index >> 16],
                )
            }
        };
        Self {
            defender,
            king,
            first,
            second,
            attacking,
        }
    }

    fn occupancy(self) -> u64 {
        bit(self.defender) | bit(self.king) | bit(self.first) | bit(self.second)
    }
}

struct MinorAttacks<'a> {
    king: &'a [u64; 64],
    knight: [u64; 64],
}

impl<'a> MinorAttacks<'a> {
    fn new(king: &'a [u64; 64]) -> Self {
        Self {
            king,
            knight: std::array::from_fn(knight_attacks),
        }
    }

    fn defender_attacked(
        &self,
        square: usize,
        attacker: MinorPosition,
        material: MinorMaterial,
        occupancy: u64,
    ) -> bool {
        let first = bit(attacker.first);
        let second = bit(attacker.second);
        match material {
            MinorMaterial::BishopPair => bishop_attacks(square, occupancy) & (first | second) != 0,
            MinorMaterial::BishopKnight => {
                self.knight[square] & second != 0 || bishop_attacks(square, occupancy) & first != 0
            }
            MinorMaterial::KnightPair => self.knight[square] & (first | second) != 0,
        }
    }
}

fn solve(material: MinorMaterial, attacks: &MinorAttacks<'_>) -> Vec<u8> {
    let size = material.position_count();
    let mut distance = vec![NO_WIN; size];
    let mut outdegree = vec![0; size];
    let mut pending = VecDeque::new();

    // The defending side loses only once every distinct reply loses.
    // A legal capture leaves insufficient mating material and keeps the
    // position outside the winning set.
    for (state, degree) in outdegree.iter_mut().enumerate().step_by(2) {
        let pos = MinorPosition::from_index(state, material);
        let occupied = pos.occupancy();
        if occupied.count_ones() != 4
            || attacks.king[pos.defender] & bit(pos.king) != 0
            || pos.index(material) != state
        {
            continue;
        }
        // Reflections can map two legal moves to the same successor.
        let mut destinations = [size; 8];
        let mut count = 0;
        let occupied = occupied & !bit(pos.defender);
        for square in squares(attacks.king[pos.defender] & !attacks.king[pos.king]) {
            if attacks.defender_attacked(square, pos, material, occupied) {
                continue;
            }
            if occupied & bit(square) != 0 {
                *degree += 1;
                continue;
            }
            let next = MinorPosition {
                defender: square,
                attacking: true,
                ..pos
            }
            .index(material);
            if !destinations[..count].contains(&next) {
                destinations[count] = next;
                count += 1;
            }
        }
        *degree += count as u8;
        if *degree == 0 && attacks.defender_attacked(pos.defender, pos, material, occupied) {
            distance[state] = 0;
            pending.push_back(state as u32);
        }
    }

    while let Some(state) = pending.pop_front() {
        // The queue visits wins by increasing distance. The first winning
        // continuation proves an attacker's win, the last proves a defender's loss.
        let pos = MinorPosition::from_index(state as usize, material);
        let occupied = pos.occupancy();
        let remaining = distance[state as usize] + 1;
        if pos.attacking {
            let mut seen = [size; 8];
            let mut count = 0;
            // Before moving, the defender may have been in check.
            let previous_squares =
                attacks.king[pos.defender] & !(occupied | attacks.king[pos.king]);
            for defender in squares(previous_squares) {
                let index = MinorPosition {
                    defender,
                    attacking: false,
                    ..pos
                }
                .index(material);
                if seen[..count].contains(&index) {
                    continue;
                }
                seen[count] = index;
                count += 1;
                debug_assert!(outdegree[index] > 0);
                outdegree[index] -= 1;
                if outdegree[index] == 0 {
                    distance[index] = remaining;
                    pending.push_back(index as u32);
                }
            }
        } else {
            let predecessor = MinorPosition {
                attacking: true,
                ..pos
            };
            let kings = attacks.king[pos.king] & !(occupied | attacks.king[pos.defender]);
            let first = if material == MinorMaterial::KnightPair {
                attacks.knight[pos.first]
            } else {
                bishop_attacks(pos.first, occupied)
            } & !occupied;
            let second = if material == MinorMaterial::BishopPair {
                bishop_attacks(pos.second, occupied)
            } else {
                attacks.knight[pos.second]
            } & !occupied;
            let candidates = squares(kings)
                .map(|king| MinorPosition {
                    king,
                    ..predecessor
                })
                .chain(squares(first).map(|first| MinorPosition {
                    first,
                    ..predecessor
                }))
                .chain(squares(second).map(|second| MinorPosition {
                    second,
                    ..predecessor
                }));
            for prev in candidates {
                let index = prev.index(material);
                if distance[index] == NO_WIN
                    && !attacks.defender_attacked(prev.defender, prev, material, prev.occupancy())
                {
                    distance[index] = remaining;
                    pending.push_back(index as u32);
                }
            }
        }
    }
    distance
}

pub(super) fn tables(king_attacks: &[u64; 64]) -> [Vec<u8>; 3] {
    let attacks = MinorAttacks::new(king_attacks);
    let [bishop_knight, bishop_pair] = [
        (MinorMaterial::BishopKnight, BISHOP_KNIGHT_BLOCK_SIZE),
        (MinorMaterial::BishopPair, BISHOP_PAIR_BLOCK_SIZE),
    ]
    .map(|(material, block_size)| {
        let distances = solve(material, &attacks);
        // Group the positions by side to move, so each block only
        // contains mate distances of one parity.
        let grouped: Vec<_> = distances
            .iter()
            .step_by(2)
            .chain(distances.iter().skip(1).step_by(2))
            .map(|&distance| {
                assert!(distance == NO_WIN || distance < 100);
                distance as i8
            })
            .collect();
        super::pack::distances(&grouped, block_size)
    });
    // Almost every two-knight position draws. Store only the forced mates.
    let mut knight_pair = Vec::new();
    let distances = solve(MinorMaterial::KnightPair, &attacks);
    for (index, &distance) in distances.iter().enumerate() {
        if distance != NO_WIN {
            assert!(distance < 100);
            let entry = ((index as u32) << KNIGHT_PAIR_DISTANCE_BITS) | u32::from(distance);
            knight_pair.extend_from_slice(&entry.to_le_bytes());
        }
    }
    [bishop_knight, bishop_pair, knight_pair]
}
