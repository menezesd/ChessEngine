use super::*;

#[test]
fn test_fen_error_too_few_parts() {
    let err = FenError::TooFewParts { found: 2 };
    assert!(err.to_string().contains('2'));
    assert!(err.to_string().contains('4'));
}

#[test]
fn test_fen_error_invalid_piece() {
    let err = FenError::InvalidPiece { char: 'z' };
    assert!(err.to_string().contains("'z'"));
}

#[test]
fn test_fen_error_invalid_castling() {
    let err = FenError::InvalidCastling { char: 'x' };
    assert!(err.to_string().contains("'x'"));
}

#[test]
fn test_fen_error_invalid_side() {
    let err = FenError::InvalidSideToMove {
        found: "X".to_string(),
    };
    assert!(err.to_string().contains("'X'"));
}

#[test]
fn test_fen_error_equality() {
    let err1 = FenError::TooFewParts { found: 2 };
    let err2 = FenError::TooFewParts { found: 2 };
    assert_eq!(err1, err2);
}

#[test]
fn test_move_error_invalid_length() {
    let err = MoveParseError::InvalidLength { len: 3 };
    assert!(err.to_string().contains('3'));
}

#[test]
fn test_move_error_invalid_square() {
    let err = MoveParseError::InvalidSquare {
        notation: "z9z9".to_string(),
    };
    assert!(err.to_string().contains("z9z9"));
}

#[test]
fn test_move_error_illegal_move() {
    let err = MoveParseError::IllegalMove {
        notation: "e2e5".to_string(),
    };
    assert!(err.to_string().contains("e2e5"));
}

#[test]
fn test_square_error_rank_bounds() {
    let err = SquareError::RankOutOfBounds { rank: 9 };
    assert!(err.to_string().contains('9'));
}

#[test]
fn test_square_error_file_bounds() {
    let err = SquareError::FileOutOfBounds { file: 10 };
    assert!(err.to_string().contains("10"));
}

#[test]
fn test_square_error_invalid_notation() {
    let err = SquareError::InvalidNotation {
        notation: "xyz".to_string(),
    };
    assert!(err.to_string().contains("xyz"));
}

#[test]
fn test_san_error_empty() {
    let err = SanError::Empty;
    assert!(err.to_string().contains("Empty"));
}

#[test]
fn test_san_error_ambiguous() {
    let err = SanError::AmbiguousMove {
        san: "Nc3".to_string(),
    };
    assert!(err.to_string().contains("Nc3"));
}

#[test]
fn test_san_error_no_match() {
    let err = SanError::NoMatchingMove {
        san: "Qh7".to_string(),
    };
    assert!(err.to_string().contains("Qh7"));
}

#[test]
fn test_error_clone() {
    let err = FenError::InvalidPiece { char: 'x' };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}
