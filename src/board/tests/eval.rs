//! Evaluation term unit tests.
//!
//! Tests for individual evaluation components:
//! - King safety
//! - Mobility
//! - Passed pawns
//! - Pawn structure
//! - Rook activity

use crate::board::state::Board;

fn make_board(fen: &str) -> Board {
    fen.parse().expect("valid fen")
}

// ============================================================================
// King Safety Tests
// ============================================================================

#[test]
fn test_king_safety_castled_vs_center() {
    // White king castled (safe), black king in center (exposed)
    let castled = make_board("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R4RK1 w kq - 0 1");
    let center = make_board("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R2K3R w kq - 0 1");

    let (mg_castled, _) = castled.eval_king_safety();
    let (mg_center, _) = center.eval_king_safety();

    // Castled position should have better king safety for white
    // (Less negative or more positive score)
    assert!(
        mg_castled >= mg_center,
        "castled={mg_castled}, center={mg_center}"
    );
}

#[test]
fn test_king_safety_under_attack() {
    // Black queen attacking white king zone
    let attacked = make_board("8/8/8/8/8/5q2/5PPP/6K1 w - - 0 1");
    let safe = make_board("8/8/8/8/8/8/5PPP/6K1 w - - 0 1");

    let (mg_attacked, _) = attacked.eval_king_safety();
    let (mg_safe, _) = safe.eval_king_safety();

    // Attacked position should have worse score for white
    assert!(
        mg_attacked < mg_safe,
        "attacked={mg_attacked}, safe={mg_safe}"
    );
}

#[test]
fn test_king_shield_with_pawns() {
    // White king with full pawn shield
    let with_shield = make_board("8/8/8/8/8/8/5PPP/6K1 w - - 0 1");
    let no_shield = make_board("8/8/8/8/8/8/8/6K1 w - - 0 1");

    let (mg_shield, _) = with_shield.eval_king_shield();
    let (mg_no, _) = no_shield.eval_king_shield();

    assert!(
        mg_shield > mg_no,
        "with_shield={mg_shield}, no_shield={mg_no}"
    );
}

#[test]
fn test_king_open_file_penalty() {
    // King on open file (no pawns)
    let open = make_board("8/8/8/8/8/8/PP3PPP/4K3 w - - 0 1");
    let closed = make_board("8/8/8/8/8/8/PPPPPPPP/4K3 w - - 0 1");

    let (mg_open, _) = open.eval_king_shield();
    let (mg_closed, _) = closed.eval_king_shield();

    assert!(mg_open < mg_closed, "open={mg_open}, closed={mg_closed}");
}

// ============================================================================
// Mobility Tests
// ============================================================================

#[test]
fn test_mobility_knight_center_vs_corner() {
    // Knight in center has more squares
    let center = make_board("8/8/8/3N4/8/8/8/8 w - - 0 1");
    let corner = make_board("N7/8/8/8/8/8/8/8 w - - 0 1");

    let (mg_center, eg_center) = center.eval_mobility();
    let (mg_corner, eg_corner) = corner.eval_mobility();

    assert!(
        mg_center > mg_corner,
        "center={mg_center}, corner={mg_corner}"
    );
    assert!(
        eg_center > eg_corner,
        "center={eg_center}, corner={eg_corner}"
    );
}

#[test]
fn test_mobility_bishop_open_vs_blocked() {
    // Bishop on open diagonal vs blocked
    let open = make_board("8/8/8/8/3B4/8/8/8 w - - 0 1");
    let blocked = make_board("8/8/8/2P1P3/3B4/2P1P3/8/8 w - - 0 1");

    let (mg_open, eg_open) = open.eval_mobility();
    let (mg_blocked, eg_blocked) = blocked.eval_mobility();

    assert!(mg_open > mg_blocked, "open={mg_open}, blocked={mg_blocked}");
    assert!(eg_open > eg_blocked, "open={eg_open}, blocked={eg_blocked}");
}

