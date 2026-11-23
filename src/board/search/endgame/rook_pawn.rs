//! Exact distance to a capture, a pawn move, or mate in rook versus pawn.
use std::collections::VecDeque;

use super::indexing::{
    major_index, pawn_index, RookPawnPosition as Position, RookPosition, NO_WIN,
    ROOK_PAWN_POSITION_COUNT as POSITION_COUNT, ROOK_PAWN_STATES_PER_RANK as STATES_PER_RANK,
};
use super::{bit, rook_attacks, squares};

const PAWN: usize = 0;
const ROOK: usize = 1;
impl Position {
    fn from_index(index: usize) -> Self {
        let pawn = index >> 19;
        Self {
            kings: [(index >> 13) & 63, (index >> 7) & 63],
            pawn: (pawn / 4 + 1) * 8 + pawn % 4,
            rook: (index >> 1) & 63,
            turn: index & 1,
        }
    }

    fn occupancy(self) -> u64 {
        bit(self.kings[0]) | bit(self.kings[1]) | bit(self.pawn) | bit(self.rook)
    }
}

struct Attacks<'a> {
    king: &'a [u64; 64],
    pawn: [u64; 64],
}

impl<'a> Attacks<'a> {
    fn new(king: &'a [u64; 64]) -> Self {
        Self {
            king,
            pawn: std::array::from_fn(|sq| {
                if sq < 8 {
                    return 0;
                }
                let west = if sq % 8 > 0 { bit(sq - 9) } else { 0 };
                let east = if sq % 8 < 7 { bit(sq - 7) } else { 0 };
                west | east
            }),
        }
    }

    fn checked(&self, pos: Position, side: usize) -> bool {
        self.king[pos.kings[0]] & bit(pos.kings[1]) != 0
            || if side == PAWN {
                rook_attacks(pos.kings[PAWN], pos.occupancy()) & bit(pos.rook) != 0
            } else {
                self.pawn[pos.pawn] & bit(pos.kings[ROOK]) != 0
            }
    }
}

pub(super) struct LowerTables<'a> {
    pub pawns: &'a [u8],
    pub rooks: &'a [u8],
    pub queens: &'a [u8],
    pub promotions: [&'a [i8]; 4],
}

impl LowerTables<'_> {
    fn capture_result(&self, child: Position, target: usize) -> i8 {
        // The mover captured the last opposing piece. The surviving
        // pawn or rook is still on the board, or has just promoted.
        if child.turn == PAWN {
            let idx = major_index(child.kings[ROOK], child.kings[PAWN], child.rook, false);
            return i8::from(self.rooks[idx] != NO_WIN);
        }
        if target == child.pawn && target < 8 {
            let idx = major_index(child.kings[PAWN], child.kings[ROOK], target, false);
            return i8::from(self.rooks[idx] != NO_WIN || self.queens[idx] != NO_WIN);
        }
        let idx = pawn_index(
            child.kings[PAWN] ^ 56,
            child.kings[ROOK] ^ 56,
            child.pawn ^ 56,
            false,
        );
        i8::from(self.pawns[idx] != NO_WIN)
    }

    fn advance_result(&self, child: Position, distance: &[i8]) -> i8 {
        if child.pawn < 8 {
            // With the same coordinates, only the promoted piece type
            // changes the resulting outcome.
            let idx = RookPosition {
                kings: child.kings,
                pieces: [child.pawn, child.rook],
                turn: ROOK,
            }
            .index();
            return self
                .promotions
                .iter()
                .map(|table| -table[idx].signum())
                .max()
                .unwrap();
        }
        -distance[child.index()].signum()
    }
}

#[derive(Default)]
struct Replies {
    pending: u8,
    losing_exit: bool,
    winning_exit: bool,
}

impl Replies {
    fn zeroing(&mut self, outcome: i8) {
        match outcome {
            1 => self.winning_exit = true,
            -1 => self.losing_exit = true,
            _ => self.pending += 1, // A draw can never become a loss.
        }
    }
}

