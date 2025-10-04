#!/usr/bin/env python3
"""
Play ChessEngine against Stockfish and analyze for blunders.
Requires: pip install python-chess
"""

import chess
import chess.engine
import chess.pgn
import subprocess
import time
import io
import sys
from pathlib import Path

class UCIEngine:
    """Wrapper for UCI chess engines"""
    
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
        print(f"→ {command}")
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()
    
    def _wait_for(self, expected):
        while True:
            line = self.process.stdout.readline().strip()
            print(f"← {line}")
            if line == expected:
                break
    
    def get_best_move(self, board, time_ms=2000):
        """Get best move for current position"""
        fen = board.fen()
        self._send(f"position fen {fen}")
        self._send(f"go movetime {time_ms}")
        
        while True:
            line = self.process.stdout.readline().strip()
            print(f"← {line}")
            if line.startswith("bestmove"):
                move_str = line.split()[1]
                if move_str == "(none)":
                    return None
                try:
                    return chess.Move.from_uci(move_str)
                except:
                    return None
    
    def get_evaluation(self, board, depth=12):
        """Get position evaluation in centipawns"""
        fen = board.fen()
        self._send(f"position fen {fen}")
        self._send(f"go depth {depth}")
        
        score = 0
        while True:
            line = self.process.stdout.readline().strip()
            print(f"← {line}")
            if line.startswith("info") and "score cp" in line:
                parts = line.split()
                try:
                    cp_idx = parts.index("cp") + 1
                    score = int(parts[cp_idx])
                except:
                    pass
            elif line.startswith("bestmove"):
                return score
    
    def quit(self):
        self._send("quit")
        self.process.wait()

def format_score(score):
    """Format centipawn score for display"""
    if abs(score) > 9000:
        if score > 0:
            return f"+M{(10000 - score) // 2}"
        else:
            return f"-M{(score + 10000) // 2}"
    else:
        return f"{score/100:+.2f}"

def is_blunder(prev_score, new_score, threshold=200):
    """Check if move is a blunder (evaluation drops by threshold centipawns)"""
    # Flip score for opponent's perspective
    return (prev_score - (-new_score)) >= threshold

