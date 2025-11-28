use chess_engine::tablebase::{Outcome, PrecomputedKpkTable, ThreePieceTablebase};
use chess_engine::core::types::{Color, Piece, Square};
use chess_engine::core::board::Board;
use std::fs::{self, File};
use std::io::{BufWriter, Write};

fn main() {
    let out_dir = "target/tablebases";
    let out_file = format!("{}/kpk.tb", out_dir);
    fs::create_dir_all(out_dir).expect("create tablebase dir");

    let mut tb = ThreePieceTablebase::new();
    let mut data = vec![0i8; PrecomputedKpkTable::max_size()];
    let mut count = 0usize;

    for white_to_move in [true, false] {
        for wk in 0u8..64 {
            for bk in 0u8..64 {
                if wk == bk {
                    continue;
                }
                let wr = wk / 8;
                let wf = wk % 8;
                let br = bk / 8;
                let bf = bk % 8;
                if wr.abs_diff(br) <= 1 && wf.abs_diff(bf) <= 1 {
                    continue;
                }
                for pawn_color in 0u8..2 {
                    for psq in 0u8..64 {
                        // Pawn cannot be on home rank of its color
                        if (pawn_color == 0 && psq / 8 == 0) || (pawn_color == 1 && psq / 8 == 7) {
                            continue;
                        }
                        // Pawn cannot overlap a king
                        if psq == wk || psq == bk {
                            continue;
                        }
                        let idx = match PrecomputedKpkTable::idx(white_to_move, wk, bk, pawn_color, psq) {
                            Some(i) => i,
                            None => continue,
                        };

                        let mut board = Board::empty();
                        // Place kings
                        board.place_piece_at(
                            Square((wk / 8) as usize, (wk % 8) as usize),
                            (Color::White, Piece::King),
                        );
                        board.place_piece_at(
                            Square((bk / 8) as usize, (bk % 8) as usize),
                            (Color::Black, Piece::King),
                        );
                        // Place pawn
                        let color = if pawn_color == 0 { Color::White } else { Color::Black };
                        board.place_piece_at(
                            Square((psq / 8) as usize, (psq % 8) as usize),
                            (color, Piece::Pawn),
                        );
                        board.white_to_move = white_to_move;
                        board.hash = board.calculate_initial_hash();
                        board.position_history.clear();
                        board.position_history.push(board.hash);

                        let outcome = tb.probe(&board).unwrap_or(Outcome::Draw);
                        let val: i8 = match outcome {
                            Outcome::Draw => 0,
                            Outcome::Win(d) => (d.min(120)) as i8,
                            Outcome::Loss(d) => -((d.min(120)) as i8),
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

    let mut writer = BufWriter::new(File::create(&out_file).expect("create tb file"));
    for b in data {
        writer.write_all(&[b as u8]).expect("write tb byte");
    }
    writer.flush().expect("flush tb file");
    eprintln!("Wrote KPK table to {} (entries: {})", out_file, count);
}