fn replies(
    pos: Position,
    attacks: &Attacks<'_>,
    lower: &LowerTables<'_>,
    distance: &[i8],
) -> Replies {
    let mover = pos.turn;
    let other = mover ^ 1;
    let occupied = pos.occupancy();
    let (own_piece, victim) = if mover == PAWN {
        (pos.pawn, pos.rook)
    } else {
        (pos.rook, pos.pawn)
    };
    let allowed = !(bit(own_piece) | bit(pos.kings[other]) | attacks.king[pos.kings[other]]);
    let mut counters = Replies::default();

    for target in squares(attacks.king[pos.kings[mover]] & allowed) {
        let mut child = pos;
        child.kings[mover] = target;
        child.turn = other;
        if target == victim {
            counters.zeroing(lower.capture_result(child, target));
        } else if !attacks.checked(child, mover) {
            counters.pending += 1;
        }
    }

    if mover == ROOK {
        let targets =
            rook_attacks(pos.rook, occupied) & !(bit(pos.kings[ROOK]) | bit(pos.kings[PAWN]));
        for target in squares(targets) {
            let child = Position {
                rook: target,
                turn: other,
                ..pos
            };
            if target == victim {
                counters.zeroing(lower.capture_result(child, target));
            } else if !attacks.checked(child, mover) {
                counters.pending += 1;
            }
        }
    } else {
        if attacks.pawn[pos.pawn] & bit(pos.rook) != 0 {
            let child = Position {
                pawn: pos.rook,
                turn: other,
                ..pos
            };
            counters.zeroing(lower.capture_result(child, pos.rook));
        }
        let single = pos.pawn - 8;
        if occupied & bit(single) == 0 {
            let child = Position {
                pawn: single,
                turn: other,
                ..pos
            };
            if !attacks.checked(child, mover) {
                counters.zeroing(lower.advance_result(child, distance));
            }
            if pos.pawn >= 48 && occupied & bit(single - 8) == 0 {
                let child = Position {
                    pawn: single - 8,
                    turn: other,
                    ..pos
                };
                if !attacks.checked(child, mover) {
                    counters.zeroing(lower.advance_result(child, distance));
                }
            }
        }
    }
    counters
}

pub(super) fn solve(king_attacks: &[u64; 64], lower: &LowerTables<'_>) -> Vec<i8> {
    let attacks = Attacks::new(king_attacks);
    let mut distance = vec![0i8; POSITION_COUNT];
    let mut outdegree = vec![0u8; STATES_PER_RANK];
    let mut pending = VecDeque::new();
    let mut exits = Vec::new();

    // A black pawn only moves to a lower rank. A reset into that rank can
    // therefore use its solved result with a fresh halfmove clock.
    for rank in 0..6 {
        let start = rank * STATES_PER_RANK;
        for (offset, degree) in outdegree.iter_mut().enumerate() {
            let index = start + offset;
            let pos = Position::from_index(index);
            *degree = 0;
            if pos.occupancy().count_ones() != 4 || attacks.checked(pos, pos.turn ^ 1) {
                continue;
            }
            let replies = replies(pos, &attacks, lower, &distance);
            *degree = replies.pending;
            if replies.winning_exit || (replies.pending == 0 && replies.losing_exit) {
                distance[index] = if replies.winning_exit { 2 } else { -2 };
                exits.push(index as u32);
            } else if replies.pending == 0 && attacks.checked(pos, pos.turn) {
                distance[index] = -1;
                pending.push_back(index as u32);
            }
        }

        // Mate has distance zero. Captures and pawn advances have distance
        // one, regardless of how long it takes to finish after resetting.
        pending.extend(exits.drain(..));
        while let Some(index) = pending.pop_front() {
            let pos = Position::from_index(index as usize);
            let value = distance[index as usize];
            let mover = pos.turn ^ 1;
            let occupied = pos.occupancy();
            let king_origins = attacks.king[pos.kings[mover]] & !occupied;
            let rook_origins = if mover == ROOK {
                rook_attacks(pos.rook, occupied) & !occupied
            } else {
                0
            };
            for (moves_king, origin) in squares(king_origins)
                .map(|sq| (true, sq))
                .chain(squares(rook_origins).map(|sq| (false, sq)))
            {
                let mut parent = Position { turn: mover, ..pos };
                if moves_king {
                    parent.kings[mover] = origin;
                } else {
                    parent.rook = origin;
                }
                if attacks.checked(parent, pos.turn) {
                    continue;
                }
                let index = parent.index();
                if distance[index] != 0 {
                    continue;
                }
                if value < 0 {
                    distance[index] = 1 - value;
                } else {
                    let degree = &mut outdegree[index - start];
                    assert!(*degree > 0);
                    *degree -= 1;
                    if *degree != 0 {
                        continue;
                    }
                    distance[index] = -1 - value;
                }
                pending.push_back(index as u32);
            }
        }
    }
    distance
}
