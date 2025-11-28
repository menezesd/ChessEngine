#!/usr/bin/env python3
"""
Simple UCI match runner between two engines.

Usage:
  python3 scripts/uci_match.py [--engine-a PATH] [--engine-b PATH] [--games N] [--movetime MS]

Defaults:
  --engine-a ./target/debug/chess_engine
  --engine-b /usr/games/stockfish

This script performs a simple match by sending UCI commands to both engines and
alternating moves. It's intended for quick testing; it's not a full tournament
controller (no pondering, no resign, no repeat detection). Use small movetime
values for quick matches.
"""

import argparse
import subprocess
import sys
import os
import time


class UCIEngine:
    def __init__(self, cmd):
        self.cmd = cmd
        self.proc = None

    def start(self):
        self.proc = subprocess.Popen(
            self.cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            universal_newlines=True,
        )
        # Run handshake
        self._send('uci')
        self._drain_until('uciok')
        self._send('isready')
        self._drain_until('readyok')

    def _send(self, line: str):
        if self.proc is None:
            raise RuntimeError('engine not started')
        # print(f"-> {line}")
        self.proc.stdin.write(line + '\n')
        self.proc.stdin.flush()

    def _drain_until(self, token: str, timeout=5.0):
        """Read lines until a line starting with token is seen. Returns the buffer."""
        if self.proc is None:
            raise RuntimeError('engine not started')
        out_lines = []
        deadline = time.time() + timeout
        while True:
            if time.time() > deadline:
                raise RuntimeError(f'timeout waiting for {token} from {self.cmd}')
            line = self.proc.stdout.readline()
            if line is None or line == '':
                time.sleep(0.01)
                continue
            line = line.strip()
            out_lines.append(line)
            if line.startswith(token):
                return out_lines

    def set_position(self, moves):
        if not moves:
            self._send('position startpos')
        else:
            self._send('position startpos moves ' + ' '.join(moves))
        self._send('isready')
        self._drain_until('readyok')

    def get_fen(self) -> str:
        self._send('d')
        fen_line = None
        while True:
            line = self.proc.stdout.readline()
            if line is None or line == '':
                time.sleep(0.01)
                continue
            line = line.strip()

            if line.startswith('FEN: '):
                fen_line = line[len('FEN: '):]
                break
            if line.startswith('Fen: '):
                fen_line = line[len('Fen: '):]
                break
            # Continue reading until a FEN line or a "readyok" (for our engine) or timeout
            if line.startswith('readyok'):
                if self.cmd[0].endswith('chess_engine'): # Only break on readyok if it's our engine
                    break
        if fen_line:
            return fen_line
        raise RuntimeError('Fen not found in debug output')

    def go_movetime(self, ms: int):
        self._send(f'go movetime {ms}')
        # read until bestmove
        bestmove = None
        info_lines = []
        while True:
            line = self.proc.stdout.readline()
            if line is None or line == '':
                time.sleep(0.01)
                continue
            line = line.strip()
            info_lines.append(line)
            if line.startswith('bestmove'):
                parts = line.split()
                if len(parts) >= 2:
                    bestmove = parts[1]
                break
        return bestmove, info_lines

    def go_time(self, wtime_ms: int, btime_ms: int, winc_ms: int, binc_ms: int):
        # Send a time-control based 'go' with both sides' times and increments
        self._send(f'go wtime {wtime_ms} btime {btime_ms} winc {winc_ms} binc {binc_ms}')
        bestmove = None
        info_lines = []
        while True:
            line = self.proc.stdout.readline()
            if line is None or line == '':
                time.sleep(0.01)
                continue
            line = line.strip()
            info_lines.append(line)
            if line.startswith('bestmove'):
                parts = line.split()
                if len(parts) >= 2:
                    bestmove = parts[1]
                break
        return bestmove, info_lines

    def quit(self):
        if self.proc is None:
            return
        try:
            self._send('quit')
        except Exception:
            pass
        try:
            self.proc.terminate()
        except Exception:
            pass
        self.proc = None


