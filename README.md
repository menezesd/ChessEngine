# ChessEngine

A fast, UCI-compatible chess engine written in Rust. The engine uses a
bitboard representation, iterative deepening with alpha-beta pruning, and a
feature-rich evaluation function to provide strong play while remaining easy to
hack on. It can be used as a library, driven from the command line, or plugged
into any UCI-compatible GUI.

## Highlights
- **Modern search:** Negamax with alpha-beta pruning, iterative deepening,
  quiescence search, late-move pruning, and aspiration windows.
- **Smart move ordering:** Hash move, MVV-LVA captures, killer moves, and
  history heuristics to reach cutoffs quickly.
- **Transposition table:** Zobrist hashing with configurable table size and
  automatic re-initialization when the size changes.
- **Evaluation:** Material balance, piece-square tables, tempo bonuses, passed
  pawns, king safety, and mobility scoring.
- **Protocols:** First-class UCI support with options for hash size, thread
  count, pondering, and configurable timing margins. XBoard hooks are available
  via `src/xboard`.
- **Position handling:** FEN parsing/building, legal move generation, make/unmake
  with incremental hashing, and draw/stalemate detection.
- **Parallel search:** Optional symmetric multiprocessing (SMP) search to use
  multiple threads when configured.
- **Extensibility:** The `chess_engine` crate exposes the board, move generation,
  search, and transposition table APIs for embedding in other projects.

## Getting started
1. Install Rust (stable) from [rustup.rs](https://rustup.rs/).
2. Build the engine:
   ```bash
   cargo build --release
   ```
3. Run the UCI console:
   ```bash
   cargo run --release
   ```

From here you can issue standard UCI commands. A typical session looks like:
```
uci              # Print engine id and available options
isready          # Wait for the engine to finish initialization
position startpos moves e2e4 e7e5
setoption name Threads value 4
setoption name Hash value 1024
setoption name Move Overhead value 75
setoption name Ponder value true
isready
perft depth 4    # Verify move generation
go wtime 600000 btime 600000 winc 2000 binc 2000
```

## Library usage
Use the crate directly when you need programmatic access:
```rust
use chess_engine::board::{Board, find_best_move, SearchState};
use std::sync::atomic::AtomicBool;

let mut board = Board::new();
board.make_move_uci("e2e4").unwrap();

let mut state = SearchState::new(256); // transposition table size in MB
let stop = AtomicBool::new(false);
if let Some(best) = find_best_move(&mut board, &mut state, 6, &stop) {
    println!("Best move: {}", best);
}
```

## Configuration and options
- **Hash / Threads:** `setoption name Hash value <mb>` and `setoption name
  Threads value <n>` reconfigure the transposition table and SMP search.
- **Timing:** `Move Overhead`, `Soft Time Percent`, and `Hard Time Percent`
  adjust how conservative the engine is with time usage.
- **Limits:** `Max Nodes` and `MultiPV` control search scope and number of
  principal variations returned.
- **Ponder:** Enable with `setoption name Ponder value true` and use `ponderhit`
  when the GUI transitions from pondering to actual search.
- **Endgames:** Built-in tables cover a pawn, rook, queen, bishop and knight,
  opposite-color bishop pair, or two knights against a bare king. A rook versus
  a pawn, knight, bishop, rook, or queen uses the exact distance to a winning
  capture, pawn move, or checkmate. Compilation generates the tables. Search
  respects the fifty-move clock, reports mating lines when their distance is
  known, and recognizes the wrong-bishop rook-pawn fortress.

The default engine uses 60% fast handcrafted evaluation (HCE) and 40%
embedded NNUE throughout the search. Set `UseNNUE` to `false` for HCE
alone, `EvalFile` to load a compatible network, or `NnueHceBlend` to change
the NNUE percentage. `NnuePureStaticEval` lets pruning and quiescence reuse
the main evaluator and its blend. Disabling it selects HCE for those calls
by default. A loaded `StaticEvalFile` takes precedence, with `NnueStaticBlend`
setting its NNUE percentage. A zero static blend selects HCE.
Build with `--no-default-features` to omit the embedded network. Set either
external file, then enable `UseNNUE` to load it in that build.

### Optional Syzygy tablebases

With a C compiler available, build with
`cargo build --release --features syzygy`. Set the UCI option `SyzygyPath`
to the directory containing your Syzygy `.rtbw` and `.rtbz` files.
Separate multiple directories with `:` on Unix or `;` on Windows.
Set the option to `<empty>` to disable probing.

The engine probes covered root positions of up to seven pieces using the
current fifty-move clock and honors `searchmoves` and `MultiPV`. It can use
WDL files alone when they prove the result for the requested moves. During
search it also probes WDL after a pawn move or capture resets the clock.
The files are downloaded separately. The Rust library API exposes
`SearchState::set_syzygy_path` for one configured search state per process.

## Development
- Run tests: `cargo test`
- Property tests: `cargo test -- --ignored` (for proptest-heavy cases)
- Benchmarks: `cargo bench --bench engine_benchmarks` (requires nightly for
  HTML reports)
- Linting: `cargo clippy --all-targets --all-features`

## HCE distillation

The repository already includes Stockfish-labelled HCE feature data, including
`data/hce_tune_sf_d10_5m_features.csv`; re-labelling is not needed for an
initial tuning pass. Fit candidate multipliers with a held-out report:

```bash
python3 scripts/fit_hce_weights.py \
  --input data/hce_tune_sf_d10_5m_features.csv \
  --target sf_cp --solver ridge \
  --output runs/hce_tune/candidate_weights.csv
```

The fitter only emits the engine's 21 representable HCE term weights, keeps
tempo fixed, excludes draw-scaled positions by default, and bounds each default
weight change. Treat its held-out metric as a screening signal: adopt a
candidate only after same-time-control WAC/STS and broader regression testing.

For a file of previously labelled endgame positions, run
`scripts/label_stockfish_fens.py` with `--depth 18 --screen-depth 14`. It
preserves input order and deepens scores that disagree by at least 40 cp.
Set `--screen-threshold` to adjust that threshold.
