Quick script to run simple UCI matches between engines.

Usage:

  # build your engine first
  cargo build

  # run a quick match (our engine vs system Stockfish)
  python3 scripts/uci_match.py --movetime 100 --games 1

Notes:
- The script is intentionally simple and synchronous; it's for quick smoke tests.
- It sends `position startpos` + `go movetime N` commands and reads `bestmove` responses.
- No result adjudication, resigns, or draw detection is implemented.
