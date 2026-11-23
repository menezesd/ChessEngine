//! Build-time solver for rook versus any other non-pawn piece.
//! The distance is measured in plies to a winning capture or checkmate.
use super::indexing::{
    major_index, RookPosition as Position, KING_ANCHORS, NO_WIN, ROOK_POSITION_COUNT as STATES,
};
use super::{bishop_attacks, bit, knight_attacks, rook_attacks, squares};
use std::collections::VecDeque;

const OPPONENT: usize = 0;
const ROOK: usize = 1;
const MAX_REPLIES: usize = 35;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Opponent {
    Knight,
    Bishop,
    Rook,
    Queen,
}

impl Position {
    fn from_index(index: usize) -> Self {
        Self {
            kings: [KING_ANCHORS[index >> 19], (index >> 13) & 63],
            pieces: [(index >> 1) & 63, (index >> 7) & 63],
            turn: index & 1,
        }
    }

    fn occupancy(self) -> u64 {
        bit(self.kings[0]) | bit(self.kings[1]) | bit(self.pieces[0]) | bit(self.pieces[1])
    }

    fn relocate(&mut self, side: usize, king: bool, target: usize) -> usize {
        let square = if king {
            &mut self.kings[side]
        } else {
            &mut self.pieces[side]
        };
        std::mem::replace(square, target)
    }
}

struct Attacks<'a> {
    king: &'a [u64; 64],
    opponent: Opponent,
    knight: [u64; 64],
}

impl<'a> Attacks<'a> {
    fn new(king: &'a [u64; 64], opponent: Opponent) -> Self {
        Self {
            king,
            opponent,
            knight: std::array::from_fn(knight_attacks),
        }
    }

    fn piece(&self, side: usize, square: usize, occupancy: u64) -> u64 {
        match if side == ROOK {
            Opponent::Rook
        } else {
            self.opponent
        } {
            Opponent::Knight => self.knight[square],
            Opponent::Bishop => bishop_attacks(square, occupancy),
            Opponent::Rook => rook_attacks(square, occupancy),
            Opponent::Queen => rook_attacks(square, occupancy) | bishop_attacks(square, occupancy),
        }
    }

    fn checked(&self, pos: Position, side: usize, occupancy: u64) -> bool {
        let king = pos.kings[side];
        let enemy = side ^ 1;
        self.king[king] & bit(pos.kings[enemy]) != 0
            || self.piece(enemy, king, occupancy) & bit(pos.pieces[enemy]) != 0
    }
}

/// The capture leaves the losing side with a bare king. Its reply can
/// still draw by stalemate or by taking the rook.
fn winning_capture(
    child: Position,
    opponent: Opponent,
    rook_distances: &[u8],
    queen_distances: &[u8],
) -> bool {
    if child.turn != OPPONENT && matches!(opponent, Opponent::Knight | Opponent::Bishop) {
        return false;
    }
    let mover = child.turn ^ 1;
    let index = major_index(
        child.kings[mover],
        child.kings[child.turn],
        child.pieces[mover],
        false,
    );
    let distances = if mover == OPPONENT && opponent == Opponent::Queen {
        queen_distances
    } else {
        rook_distances
    };
    distances[index] != NO_WIN
}

// Zero means unresolved or drawn. For decisive positions the sign gives
// the side-to-move result and the magnitude is one plus the distance.
pub(super) fn solve(
    king_attacks: &[u64; 64],
    rook_distances: &[u8],
    queen_distances: &[u8],
    opponent: Opponent,
) -> Vec<i8> {
    let attacks = Attacks::new(king_attacks, opponent);
    let mut distance = vec![0i8; STATES];
    let mut outdegree = vec![0u8; STATES];
    let mut queue = VecDeque::new();
    let mut capture_wins = Vec::new();
    for state in 0..STATES {
        let pos = Position::from_index(state);
        let occupied = pos.occupancy();
        let mover = pos.turn;
        let other = mover ^ 1;
        if occupied.count_ones() != 4
            || pos.index() != state
            || attacks.checked(pos, other, occupied)
        {
            continue;
        }

        let own = bit(pos.kings[mover]) | bit(pos.pieces[mover]);
        let targets = !own & !bit(pos.kings[other]);
        let captures = bit(pos.pieces[other]);
        let king = attacks.king[pos.kings[mover]] & targets;
        let piece = attacks.piece(mover, pos.pieces[mover], occupied) & targets;
        let mut seen = [STATES; MAX_REPLIES];
        let mut unique = 0;
        let mut draw_captures = 0;
        for (moving_king, target) in squares(king)
            .map(|sq| (true, sq))
            .chain(squares(piece).map(|sq| (false, sq)))
        {
            let mut child = pos;
            let from = child.relocate(mover, moving_king, target);
            child.turn = other;
            let after = (occupied & !bit(from)) | bit(target);
            if captures & bit(target) != 0 {
                if attacks.king[child.kings[mover]] & bit(pos.kings[other]) != 0 {
                    continue;
                }
                if winning_capture(child, attacks.opponent, rook_distances, queen_distances) {
                    distance[state] = 2;
                    capture_wins.push(state as u32);
                    break;
                }
                draw_captures += 1;
            } else if !attacks.checked(child, mover, after) {
                let index = child.index();
                if !seen[..unique].contains(&index) {
                    seen[unique] = index;
                    unique += 1;
                }
            }
        }

        let degree = unique + draw_captures;
        outdegree[state] = degree as u8;
        if distance[state] == 0 && degree == 0 && attacks.checked(pos, mover, occupied) {
            distance[state] = -1;
            queue.push_back(state as u32);
        }
    }
    // Process checkmates before the one-ply wins. The first proved win
    // gives the shortest conversion, and the last losing reply sets
    // the longest resistance.
    queue.extend(capture_wins);

    while let Some(state) = queue.pop_front() {
        let pos = Position::from_index(state as usize);
        let value = distance[state as usize];
        let mover = pos.turn ^ 1;
        let occupied = pos.occupancy();
        let king = attacks.king[pos.kings[mover]] & !occupied;
        let piece = attacks.piece(mover, pos.pieces[mover], occupied) & !occupied;
        let mut seen = [STATES; MAX_REPLIES];
        let mut unique = 0;
        for (moving_king, target) in squares(king)
            .map(|sq| (true, sq))
            .chain(squares(piece).map(|sq| (false, sq)))
        {
            let mut previous = pos;
            let from = previous.relocate(mover, moving_king, target);
            if attacks.checked(previous, pos.turn, occupied ^ bit(from) ^ bit(target)) {
                continue;
            }
            previous.turn = mover;
            let index = previous.index();
            if distance[index] != 0 || seen[..unique].contains(&index) {
                continue;
            }
            seen[unique] = index;
            unique += 1;
            if value < 0 {
                distance[index] = 1 - value;
            } else {
                assert!(outdegree[index] > 0);
                outdegree[index] -= 1;
                if outdegree[index] != 0 {
                    continue;
                }
                distance[index] = -1 - value;
            }
            queue.push_back(index as u32);
        }
    }
    distance
}
