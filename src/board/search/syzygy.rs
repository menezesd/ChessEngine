//! Optional Syzygy probes at the root and after moves that reset the clock.

use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};

use fathom_syzygy_sys as fathom;

use super::{Board, Move, MATE_SCORE};
use crate::board::draw::FIFTY_MOVE_RULE_HALFMOVES;
use crate::board::{Color, Piece};

const TB_WIN_SCORE: i32 = 1_000;
const FIRST_OR_LAST_RANK: u64 = 0xff00_0000_0000_00ff;

// Fathom owns process-global tables. Only this owner may initialize or
// release them. Workers share read access for thread-safe WDL probes.
static IN_USE: AtomicBool = AtomicBool::new(false);

pub(super) struct Tablebases {
    path: CString,
    max_pieces: u32,
}

struct RootMove {
    mv: Move,
    lower: i32,
    upper: i32,
    preference: i32,
}

fn proven_lines(mut moves: Vec<RootMove>, line_count: usize) -> Option<Vec<(Move, i32)>> {
    moves.sort_by_key(|m| std::cmp::Reverse((m.upper, m.lower, m.preference)));
    moves
        .into_iter()
        .take(line_count)
        .map(|m| (m.lower == m.upper).then_some((m.mv, m.lower)))
        .collect()
}