def run_game(engine_white: UCIEngine, engine_black: UCIEngine, movetime_ms: int, max_moves: int, minutes_per_side: float = None, increment_ms: int = 0):
    moves = []
    engine_white.set_position(moves)
    engine_black.set_position(moves)
    side = 0  # 0 white to move, 1 black

    # Time control: initialize clocks (ms)
    if minutes_per_side is not None:
        wtime = int(minutes_per_side * 60 * 1000)
        btime = int(minutes_per_side * 60 * 1000)
        winc = increment_ms
        binc = increment_ms
    else:
        wtime = btime = None
        winc = binc = 0

    for ply in range(max_moves):
        engine = engine_white if side == 0 else engine_black
        # ensure engine has current position
        engine.set_position(moves)
        # print(f'FEN: {engine.get_fen()}')

        if minutes_per_side is None:
            bestmove, info = engine.go_movetime(movetime_ms)
        else:
            # measure wall-time around go_time call and update clocks accordingly
            t0 = time.time()
            bestmove, info = engine.go_time(wtime, btime, winc, binc)
            t1 = time.time()
            elapsed_ms = int((t1 - t0) * 1000)
            # Subtract elapsed from the side that moved, and add increment after move
            if side == 0:
                wtime = max(0, wtime - elapsed_ms) + winc
            else:
                btime = max(0, btime - elapsed_ms) + binc

        if not bestmove or bestmove == '0000':
            print('Engine returned no move or 0000, terminating game')
            break

        moves.append(bestmove)
        # print(f'Ply {ply+1:>3} {"White" if side==0 else "Black"} -> {bestmove}')

        # Update clocks: approximate by parsing engine info lines for 'bestmove' timing is tricky,
        # so measure wall-time around the go invocation to subtract from the side's clock.
        # (Note: this is an approximation; stronger control would parse 'info' lines.)
        # For now, we do a small wall-time measurement per move.
        # TODO: can be improved by measuring before and after go_time call within the engine object.

        # Update the other engine too
        other = engine_black if side == 0 else engine_white
        other.set_position(moves)
        side = 1 - side
    return moves


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--engine-a', default='./target/debug/chess_engine', help='Path to engine A (default: our engine)')
    parser.add_argument('--engine-b', default='/usr/games/stockfish', help='Path to engine B (default: /usr/games/stockfish)')
    parser.add_argument('--games', type=int, default=1, help='Number of games to play')
    parser.add_argument('--movetime', type=int, default=500, help='Milliseconds per move')
    parser.add_argument('--minutes', type=float, default=None, help='Minutes per side (if set, use time control instead of movetime)')
    parser.add_argument('--increment', type=int, default=0, help='Increment per move in milliseconds (used with --minutes)')
    parser.add_argument('--max-moves', type=int, default=200, help='Maximum plies per game')
    args = parser.parse_args()

    if not os.path.exists(args.engine_a):
        print(f'Engine A binary not found at {args.engine_a}', file=sys.stderr)
        print('Build your engine (e.g. `cargo build`) or pass the path explicitly', file=sys.stderr)
        sys.exit(1)
    if not os.path.exists(args.engine_b):
        print(f'Engine B binary not found at {args.engine_b}', file=sys.stderr)
        sys.exit(1)

    for g in range(args.games):
        print(f'=== Starting game {g+1} ===')
        # For fairness we could swap colors; here engine A is white for odd games
        if g % 2 == 0:
            white_cmd = [args.engine_a]
            black_cmd = [args.engine_b]
        else:
            white_cmd = [args.engine_b]
            black_cmd = [args.engine_a]

        w = UCIEngine(white_cmd)
        b = UCIEngine(black_cmd)
        try:
            print('Starting white engine:', white_cmd)
            w.start()
            print('Starting black engine:', black_cmd)
            b.start()

            moves = run_game(w, b, args.movetime, args.max_moves, minutes_per_side=args.minutes, increment_ms=args.increment)
            print('Game finished. Moves:')
            print(' '.join(moves))
        except Exception as e:
            print('Error during game:', e)
        finally:
            w.quit()
            b.quit()


if __name__ == '__main__':
    main()
