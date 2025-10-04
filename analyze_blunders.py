#!/usr/bin/env python3
"""
Analyze ChessEngine blunders to determine if they're caused by:
1. Search issues (depth, time management, pruning)
2. Evaluation issues (piece values, positional evaluation)
3. Move generation or UCI communication issues
"""

import chess
import chess.engine
import subprocess
import time
import sys

class UCIEngine:
    """Wrapper for UCI chess engines with detailed analysis"""
    
    def __init__(self, command):
        self.process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )
        self._send("uci")
        self._wait_for("uciok")
        self._send("isready")
        self._wait_for("readyok")
    
    def _send(self, command):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()
    
    def _wait_for(self, expected):
        while True:
            line = self.process.stdout.readline().strip()
            if line == expected:
                break
    
    def get_best_move_with_info(self, board, time_ms=5000):
        """Get best move with detailed search info"""
        fen = board.fen()
        self._send(f"position fen {fen}")
        self._send(f"go movetime {time_ms}")
        
        info_lines = []
        best_move = None
        
        while True:
            line = self.process.stdout.readline().strip()
            if line.startswith("info"):
                info_lines.append(line)
            elif line.startswith("bestmove"):
                parts = line.split()
                if len(parts) >= 2:
                    best_move = parts[1]
                break
        
        return best_move, info_lines

def test_basic_tactics():
    """Test if engine can solve basic tactical puzzles"""
    print("=== BASIC TACTICAL TEST ===")
    
    engine = UCIEngine(["./target/release/chess_engine"])
    
    # Test 1: Mate in 1 - should find Qh5#
    print("\n1. Mate in 1 test (Qh5#):")
    board = chess.Board("rnbqk2r/pppp1ppp/5n2/2b1p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 0 4")
    print(f"Position: {board.fen()}")
    best_move, info = engine.get_best_move_with_info(board, 3000)
    print(f"Engine move: {best_move}")
    print(f"Expected: Qh5# (mate in 1)")
    if info:
        print(f"Last search info: {info[-1] if info else 'None'}")
    
    # Test 2: Hanging queen - should NOT take with check when queen hangs
    print("\n2. Hanging queen test:")
    board = chess.Board("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
    # Set up position where taking queen hangs our queen
    board = chess.Board("r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/3P1N2/PPP2PPP/RNBQK2R w KQkq - 0 5")
    print(f"Position: {board.fen()}")
    best_move, info = engine.get_best_move_with_info(board, 3000)
    print(f"Engine move: {best_move}")
    if info:
        print(f"Last search info: {info[-1] if info else 'None'}")

def test_evaluation_consistency():
    """Test if evaluation is consistent and reasonable"""
    print("\n=== EVALUATION CONSISTENCY TEST ===")
    
    engine = UCIEngine(["./target/release/chess_engine"])
    
    positions = [
        ("Starting position", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("Up a queen", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBK1BNR w KQkq - 0 1"), 
        ("Down a queen", "rnbk1bnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("Mate in 1", "rnbqk2r/pppp1ppp/5n2/2b1p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 0 4")
    ]
    
    for name, fen in positions:
        print(f"\n{name}:")
        board = chess.Board(fen)
        best_move, info = engine.get_best_move_with_info(board, 2000)
        
        # Extract evaluation from search info
        eval_score = None
        for line in info:
            if "score cp" in line:
                try:
                    parts = line.split()
                    cp_idx = parts.index("cp")
                    if cp_idx + 1 < len(parts):
                        eval_score = int(parts[cp_idx + 1])
                except:
                    pass
            elif "score mate" in line:
                try:
                    parts = line.split()
                    mate_idx = parts.index("mate")
                    if mate_idx + 1 < len(parts):
                        mate_moves = int(parts[mate_idx + 1])
                        eval_score = f"mate in {mate_moves}"
                except:
                    pass
        
        print(f"  Best move: {best_move}")
        print(f"  Evaluation: {eval_score}")

def test_time_management():
    """Test if engine uses time properly"""
    print("\n=== TIME MANAGEMENT TEST ===")
    
    engine = UCIEngine(["./target/release/chess_engine"])
    board = chess.Board()
    
    times = [500, 1000, 2000, 5000]
    
    for time_ms in times:
        print(f"\nTime limit: {time_ms}ms")
        start_time = time.time()
        best_move, info = engine.get_best_move_with_info(board, time_ms)
        actual_time = (time.time() - start_time) * 1000
        
        print(f"  Move: {best_move}")
        print(f"  Actual time: {actual_time:.0f}ms")
        print(f"  Time usage: {actual_time/time_ms*100:.1f}%")

def analyze_blunder_game():
    """Analyze the specific blunder game"""
    print("\n=== ANALYZING BLUNDER GAME ===")
    
    moves = ["e3", "Nf6", "Qf3", "d5", "Bd3", "e5", "Nc3", "e4", "Bb5+", "c6", 
             "Nxd5", "exf3", "Ne2", "Qxd5", "g3", "fxe2", "Rg1", "cxb5", 
             "d3", "Qf3", "Bd2", "Ng4", "Bc1", "Qxf2+", "Kd2", "e1=Q#"]
    
    engine = UCIEngine(["./target/release/chess_engine"])
    board = chess.Board()
    
    print("Analyzing each engine move:")
    
    for i in range(0, len(moves), 2):
        if i + 1 < len(moves):
            white_move = moves[i]
            black_move = moves[i + 1]
            
            # Make white move and analyze
            move = chess.Move.from_uci(white_move) if len(white_move) == 4 else None
            if not move:
                # Try to parse algebraic notation
                try:
                    move = board.parse_san(white_move)
                except:
                    print(f"Could not parse move: {white_move}")
                    continue
            
            print(f"\nMove {i//2 + 1}. {white_move}")
            print(f"Position before: {board.fen()}")
            
            # Get engine's evaluation of this position
            best_move, info = engine.get_best_move_with_info(board, 3000)
            print(f"Engine suggests: {best_move}")
            print(f"Engine played: {white_move}")
            
            if best_move and best_move != white_move:
                print(f"*** BLUNDER: Engine played {white_move} instead of {best_move} ***")
            
            # Make the moves
            board.push(move)
            if black_move and i + 1 < len(moves):
                try:
                    black_move_obj = board.parse_san(black_move)
                    board.push(black_move_obj)
                except:
                    print(f"Could not parse black move: {black_move}")
                    break

def main():
    print("ChessEngine Blunder Analysis")
    print("=" * 40)
    
    # Build engine first
    print("Building engine...")
    result = subprocess.run(["cargo", "build", "--release"], capture_output=True, text=True)
    if result.returncode != 0:
        print("Failed to build engine!")
        print(result.stderr)
        return
    
    try:
        test_basic_tactics()
        test_evaluation_consistency() 
        test_time_management()
        analyze_blunder_game()
        
    except Exception as e:
        print(f"Error during analysis: {e}")
        import traceback
        traceback.print_exc()

if __name__ == "__main__":
    main()