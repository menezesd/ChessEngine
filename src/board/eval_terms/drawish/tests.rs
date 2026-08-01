use super::*;

#[test]
fn test_kk_is_draw() {
    // Just two kings
    let board: Board = "8/8/8/4k3/8/8/4K3/8 w - - 0 1".parse().unwrap();
    assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
}

#[test]
fn test_kn_vs_k_is_draw() {
    // King and knight vs lone king
    let board: Board = "8/8/8/4k3/8/8/4K3/4N3 w - - 0 1".parse().unwrap();
    assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
}

#[test]
fn test_kb_vs_k_is_draw() {
    // King and bishop vs lone king
    let board: Board = "8/8/8/4k3/8/8/4K3/4B3 w - - 0 1".parse().unwrap();
    assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
}

#[test]
fn test_knn_vs_k_is_game_theoretic_draw() {
    // Legal mate-in-one positions exist, but the ending cannot force mate.
    let board: Board = "8/8/8/4k3/8/8/4K3/3NN3 w - - 0 1".parse().unwrap();
    assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
}

#[test]
fn test_same_color_bishops_are_a_certain_draw() {
    let board: Board = "7k/8/8/8/8/4B3/8/2B1K3 w - - 0 1".parse().unwrap();

    assert_eq!(board.get_draw_multiplier(Color::White), DRAW_CERTAIN);
}

#[test]
fn test_knight_vs_promotable_pawn_is_not_scaled_as_bare_king_draw() {
    let board: Board = "7k/8/8/8/8/K7/p7/2N5 w - - 0 1".parse().unwrap();

    assert_eq!(board.get_draw_multiplier(Color::White), NO_DRAW_SCALING);
}

#[test]
fn test_kr_vs_km_is_drawish() {
    // Rook vs minor piece - usually draw
    let board: Board = "8/8/8/4k3/8/4n3/4K3/4R3 w - - 0 1".parse().unwrap();
    assert_eq!(board.get_draw_multiplier(Color::White), DRAW_LIKELY);
}

#[test]
fn test_normal_position_no_scaling() {
    // Normal position with pawns
    let board = Board::new();
    assert_eq!(board.get_draw_multiplier(Color::White), NO_DRAW_SCALING);
}

#[test]
fn test_kq_vs_k_is_winning() {
    // Queen vs lone king is winning
    let board: Board = "8/8/8/4k3/8/8/4K3/4Q3 w - - 0 1".parse().unwrap();
    // Strong side has major piece, should not be draw
    assert_eq!(board.get_draw_multiplier(Color::White), NO_DRAW_SCALING);
}
