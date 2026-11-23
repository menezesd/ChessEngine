//! Build-time retrograde solvers for the built-in endgame tables.
//!
//! The attacker chooses the shortest winning line. The defender chooses
//! the longest line, or draws if any reply escapes the winning set.

#[path = "index.rs"]
mod indexing;
#[path = "minor.rs"]
mod minor;
#[path = "pack.rs"]
mod pack;
#[path = "rook.rs"]
mod rook;
#[path = "rook_pawn.rs"]
mod rook_pawn;

use indexing::{
    clock_entry, major_index, pawn_index, CLOCK_DISTANCE_MASK, MAJOR_POSITION_COUNT, NO_WIN,
    PAWN_POSITION_COUNT,
};
use std::ops::Range;

#[derive(Clone, Copy)]
enum Reply {
    Position { index: usize, pawn_move: bool },
    Promotion(u8),
    Draw,
}

struct Position {
    index: usize,
    attacking: bool,
    replies: Range<usize>,
}

fn bit(square: usize) -> u64 {
    1 << square
}

fn squares(mut mask: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let square = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(square)
    })
}

fn king_attacks(square: usize) -> u64 {
    let rank = (square / 8) as i32;
    let file = (square % 8) as i32;
    let mut mask = 0;
    for r in (rank - 1).max(0)..=(rank + 1).min(7) {
        for f in (file - 1).max(0)..=(file + 1).min(7) {
            mask |= bit((r * 8 + f) as usize);
        }
    }
    mask & !bit(square)
}

fn knight_attacks(square: usize) -> u64 {
    let rank = (square / 8) as i32;
    let file = (square % 8) as i32;
    let mut mask = 0;
    for (dr, df) in [
        (-2, -1),
        (-2, 1),
        (-1, -2),
        (-1, 2),
        (1, -2),
        (1, 2),
        (2, -1),
        (2, 1),
    ] {
        let (r, f) = (rank + dr, file + df);
        if (0..8).contains(&r) && (0..8).contains(&f) {
            mask |= bit((r * 8 + f) as usize);
        }
    }
    mask
}

/// The first two directions must decrease square numbers, and the last two
/// must increase them, so the nearest occupied square can be found by bit scan.
const fn sliding_rays(directions: [(i32, i32); 4]) -> [[u64; 4]; 64] {
    let mut rays = [[0; 4]; 64];
    let mut square = 0;
    while square < 64 {
        let mut direction = 0;
        while direction < 4 {
            let (dr, df) = directions[direction];
            let (mut r, mut f) = (square as i32 / 8 + dr, square as i32 % 8 + df);
            while r >= 0 && r < 8 && f >= 0 && f < 8 {
                rays[square][direction] |= 1 << (r * 8 + f);
                r += dr;
                f += df;
            }
            direction += 1;
        }
        square += 1;
    }
    rays
}

const ROOK_RAYS: [[u64; 4]; 64] = sliding_rays([(-1, 0), (0, -1), (1, 0), (0, 1)]);
const BISHOP_RAYS: [[u64; 4]; 64] = sliding_rays([(-1, -1), (-1, 1), (1, -1), (1, 1)]);

fn sliding_attacks([down_a, down_b, up_a, up_b]: [u64; 4], occupancy: u64) -> u64 {
    let descending = |ray: u64| {
        let occupied = ray & occupancy;
        if occupied == 0 {
            ray
        } else {
            ray & (u64::MAX << (63 - occupied.leading_zeros()))
        }
    };
    let ascending = |ray: u64| {
        let occupied = ray & occupancy;
        ray & (occupied ^ occupied.wrapping_sub(1))
    };
    descending(down_a) | descending(down_b) | ascending(up_a) | ascending(up_b)
}

fn rook_attacks(square: usize, occupancy: u64) -> u64 {
    sliding_attacks(ROOK_RAYS[square], occupancy)
}

fn bishop_attacks(square: usize, occupancy: u64) -> u64 {
    sliding_attacks(BISHOP_RAYS[square], occupancy)
}