#[test]
fn test_mobility_rook_open_file() {
    // Rook on open file has great mobility
    let open = make_board("8/8/8/8/4R3/8/8/8 w - - 0 1");
    let blocked = make_board("4p3/8/8/8/4R3/8/8/4P3 w - - 0 1");

    let (mg_open, eg_open) = open.eval_mobility();
    let (mg_blocked, eg_blocked) = blocked.eval_mobility();

    assert!(mg_open > mg_blocked, "open={mg_open}, blocked={mg_blocked}");
    assert!(eg_open > eg_blocked, "open={eg_open}, blocked={eg_blocked}");
}

#[test]
fn test_mobility_queen() {
    let queen = make_board("8/8/8/3Q4/8/8/8/8 w - - 0 1");
    let (mg, eg) = queen.eval_mobility();

    // Queen in center should have positive mobility
    assert!(mg > 0, "queen mobility mg={mg}");
    assert!(eg > 0, "queen mobility eg={eg}");
}

#[test]
fn test_mobility_symmetry() {
    // Symmetric position should have ~0 mobility difference
    let board = make_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let (mg, eg) = board.eval_mobility();

    assert!(mg.abs() < 20, "startpos mobility mg={mg}");
    assert!(eg.abs() < 20, "startpos mobility eg={eg}");
}

// ============================================================================
// Passed Pawn Tests
// ============================================================================

#[test]
fn test_passed_pawn_basic() {
    // White has passed pawn on d5
    let board = make_board("8/8/8/3P4/8/8/8/8 w - - 0 1");
    let (mg, eg) = board.eval_passed_pawns();

    assert!(mg > 0, "passed pawn mg={mg}");
    assert!(eg > 0, "passed pawn eg={eg}");
}

#[test]
fn test_passed_pawn_advanced_more_valuable() {
    // More advanced passer is worth more
    // Pawn on rank 5 (4th index for white)
    let rank4 = make_board("8/8/8/8/3P4/8/8/8 w - - 0 1");
    // Pawn on rank 6 (5th index for white)
    let rank6 = make_board("8/8/3P4/8/8/8/8/8 w - - 0 1");

    let (_, eg4) = rank4.eval_passed_pawns();
    let (_, eg6) = rank6.eval_passed_pawns();

    assert!(eg6 > eg4, "rank6={eg6}, rank4={eg4}");
}

#[test]
fn test_passed_pawn_not_passed() {
    // Pawn blocked by enemy pawn on same file - not passed
    let not_passed = make_board("3p4/8/8/3P4/8/8/8/8 w - - 0 1");
    let (mg, eg) = not_passed.eval_passed_pawns();

    // No passed pawns, score should be 0 or close
    assert!(mg.abs() <= 10, "not passed mg={mg}");
    assert!(eg.abs() <= 10, "not passed eg={eg}");
}

#[test]
fn test_passed_pawn_blocked_by_adjacent() {
    // Pawn "passed" but blocked by pawn on adjacent file ahead - still not passed
    let blocked = make_board("8/8/4p3/3P4/8/8/8/8 w - - 0 1");
    let (mg, eg) = blocked.eval_passed_pawns();

    // Enemy pawn on e6 blocks d5 from being a passer
    assert!(mg.abs() <= 10, "blocked passer mg={mg}");
    assert!(eg.abs() <= 10, "blocked passer eg={eg}");
}

#[test]
fn test_passed_pawn_both_sides() {
    // Both sides have passed pawns - should roughly cancel
    let board = make_board("8/3p4/8/8/8/8/3P4/8 w - - 0 1");
    let (mg, eg) = board.eval_passed_pawns();

    // Pawns are at same relative advancement, should roughly cancel
    assert!(mg.abs() < 20, "both passers mg={mg}");
    assert!(eg.abs() < 20, "both passers eg={eg}");
}

// ============================================================================
// Pawn Structure Tests
// ============================================================================

#[test]
fn test_doubled_pawns_penalty() {
    let doubled = make_board("8/8/8/3P4/3P4/8/8/8 w - - 0 1");
    let single = make_board("8/8/8/3P4/8/8/8/8 w - - 0 1");

    let (mg_doubled, _) = doubled.eval_pawn_structure();
    let (mg_single, _) = single.eval_pawn_structure();

    assert!(
        mg_doubled < mg_single,
        "doubled={mg_doubled}, single={mg_single}"
    );
}

