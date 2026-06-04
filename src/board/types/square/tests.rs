use super::*;

#[test]
fn test_square_new() {
    let sq = Square::new(0, 0);
    assert_eq!(sq.rank(), 0);
    assert_eq!(sq.file(), 0);

    let sq = Square::new(7, 7);
    assert_eq!(sq.rank(), 7);
    assert_eq!(sq.file(), 7);
}

#[test]
fn test_square_try_new() {
    assert!(Square::try_new(0, 0).is_some());
    assert!(Square::try_new(7, 7).is_some());
    assert!(Square::try_new(8, 0).is_none());
    assert!(Square::try_new(0, 8).is_none());
}

#[test]
fn test_square_index() {
    let a1 = Square::new(0, 0);
    assert_eq!(a1.index(), 0);
    assert_eq!(a1.as_index(), 0);

    let h8 = Square::new(7, 7);
    assert_eq!(h8.index(), 63);
}

#[test]
fn test_square_from_index() {
    let sq = Square::from_index(0);
    assert_eq!(sq.rank(), 0);
    assert_eq!(sq.file(), 0);

    let sq = Square::from_index(63);
    assert_eq!(sq.rank(), 7);
    assert_eq!(sq.file(), 7);
}

#[test]
fn test_square_flip_vertical() {
    let a1 = Square::new(0, 0);
    let a8 = a1.flip_vertical();
    assert_eq!(a8.rank(), 7);
    assert_eq!(a8.file(), 0);
}

#[test]
fn test_square_flip_horizontal() {
    let a1 = Square::new(0, 0);
    let h1 = a1.flip_horizontal();
    assert_eq!(h1.rank(), 0);
    assert_eq!(h1.file(), 7);
}

#[test]
fn test_square_forward_white() {
    let e4 = Square::new(3, 4);
    let e5 = e4.forward(true).unwrap();
    assert_eq!(e5.rank(), 4);
    assert_eq!(e5.file(), 4);

    let e8 = Square::new(7, 4);
    assert!(e8.forward(true).is_none());
}

#[test]
fn test_square_forward_black() {
    let e5 = Square::new(4, 4);
    let e4 = e5.forward(false).unwrap();
    assert_eq!(e4.rank(), 3);

    let e1 = Square::new(0, 4);
    assert!(e1.forward(false).is_none());
}

#[test]
fn test_square_manhattan_distance() {
    let a1 = Square::new(0, 0);
    let h8 = Square::new(7, 7);
    assert_eq!(a1.manhattan_distance(h8), 14);

    let e4 = Square::new(3, 4);
    assert_eq!(e4.manhattan_distance(e4), 0);
}

#[test]
fn test_square_file_distance() {
    let a1 = Square::new(0, 0);
    let h1 = Square::new(0, 7);
    assert_eq!(a1.file_distance(h1), 7);

    let a1 = Square::new(0, 0);
    let a8 = Square::new(7, 0);
    assert_eq!(a1.file_distance(a8), 0);
}

#[test]
fn test_square_display() {
    let a1 = Square::new(0, 0);
    assert_eq!(a1.to_string(), "a1");

    let h8 = Square::new(7, 7);
    assert_eq!(h8.to_string(), "h8");

    let e4 = Square::new(3, 4);
    assert_eq!(e4.to_string(), "e4");
}

#[test]
fn test_square_from_str() {
    let sq: Square = "a1".parse().unwrap();
    assert_eq!(sq.rank(), 0);
    assert_eq!(sq.file(), 0);

    let sq: Square = "h8".parse().unwrap();
    assert_eq!(sq.rank(), 7);
    assert_eq!(sq.file(), 7);
}

#[test]
fn test_square_from_str_error() {
    assert!("z1".parse::<Square>().is_err());
    assert!("a9".parse::<Square>().is_err());
    assert!("a".parse::<Square>().is_err());
    assert!("a1b".parse::<Square>().is_err());
}

#[test]
fn test_square_try_from_tuple() {
    let sq: Square = (3, 4).try_into().unwrap();
    assert_eq!(sq.rank(), 3);
    assert_eq!(sq.file(), 4);

    assert!(Square::try_from((8, 0)).is_err());
    assert!(Square::try_from((0, 8)).is_err());
}

#[test]
fn test_square_ord() {
    let a1 = Square::new(0, 0);
    let b1 = Square::new(0, 1);
    let a2 = Square::new(1, 0);

    assert!(a1 < b1);
    assert!(b1 < a2);
}