fn pawn_attacks(pawn: usize) -> u64 {
    let next = pawn + 8;
    let file = pawn % 8;
    let left = if file > 0 { bit(next - 1) } else { 0 };
    let right = if file < 7 { bit(next + 1) } else { 0 };
    left | right
}

fn promotion_distance(king: usize, defender: usize, promoted: usize, majors: [&[u8]; 2]) -> u8 {
    let index = major_index(king, defender, promoted, false);
    majors[0][index].min(majors[1][index])
}

fn major_graph(queen: bool, attacks: &[u64; 64]) -> (Vec<Position>, Vec<Reply>, Vec<u8>) {
    let mut positions = Vec::new();
    let mut replies = Vec::new();
    let mut mate = vec![NO_WIN; MAJOR_POSITION_COUNT];
    for rank in 0..4 {
        for file in 0..4 {
            let major = rank * 8 + file;
            for king in squares(!bit(major)) {
                let major_attacks = |occupied| {
                    let straight = rook_attacks(major, occupied);
                    if queen {
                        straight | bishop_attacks(major, occupied)
                    } else {
                        straight
                    }
                };
                let defended = major_attacks(bit(king));
                let defender_squares = !(bit(major) | bit(king) | attacks[king]);
                for defender in squares(defender_squares) {
                    let checked = defended & bit(defender) != 0;
                    if !checked {
                        let start = replies.len();
                        let occupied = bit(king) | bit(defender);
                        for target in
                            squares(attacks[king] & !(bit(major) | occupied | attacks[defender]))
                        {
                            replies.push(Reply::Position {
                                index: major_index(target, defender, major, false),
                                pawn_move: false,
                            });
                        }
                        for target in squares(major_attacks(occupied) & !occupied) {
                            replies.push(Reply::Position {
                                index: major_index(king, defender, target, false),
                                pawn_move: false,
                            });
                        }
                        if start < replies.len() {
                            positions.push(Position {
                                index: major_index(king, defender, major, true),
                                attacking: true,
                                replies: start..replies.len(),
                            });
                        }
                    }

                    let start = replies.len();
                    for target in squares(attacks[defender] & !(bit(king) | attacks[king])) {
                        if target == major {
                            replies.push(Reply::Draw);
                        } else if defended & bit(target) == 0 {
                            replies.push(Reply::Position {
                                index: major_index(king, target, major, true),
                                pawn_move: false,
                            });
                        }
                    }
                    let defender_index = major_index(king, defender, major, false);
                    if start < replies.len() {
                        positions.push(Position {
                            index: defender_index,
                            attacking: false,
                            replies: start..replies.len(),
                        });
                    } else if checked {
                        mate[defender_index] = 0;
                    }
                }
            }
        }
    }
    (positions, replies, mate)
}

fn pawn_side_replies(
    king: usize,
    defender: usize,
    pawn: usize,
    attacks: &[u64; 64],
    majors: [&[u8]; 2],
    replies: &mut Vec<Reply>,
) {
    let occupied = bit(king) | bit(defender);
    let king_replies = attacks[king] & !(occupied | bit(pawn) | attacks[defender]);
    for target in squares(king_replies) {
        replies.push(Reply::Position {
            index: pawn_index(target, defender, pawn, false),
            pawn_move: false,
        });
    }

    let next = pawn + 8;
    if occupied & bit(next) != 0 {
        return;
    }
    if next >= 56 {
        let distance = promotion_distance(king, defender, next, majors);
        replies.push(if distance == NO_WIN {
            Reply::Draw
        } else {
            Reply::Promotion(distance)
        });
        return;
    }
    replies.push(Reply::Position {
        index: pawn_index(king, defender, next, false),
        pawn_move: true,
    });
    if pawn < 16 && occupied & bit(next + 8) == 0 {
        replies.push(Reply::Position {
            index: pawn_index(king, defender, next + 8, false),
            pawn_move: true,
        });
    }
}