impl Tablebases {
    pub(super) fn open(path: &str) -> Result<Self, &'static str> {
        let path = checked_path(path)?;
        if IN_USE.swap(true, Ordering::AcqRel) {
            return Err("another search state already owns the Syzygy tables");
        }
        // Construct the owner before initializing so errors release the slot.
        let mut tables = Self {
            path,
            max_pieces: 0,
        };
        tables.max_pieces = Self::initialize(&tables.path).ok_or("no Syzygy tables loaded")?;
        Ok(tables)
    }

    pub(super) fn reload(&mut self, path: &str) -> Result<(), &'static str> {
        let path = checked_path(path)?;
        if let Some(max_pieces) = Self::initialize(&path) {
            self.path = path;
            self.max_pieces = max_pieces;
            Ok(())
        } else {
            self.max_pieces = if self.path == path {
                0
            } else {
                Self::initialize(&self.path).unwrap_or_default()
            };
            Err("no Syzygy tables loaded")
        }
    }

    fn initialize(path: &CStr) -> Option<u32> {
        // SAFETY: only the current owner calls this operation, and `path`
        // remains a valid C string throughout the call. Fathom copies it.
        let max_pieces = unsafe {
            if !fathom::tb_init(path.as_ptr().cast()) {
                return None;
            }
            fathom::TB_LARGEST
        };
        (max_pieces > 0).then_some(max_pieces)
    }

    pub(super) fn may_cover(&self, board: &Board) -> bool {
        board.castling_rights == 0
            && (3..=self.max_pieces.min(7)).contains(&board.all_occupied.popcount())
            && board.halfmove_clock() < FIFTY_MOVE_RULE_HALFMOVES
    }

    fn valid_root(&self, board: &Board) -> bool {
        self.may_cover(board)
            && Color::BOTH
                .iter()
                .all(|&side| board.piece_count(side, Piece::King) == 1)
            && !board.is_in_check(board.side_to_move().opponent())
            && board.all_pieces_of_type(Piece::Pawn).0 & FIRST_OR_LAST_RANK == 0
    }

    pub(super) fn rank_moves(
        &mut self,
        board: &mut Board,
        allowed_moves: &[Move],
        line_count: usize,
    ) -> Option<Vec<(Move, i32)>> {
        if allowed_moves.is_empty() || !self.valid_root(board) {
            return None;
        }
        let pawns = board.all_pieces_of_type(Piece::Pawn).0;
        let ep = en_passant_square(board);
        let clock = board.halfmove_clock();
        let mut results = [fathom::TB_RESULT_FAILED; 256];
        // SAFETY: the unique handle and its mutable borrow exclude any other
        // Fathom operation. The validated position has at most seven pieces.
        // The output buffer exceeds Fathom's root move capacity of 193.
        let root = unsafe {
            fathom::tb_probe_root(
                board.occupied_by(Color::White).0,
                board.occupied_by(Color::Black).0,
                board.all_pieces_of_type(Piece::King).0,
                board.all_pieces_of_type(Piece::Queen).0,
                board.all_pieces_of_type(Piece::Rook).0,
                board.all_pieces_of_type(Piece::Bishop).0,
                board.all_pieces_of_type(Piece::Knight).0,
                pawns,
                clock,
                0,
                ep,
                u8::from(board.white_to_move),
                results.as_mut_ptr(),
            )
        };
        if matches!(
            root,
            fathom::TB_RESULT_FAILED | fathom::TB_RESULT_CHECKMATE | fathom::TB_RESULT_STALEMATE
        ) {
            return None;
        }
        let mut ranked = Vec::with_capacity(allowed_moves.len());
        let has_repetition = board.repetition_counts.has_repeated_position();
        for &mv in allowed_moves {
            let code = results
                .iter()
                .copied()
                .take_while(|&code| code != fathom::TB_RESULT_FAILED)
                .find(|&code| matches_move(code, mv))?;
            let wdl = code & fathom::TB_RESULT_WDL_MASK;
            let dtz = (code >> fathom::TB_RESULT_DTZ_SHIFT) as i32;
            let undo = board.make_move(mv);
            let mate = board.is_checkmate();
            let zeroed = board.halfmove_clock() == 0;
            let draw = !mate
                && (board.is_theoretical_draw()
                    || (wdl == fathom::TB_WIN
                        && has_repetition
                        && super::endgame::can_claim_repetition(board)));
            board.unmake_move(mv, undo);
            let at_rounded_boundary =
                !zeroed && clock > 0 && dtz as u32 + clock == FIFTY_MOVE_RULE_HALFMOVES;
            let (lower, upper, preference) = if mate {
                (MATE_SCORE - 1, MATE_SCORE - 1, 0)
            } else if draw {
                (0, 0, 0)
            } else {
                match wdl {
                    // A rounded DTZ can understate the next zeroing distance
                    // by a ply. With a running clock that can turn a result
                    // on the boundary into a draw.
                    fathom::TB_WIN if at_rounded_boundary => (0, TB_WIN_SCORE, -dtz),
                    fathom::TB_LOSS if at_rounded_boundary => (-TB_WIN_SCORE, 0, dtz),
                    fathom::TB_WIN => (TB_WIN_SCORE, TB_WIN_SCORE, -dtz),
                    fathom::TB_CURSED_WIN => (0, 0, 1),
                    fathom::TB_DRAW => (0, 0, 0),
                    fathom::TB_BLESSED_LOSS => (0, 0, -1),
                    fathom::TB_LOSS => (-TB_WIN_SCORE, -TB_WIN_SCORE, dtz),
                    _ => return None,
                }
            };
            ranked.push(RootMove {
                mv,
                lower,
                upper,
                preference,
            });
        }
        proven_lines(ranked, line_count)
    }

    /// Use WDL alone whenever it proves the requested root ordering. With a
    /// running halfmove clock, a theoretical win or loss can become a draw.
    pub(super) fn rank_wdl(
        &mut self,
        board: &mut Board,
        allowed_moves: &[Move],
        line_count: usize,
    ) -> Option<Vec<(Move, i32)>> {
        if allowed_moves.is_empty() || !self.valid_root(board) {
            return None;
        }
        let mut ranked = Vec::with_capacity(allowed_moves.len());
        for &mv in allowed_moves {
            let undo = board.make_move(mv);
            let bounds = if board.is_checkmate() {
                Some((MATE_SCORE - 1, MATE_SCORE - 1))
            } else if board.is_theoretical_draw() {
                Some((0, 0))
            } else {
                self.wdl_move_bounds(board)
            };
            board.unmake_move(mv, undo);
            let (lower, upper) = bounds.unwrap_or((-TB_WIN_SCORE, TB_WIN_SCORE));
            ranked.push(RootMove {
                mv,
                lower,
                upper,
                preference: 0,
            });
        }
        proven_lines(ranked, line_count)
    }

    pub(super) fn zeroing_move_score(&self, child: &Board) -> Option<i32> {
        debug_assert_eq!(child.halfmove_clock(), 0);
        let (lower, upper) = self.wdl_move_bounds(child)?;
        (lower == upper).then_some(lower)
    }

    fn wdl_move_bounds(&self, child: &Board) -> Option<(i32, i32)> {
        if !self.valid_root(child) {
            return None;
        }
        let ep = en_passant_square(child);
        // SAFETY: the position is validated. Fathom allows concurrent WDL
        // probes, and shared ownership excludes root probes or reloading.
        let wdl = unsafe {
            fathom::tb_probe_wdl(
                child.occupied_by(Color::White).0,
                child.occupied_by(Color::Black).0,
                child.all_pieces_of_type(Piece::King).0,
                child.all_pieces_of_type(Piece::Queen).0,
                child.all_pieces_of_type(Piece::Rook).0,
                child.all_pieces_of_type(Piece::Bishop).0,
                child.all_pieces_of_type(Piece::Knight).0,
                child.all_pieces_of_type(Piece::Pawn).0,
                0,
                0,
                ep,
                u8::from(child.white_to_move),
            )
        };
        let zeroed = child.halfmove_clock() == 0;
        match wdl {
            fathom::TB_LOSS if zeroed => Some((TB_WIN_SCORE, TB_WIN_SCORE)),
            fathom::TB_LOSS => Some((0, TB_WIN_SCORE)),
            fathom::TB_WIN if zeroed => Some((-TB_WIN_SCORE, -TB_WIN_SCORE)),
            fathom::TB_WIN => Some((-TB_WIN_SCORE, 0)),
            fathom::TB_BLESSED_LOSS | fathom::TB_CURSED_WIN | fathom::TB_DRAW => Some((0, 0)),
            _ => None,
        }
    }
}