def play_game():
    """Play a full game between ChessEngine and Stockfish"""
    
    print("Starting game: ChessEngine (White) vs Stockfish (Black)")
    print("=" * 60)
    
    # Check if engines exist
    chess_engine_path = "./target/release/chess_engine"
    if not Path(chess_engine_path).exists():
        print("Building ChessEngine...")
        subprocess.run(["cargo", "build", "--release"], check=True)
    
    # Initialize engines
    try:
        chess_engine = UCIEngine([chess_engine_path])
        print("ChessEngine initialized successfully")
    except Exception as e:
        print(f"Failed to start ChessEngine: {e}")
        return
    
    try:
        stockfish = chess.engine.SimpleEngine.popen_uci("/usr/games/stockfish")
        print("Stockfish initialized successfully")
    except Exception as e:
        print(f"Failed to start Stockfish: {e}")
        print("Please install Stockfish or ensure it's in PATH")
        chess_engine.quit()
        return
    
    # Game setup
    board = chess.Board()
    game = chess.pgn.Game()
    game.headers["Event"] = "ChessEngine vs Stockfish"
    game.headers["Date"] = time.strftime("%Y.%m.%d")
    game.headers["White"] = "ChessEngine"
    game.headers["Black"] = "Stockfish"
    
    move_times = {"white": 2000, "black": 1500}  # ms per move
    node = game
    
    blunders = []
    prev_eval = 0
    move_count = 0
    
    print("\nGame begins!")
    print(board)
    print()
    
    while not board.is_game_over() and move_count < 100:  # Max 100 moves to prevent infinite games
        move_count += 1
        
        if board.turn == chess.WHITE:
            # ChessEngine's turn
            print(f"Move {move_count}: ChessEngine thinking...")
            start_time = time.time()
            move = chess_engine.get_best_move(board, move_times["white"])
            think_time = time.time() - start_time
            
            if move is None:
                print("ChessEngine couldn't find a move!")
                break
                
            # Get evaluation after move
            board.push(move)
            current_eval = chess_engine.get_evaluation(board, depth=10)
            
            print(f"ChessEngine plays: {move} (eval: {format_score(current_eval)}, time: {think_time:.1f}s)")
            
            # Check for blunder
            if move_count > 1 and is_blunder(prev_eval, current_eval):
                blunder_info = {
                    'move_num': move_count,
                    'player': 'ChessEngine',
                    'move': str(move),
                    'prev_eval': prev_eval,
                    'new_eval': current_eval,
                    'eval_drop': prev_eval - (-current_eval)
                }
                blunders.append(blunder_info)
                print(f"🚨 BLUNDER DETECTED! Evaluation dropped by {blunder_info['eval_drop']} centipawns")
            
            prev_eval = current_eval
            
        else:
            # Stockfish's turn
            print(f"Move {move_count}: Stockfish thinking...")
            start_time = time.time()
            
            result = stockfish.play(board, chess.engine.Limit(time=move_times["black"]/1000))
            move = result.move
            think_time = time.time() - start_time
            
            if move is None:
                print("Stockfish couldn't find a move!")
                break
            
            board.push(move)
            
            # Get evaluation from Stockfish's perspective
            info = stockfish.analyse(board, chess.engine.Limit(depth=12))
            score = info["score"].relative
            if score.is_mate():
                current_eval = 9999 if score.mate() > 0 else -9999
            else:
                current_eval = score.score()
            
            print(f"Stockfish plays: {move} (eval: {format_score(-current_eval)}, time: {think_time:.1f}s)")
            
            # Check for blunder (from Stockfish's perspective)
            if move_count > 1 and is_blunder(-prev_eval, -current_eval):
                blunder_info = {
                    'move_num': move_count,
                    'player': 'Stockfish',
                    'move': str(move),
                    'prev_eval': -prev_eval,
                    'new_eval': -current_eval,
                    'eval_drop': -prev_eval - current_eval
                }
                blunders.append(blunder_info)
                print(f"🚨 BLUNDER DETECTED! Evaluation dropped by {blunder_info['eval_drop']} centipawns")
            
            prev_eval = current_eval
        
        # Add move to PGN
        node = node.add_variation(move)
        
        # Show position every 5 moves or after blunders
        if move_count % 10 == 0 or (blunders and blunders[-1]['move_num'] == move_count):
            print(f"\nPosition after move {move_count}:")
            print(board)
            print()
        
        time.sleep(0.5)  # Brief pause between moves
    
    # Game over
    print("\n" + "=" * 60)
    print("GAME OVER")
    print(f"Result: {board.result()}")
    print(f"Reason: {board.outcome().termination if board.outcome() else 'Max moves reached'}")
    
    # Final position
    print("\nFinal position:")
    print(board)
    
    # Blunder analysis
    print(f"\n🔍 BLUNDER ANALYSIS ({len(blunders)} blunders detected):")
    if not blunders:
        print("✅ No major blunders detected! Both engines played well.")
    else:
        for i, blunder in enumerate(blunders, 1):
            print(f"{i}. Move {blunder['move_num']} - {blunder['player']} played {blunder['move']}")
            print(f"   Evaluation: {format_score(blunder['prev_eval'])} → {format_score(blunder['new_eval'])} "
                  f"(dropped by {blunder['eval_drop']} cp)")
    
    # Save PGN
    pgn_file = "chess_engine_vs_stockfish.pgn"
    with open(pgn_file, "w") as f:
        print(game, file=f)
    print(f"\n📁 Game saved to {pgn_file}")
    
    # Cleanup
    chess_engine.quit()
    stockfish.quit()

if __name__ == "__main__":
    try:
        play_game()
    except KeyboardInterrupt:
        print("\n\nGame interrupted by user.")
    except Exception as e:
        print(f"\nError occurred: {e}")
        import traceback
        traceback.print_exc()