use chess_engine::tablebase::Outcome;
use chess_engine::core::board::Board;
use chess_engine::core::types::Piece;
use chess_engine::core::zobrist::piece_to_zobrist_index;
use std::fs::{self, File};
use std::io::{BufWriter, Write};

/// Build a dense 3-piece table (2 kings + 1 extra piece) and write to target/tablebases/3pc.tb.
///
/// Note: this uses the existing on-the-fly solver, so it can take some time on first run. For now
/// we allow it to run single-threaded; result size ~ few MB.
fn main() {
    let out_dir = "target/tablebases";
    let out_file = format!("{}/3pc.tb", out_dir);
    fs::create_dir_all(out_dir).expect("create tablebase dir");

    let mut tb = chess_engine::tablebase::ThreePieceTablebase::new();

    let mut data = vec![0i8; chess_engine::tablebase::PrecomputedTable::max_size()];

    let mut count = 0usize;

    for white_to_move in [true, false] {
        for wk in 0u8..64 {
            for bk in 0u8..64 {
                if wk == bk {
                    continue;
                }
                // kings not adjacent
                let wr = wk / 8;
                let wf = wk % 8;
                let br = bk / 8;
                let bf = bk % 8;
                if wr.abs_diff(br) <= 1 && wf.abs_diff(bf) <= 1 {
                    continue;
                }
                for piece_color in 0..2 {
                    for piece_idx in 0..6 {
                        if piece_idx == piece_to_zobrist_index(Piece::King) {
                            continue;
                        }
                        let piece_code = (piece_color * 6 + piece_idx) as u8;
                        for psq in 0u8..64 {
                            if psq == wk || psq == bk {
                                continue;
                            }
                            if let Some(idx) = chess_engine::tablebase::PrecomputedTable::index_state(
                                white_to_move,
                                wk,
                                bk,
                                piece_code,
                                psq,
                            ) {
                                let mut board = Board::empty();
                                // Place kings
                                board.place_piece_at(
                                    chess_engine::core::types::Square((wk / 8) as usize, (wk % 8) as usize),
                                    (chess_engine::core::types::Color::White, Piece::King),
                                );
                                board.place_piece_at(
                                    chess_engine::core::types::Square((bk / 8) as usize, (bk % 8) as usize),
                                    (chess_engine::core::types::Color::Black, Piece::King),
                                );
                                // Place extra piece
                                let color = if piece_color == 0 {
                                    chess_engine::core::types::Color::White
                                } else {
                                    chess_engine::core::types::Color::Black
                                };
                                let piece = match piece_idx {
                                    0 => Piece::Pawn,
                                    1 => Piece::Knight,
                                    2 => Piece::Bishop,
                                    3 => Piece::Rook,
                                    4 => Piece::Queen,
                                    _ => Piece::Pawn, // shouldn't happen
                                };
                                board.place_piece_at(
                                    chess_engine::core::types::Square((psq / 8) as usize, (psq % 8) as usize),
                                    (color, piece),
                                );
                                board.white_to_move = white_to_move;
                                board.hash = board.calculate_initial_hash();
                                board.position_history.clear();
                                board.position_history.push(board.hash);

                                let outcome = tb.probe(&board).unwrap_or(Outcome::Draw);
                                let val: i8 = match outcome {
                                    Outcome::Draw => 0,
                                    Outcome::Win(d) => (d.min(120)) as i8,    // clamp
                                    Outcome::Loss(d) => -((d.min(120)) as i8), // clamp
                                };
                                if idx < data.len() {
                                    data[idx] = val;
                                }
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut writer = BufWriter::new(File::create(&out_file).expect("create tb file"));
    for b in data {
        writer.write_all(&[b as u8]).expect("write tb byte");
    }
    writer.flush().expect("flush tb file");
    eprintln!("Wrote precomputed 3pc table to {} (entries: {})", out_file, count);
}