fn en_passant_square(board: &Board) -> u32 {
    board
        .en_passant_target
        .filter(|&sq| board.en_passant_hash_component(sq, board.side_to_move()) != 0)
        .map_or(0, |sq| sq.index() as u32)
}

impl Drop for Tablebases {
    fn drop(&mut self) {
        // Reset the maps and retain Fathom's process-wide entry arrays.
        // The bundled tb_free leaves dangling pointers to those arrays,
        // so a later initialization would reuse freed storage.
        // SAFETY: the handle still exclusively owns Fathom until release.
        unsafe { fathom::tb_init(c"".as_ptr().cast()) };
        IN_USE.store(false, Ordering::Release);
    }
}

fn checked_path(path: &str) -> Result<CString, &'static str> {
    let separator = if cfg!(windows) { ';' } else { ':' };
    // The bundled path parser expects every path component to have a name.
    if path.split(separator).any(str::is_empty) {
        return Err("empty directory in Syzygy path");
    }
    CString::new(path).map_err(|_| "invalid Syzygy path")
}

fn matches_move(code: u32, mv: Move) -> bool {
    // The sys crate swapped the EP and promotion mask definitions. These
    // literal fields match the bundled C header's TB_GET_PROMOTES and TB_GET_EP.
    let promotion = match mv.promotion() {
        None => 0,
        Some(Piece::Queen) => 1,
        Some(Piece::Rook) => 2,
        Some(Piece::Bishop) => 3,
        Some(Piece::Knight) => 4,
        Some(_) => return false,
    };
    ((code >> 16) & 7) == promotion
        && ((code >> 19) & 1 != 0) == mv.is_en_passant()
        && ((code >> 10) & 63) as usize == mv.from().index()
        && ((code >> 4) & 63) as usize == mv.to().index()
}