fn pawn_graph(attacks: &[u64; 64], majors: [&[u8]; 2]) -> (Vec<Position>, Vec<Reply>, Vec<u8>) {
    let mut positions = Vec::new();
    let mut replies = Vec::new();
    let mut mate = vec![NO_WIN; PAWN_POSITION_COUNT];

    for rank in 1..7 {
        for file in 0..4 {
            let pawn = rank * 8 + file;
            let pawn_targets = pawn_attacks(pawn);
            for king in squares(!bit(pawn)) {
                let defender_squares = !(bit(pawn) | bit(king) | attacks[king]);
                for defender in squares(defender_squares) {
                    let checked = pawn_targets & bit(defender) != 0;
                    if !checked {
                        let start = replies.len();
                        pawn_side_replies(king, defender, pawn, attacks, majors, &mut replies);
                        if start < replies.len() {
                            positions.push(Position {
                                index: pawn_index(king, defender, pawn, true),
                                attacking: true,
                                replies: start..replies.len(),
                            });
                        }
                    }

                    let start = replies.len();
                    let defender_replies =
                        attacks[defender] & !(bit(king) | attacks[king] | pawn_targets);
                    for target in squares(defender_replies) {
                        replies.push(if target == pawn {
                            Reply::Draw
                        } else {
                            Reply::Position {
                                index: pawn_index(king, target, pawn, true),
                                pawn_move: false,
                            }
                        });
                    }
                    let defender_index = pawn_index(king, defender, pawn, false);
                    if start < replies.len() {
                        positions.push(Position {
                            index: defender_index,
                            attacking: false,
                            replies: start..replies.len(),
                        });
                    } else if checked {
                        mate[defender_index] = 0;
                    }
                }
            }
        }
    }
    (positions, replies, mate)
}

