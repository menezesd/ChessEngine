#!/usr/bin/env python3
"""
Test if the engine (as Black) sees the mate threat in the Blackmar-Diemer Gambit line:
1. d4 d5 2. c4 c6 3. Nc3 e5 4. cxd5 cxd5 5. dxe5 d4 6. Ne4 Qa5+ 7. Nd2 Qxe5 8. Ngf3 Qxe2+ 9. Qxe2+ Ne7 10. Ne4 Nd5 11. Nf6+ Kd8 12. Qe8+ Kc7 13. Nxd5+ Kd6 14. Bf4+ Kc5 15. Qb5#

We test after 14. Bf4+ (Black to move, must avoid mate on Qb5#)
"""
import subprocess
import chess
import sys

# FEN after 14. Bf4+ (White just played Bf4+)
fen = "8/2k1n3/3K4/2p5/3p1B2/8/8/8 b - - 0 14"

print(f"Testing BDG blunder position (Black to move):\nFEN: {fen}")

# Run engine as Black
uci_commands = f"""uci
isready
position fen {fen}
go depth 6
quit
"""

cmd = ["./target/release/chess_engine"]
try:
    result = subprocess.run(cmd, input=uci_commands, text=True, capture_output=True, timeout=15)
    output = result.stdout
    print("\nEngine output:")
    print(output)
    # Extract best move
    best_move = None
    for line in output.split('\n'):
        if line.startswith('bestmove'):
            parts = line.split()
            if len(parts) >= 2 and parts[1] != '(none)':
                best_move = parts[1]
            break
    if best_move:
        print(f"\nEngine best move as Black: {best_move}")
        # Check if this move allows Qb5#
        board = chess.Board(fen)
        move = chess.Move.from_uci(best_move)
        if move in board.legal_moves:
            board.push(move)
            # Can White now play Qb5#?
            mate_found = False
            for m in board.legal_moves:
                if board.san(m) == "Qb5#":
                    mate_found = True
                    break
            if mate_found:
                print("❌ Engine blundered: allows Qb5# mate!")
            else:
                print("✅ Engine avoided immediate mate.")
        else:
            print("❌ Engine produced illegal move.")
    else:
        print("❌ Engine did not return a move.")
except Exception as e:
    print(f"Error running engine: {e}")
    sys.exit(1)
