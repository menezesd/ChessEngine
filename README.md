# ChessEngine

A simple chess engine written in Rust. It implements standard chess rules, move generation, evaluation, and uses Negamax search with Alpha-Beta pruning, Quiescence Search, and a Transposition Table for finding the best move in a given position.

## Features

* **Standard Chess Rules:** Implements all standard moves including pawn pushes (single/double), captures, en passant, castling (kingside/queenside), and pawn promotions (Queen, Rook, Bishop, Knight).
* **FEN Parsing:** Load positions using Forsyth–Edwards Notation (FEN) strings via `Board::from_fen`.
* **Legal Move Generation:** Accurately generates all legal moves for the current player in a given position.
* **Board Evaluation:** Simple static evaluation based on:
    * Material balance (standard piece values).
    * Piece-Square Tables (PSTs) for positional evaluation based on piece placement.
* **Search Algorithm:**
    * **Negamax:** Core search framework.
    * **Alpha-Beta Pruning:** Optimizes the search by cutting off branches unlikely to yield the best move.
    * **Quiescence Search:** Extends the search horizon for "noisy" moves (captures, promotions) at leaf nodes to avoid the horizon effect and improve tactical accuracy.
* **Transposition Table (TT):**
    * Uses Zobrist hashing for unique(ish) position identification.
    * Stores previously searched positions and their scores/bounds to avoid redundant computation, significantly speeding up search.
    * Supports storing the best move found for improved move ordering (hash move heuristic).
    * Uses incremental Zobrist key updates during make/unmake operations for efficiency.

