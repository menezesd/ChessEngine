//! Benchmarks for chess engine performance.

use std::sync::atomic::AtomicBool;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use chess_engine::board::{find_best_move, Board, SearchState, DEFAULT_TT_MB};

fn bench_perft(c: &mut Criterion) {
    let mut group = c.benchmark_group("perft");

    // Starting position
    let mut board = Board::new();

    for depth in 1..=4 {
        group.bench_with_input(BenchmarkId::new("startpos", depth), &depth, |b, &depth| {
            b.iter(|| board.perft(black_box(depth)))
        });
    }

    // Complex middlegame position (Kiwipete)
    let mut kiwipete =
        Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");

    for depth in 1..=3 {
        group.bench_with_input(BenchmarkId::new("kiwipete", depth), &depth, |b, &depth| {
            b.iter(|| kiwipete.perft(black_box(depth)))
        });
    }

    group.finish();
}

fn bench_movegen(c: &mut Criterion) {
    let mut group = c.benchmark_group("movegen");

    // Starting position
    let mut startpos = Board::new();
    group.bench_function("startpos", |b| {
        b.iter(|| black_box(startpos.generate_moves()))
    });

    // Complex middlegame
    let mut middlegame =
        Board::from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4");
    group.bench_function("middlegame", |b| {
        b.iter(|| black_box(middlegame.generate_moves()))
    });

    // Kiwipete (many moves available)
    let mut kiwipete =
        Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
    group.bench_function("kiwipete", |b| {
        b.iter(|| black_box(kiwipete.generate_moves()))
    });

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.sample_size(10); // Fewer samples for slower benchmarks

    let stop = AtomicBool::new(false);

    // Starting position search
    for depth in [3, 4, 5] {
        group.bench_with_input(BenchmarkId::new("startpos", depth), &depth, |b, &depth| {
            b.iter(|| {
                let mut board = Board::new();
                let mut state = SearchState::new(DEFAULT_TT_MB);
                find_best_move(&mut board, &mut state, depth, &stop)
            })
        });
    }

    // Tactical position
    for depth in [3, 4] {
        group.bench_with_input(BenchmarkId::new("tactical", depth), &depth, |b, &depth| {
            b.iter(|| {
                let mut board = Board::from_fen(
                    "r1bqkb1r/pppp1Qpp/2n2n2/4p3/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 0 4",
                );
                let mut state = SearchState::new(DEFAULT_TT_MB);
                find_best_move(&mut board, &mut state, depth, &stop)
            })
        });
    }

    group.finish();
}

fn bench_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("eval");

    // Various positions to evaluate
    let positions = [
        (
            "startpos",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ),
        (
            "middlegame",
            "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        ),
        ("endgame", "8/5k2/8/8/8/8/5K2/4R3 w - - 0 1"),
    ];

    for (name, fen) in positions {
        let board = Board::from_fen(fen);
        group.bench_with_input(BenchmarkId::new("position", name), &board, |b, board| {
            b.iter(|| black_box(board.evaluate()))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_perft,
    bench_movegen,
    bench_search,
    bench_eval
);
criterion_main!(benches);