fn solve(positions: &[Position], replies: &[Reply], mut distance: Vec<u8>) -> Vec<u8> {
    let last_promotion = replies
        .iter()
        .filter_map(|reply| match reply {
            Reply::Promotion(distance) => Some(*distance),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    // Layer n contains wins achievable within n plies. Assigning a
    // position in this pass cannot affect another position until n+1.
    for layer in 1..NO_WIN {
        let mut changed = false;
        for position in positions {
            let index = position.index;
            if distance[index] != NO_WIN {
                continue;
            }
            let wins = |reply: &Reply| match *reply {
                Reply::Draw => false,
                Reply::Promotion(distance) => distance < layer,
                Reply::Position { index, .. } => distance[index] < layer,
            };
            let mut successors = replies[position.replies.clone()].iter();
            let winning = if position.attacking {
                successors.any(wins)
            } else {
                successors.all(wins)
            };
            if winning {
                distance[index] = layer;
                changed = true;
            }
        }
        if !changed && layer > last_promotion {
            break;
        }
    }
    distance
}

fn solve_major(queen: bool, attacks: &[u64; 64]) -> Vec<u8> {
    let (positions, replies, mate) = major_graph(queen, attacks);
    solve(&positions, &replies, mate)
}

/// Solve the pawn ending with one more ply on the clock at each step.
/// A pawn move resets the clock. The unbounded distance then applies because
/// all remaining winning distances are below 100. A quiet move consumes a
/// ply from the previous layer. Record only changes to the unbounded distance.
fn solve_pawn_clock(positions: &[Position], replies: &[Reply], mates: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut zeroing: Vec<u8> = mates
        .iter()
        .map(|&mate| if mate == 0 { 0 } else { NO_WIN })
        .collect();
    let mut previous = zeroing.clone();
    let mut next = previous.clone();
    let mut changes = Vec::new();
    let max_mate = mates
        .iter()
        .copied()
        .filter(|&mate| mate != NO_WIN)
        .max()
        .unwrap_or(0);
    for budget in 1..=max_mate {
        for position in positions {
            let index = position.index;
            if previous[index] == mates[index] {
                next[index] = previous[index];
                continue;
            }
            let successors = replies[position.replies.clone()]
                .iter()
                .map(|reply| match *reply {
                    Reply::Draw => NO_WIN,
                    Reply::Promotion(distance) => distance,
                    Reply::Position {
                        index,
                        pawn_move: true,
                    } => mates[index],
                    Reply::Position {
                        index,
                        pawn_move: false,
                    } => previous[index],
                });
            let child = if position.attacking {
                successors.min()
            } else {
                successors.max()
            };
            let distance = child
                .expect("non-terminal positions have legal moves")
                .saturating_add(1);
            if distance != NO_WIN && distance != previous[index] {
                zeroing[index] = zeroing[index].min(budget);
                // Include the return to the fastest distance after a
                // previous entry recorded a longer mate.
                if previous[index] != NO_WIN || distance != mates[index] {
                    assert!(u32::from(distance) <= CLOCK_DISTANCE_MASK);
                    changes.push(clock_entry(index, budget, distance));
                }
            }
            next[index] = distance;
        }
        std::mem::swap(&mut previous, &mut next);
    }
    assert_eq!(previous, mates);
    changes.sort_unstable();
    (
        zeroing,
        changes.into_iter().flat_map(u32::to_le_bytes).collect(),
    )
}

pub fn generate() -> [(&'static str, Vec<u8>); 10] {
    let attacks = std::array::from_fn(king_attacks);
    let queens = solve_major(true, &attacks);
    let rooks = solve_major(false, &attacks);
    let (positions, replies, mate) = pawn_graph(&attacks, [&queens, &rooks]);
    let to_mate = solve(&positions, &replies, mate);
    let (to_pawn_move, clock) = solve_pawn_clock(&positions, &replies, &to_mate);
    assert!(to_mate
        .iter()
        .zip(&to_pawn_move)
        .all(|(&mate, &pawn_move)| {
            (mate == NO_WIN && pawn_move == NO_WIN)
                || (mate != NO_WIN && pawn_move <= mate && pawn_move < 100)
        }));
    assert!(to_mate
        .iter()
        .chain(&queens)
        .chain(&rooks)
        .all(|&mate| mate == NO_WIN || mate < 100));
    // Solve the four possible promotion outcomes before rook versus pawn.
    let promotions = [
        rook::Opponent::Knight,
        rook::Opponent::Bishop,
        rook::Opponent::Rook,
        rook::Opponent::Queen,
    ]
    .map(|opponent| rook::solve(&attacks, &rooks, &queens, opponent));
    let rook_pawn = rook_pawn::solve(
        &attacks,
        &rook_pawn::LowerTables {
            pawns: &to_mate,
            rooks: &rooks,
            queens: &queens,
            promotions: promotions.each_ref().map(Vec::as_slice),
        },
    );
    let [rook_knight, rook_bishop, rook_rook, rook_queen] = promotions;
    // Equal rook positions use the side to move as the designated rook side.
    let rook_rook: Vec<_> = rook_rook.into_iter().skip(1).step_by(2).collect();
    let table: Vec<_> = to_mate
        .into_iter()
        .zip(to_pawn_move)
        .flat_map(|(mate, pawn_move)| [mate, pawn_move])
        .chain(queens)
        .chain(rooks)
        .collect();
    assert_eq!(table.len(), indexing::THREE_PIECE_LENGTH);
    let [bishop_knight, bishop_pair, knight_pair] = minor::tables(&attacks);
    [
        ("endgames.bin", table),
        ("endgame-clock.bin", clock),
        ("endgame-bishop-knight.bin", bishop_knight),
        ("endgame-bishop-pair.bin", bishop_pair),
        ("endgame-knight-pair.bin", knight_pair),
        (
            "endgame-rook-knight.bin",
            pack::distances(&rook_knight, indexing::ROOK_KNIGHT_BLOCK_SIZE),
        ),
        (
            "endgame-rook-bishop.bin",
            pack::distances(&rook_bishop, indexing::ROOK_BISHOP_BLOCK_SIZE),
        ),
        (
            "endgame-rook-rook.bin",
            pack::distances(&rook_rook, indexing::ROOK_ROOK_BLOCK_SIZE),
        ),
        (
            "endgame-rook-queen.bin",
            pack::distances(&rook_queen, indexing::ROOK_QUEEN_BLOCK_SIZE),
        ),
        (
            "endgame-rook-pawn.bin",
            pack::distances(&rook_pawn, indexing::ROOK_PAWN_BLOCK_SIZE),
        ),
    ]
}
