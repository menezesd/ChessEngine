import subprocess
import chess
import chess.engine
import time

STOCKFISH_PATH = "/usr/games/stockfish"
OUR_ENGINE_PATH = "target/release/chess_engine"  # Adjusted to match binary name

TIME_LIMIT = 5 * 60  # 5 minutes per side in seconds

def play_match():
    board = chess.Board()
    stockfish = chess.engine.SimpleEngine.popen_uci(STOCKFISH_PATH)
    our_engine = chess.engine.SimpleEngine.popen_uci(OUR_ENGINE_PATH)

    white_clock = TIME_LIMIT
    black_clock = TIME_LIMIT
    move_history = []
    result = None
    per_move_time = 5.0  # 1 second per move

    while not board.is_game_over():
        start = time.time()
        if board.turn == chess.WHITE:
            # Our engine as White
            result = our_engine.play(board, chess.engine.Limit(time=per_move_time))
            elapsed = time.time() - start
            white_clock -= elapsed
        else:
            # Stockfish as Black
            result = stockfish.play(board, chess.engine.Limit(time=per_move_time))
            elapsed = time.time() - start
            black_clock -= elapsed
        move = result.move
        move_history.append(move)
        board.push(move)
        print(board)
        print(f"Move: {move}, White clock: {white_clock:.2f}s, Black clock: {black_clock:.2f}s")
        if white_clock <= 0:
            print("White ran out of time!")
            break
        if black_clock <= 0:
            print("Black ran out of time!")
            break

    print("Game over:", board.result())
    print("PGN:")
    print(board.board_fen())
    stockfish.quit()
    our_engine.quit()

if __name__ == "__main__":
    play_match()