#[test]
fn test_isolated_pawn_penalty() {
    // Isolated pawn (no pawns on adjacent files)
    let isolated = make_board("8/8/8/3P4/8/8/8/8 w - - 0 1");
    let supported = make_board("8/8/8/2PP4/8/8/8/8 w - - 0 1");

    let (mg_iso, _) = isolated.eval_pawn_structure();
    let (mg_sup, _) = supported.eval_pawn_structure();

    // Note: doubled pawns also penalized, but isolated is usually worse
    // This test may need adjustment based on actual weights
    assert!(mg_iso <= mg_sup + 20, "iso={mg_iso}, supported={mg_sup}");
}

#[test]
fn test_pawn_structure_symmetry() {
    let board = make_board("8/pppppppp/8/8/8/8/PPPPPPPP/8 w - - 0 1");
    let (mg, eg) = board.eval_pawn_structure();

    // Symmetric pawn structure should be roughly 0
    assert!(mg.abs() < 10, "symmetric pawn structure mg={mg}");
    assert!(eg.abs() < 10, "symmetric pawn structure eg={eg}");
}

// ============================================================================
// Rook Activity Tests
// ============================================================================

#[test]
fn test_rook_open_file_bonus() {
    // Rook on open file (no pawns)
    let open = make_board("8/8/8/8/8/8/8/4R3 w - - 0 1");
    let closed = make_board("8/4p3/8/8/8/8/4P3/4R3 w - - 0 1");

    let (mg_open, eg_open) = open.eval_rooks();
    let (mg_closed, eg_closed) = closed.eval_rooks();

    assert!(mg_open > mg_closed, "open={mg_open}, closed={mg_closed}");
    assert!(eg_open > eg_closed, "open={eg_open}, closed={eg_closed}");
}

#[test]
fn test_rook_semi_open_file() {
    // Rook on semi-open file (enemy pawn but no own pawn)
    let semi_open = make_board("8/4p3/8/8/8/8/8/4R3 w - - 0 1");
    let closed = make_board("8/4p3/8/8/8/8/4P3/4R3 w - - 0 1");

    let (mg_semi, _) = semi_open.eval_rooks();
    let (mg_closed, _) = closed.eval_rooks();

    assert!(mg_semi > mg_closed, "semi={mg_semi}, closed={mg_closed}");
}

#[test]
fn test_rook_seventh_rank_bonus() {
    // Rook on 7th rank (2nd rank for black) is valuable
    let seventh = make_board("8/R7/8/8/8/8/8/8 w - - 0 1");
    let normal = make_board("8/8/8/R7/8/8/8/8 w - - 0 1");

    let (mg_7th, eg_7th) = seventh.eval_rooks();
    let (mg_normal, eg_normal) = normal.eval_rooks();

    assert!(mg_7th >= mg_normal, "7th={mg_7th}, normal={mg_normal}");
    assert!(eg_7th >= eg_normal, "7th={eg_7th}, normal={eg_normal}");
}

// ============================================================================
// Combined Evaluation Tests
// ============================================================================

#[test]
fn test_eval_material_advantage() {
    // White up a queen
    let board = make_board("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let score = board.evaluate();
    assert!(score > 800, "queen advantage score={score}");
}

#[test]
fn test_eval_symmetry() {
    // Symmetric position should evaluate close to 0
    let board = make_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let score = board.evaluate();
    assert!(score.abs() < 50, "startpos eval={score}");
}

#[test]
fn test_eval_perspective() {
    // Evaluation should be from side-to-move perspective
    let white_to_move = make_board("8/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let black_to_move = make_board("8/8/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1");

    let white_eval = white_to_move.evaluate();
    let black_eval = black_to_move.evaluate();

    // When black is to move with same material, score should be negative
    // (from black's perspective, white has all the pieces)
    assert!(black_eval < 0, "black to move eval={black_eval}");
    assert!(white_eval > 0, "white to move eval={white_eval}");
}
