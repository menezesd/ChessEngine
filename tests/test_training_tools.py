"""Regression tests for training-data sampling and UCI label collection."""

from itertools import chain, repeat
from pathlib import Path
import importlib
import queue
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

import chess.pgn

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

import collect_hard_pairs
import label_positions_engine
import label_positions_static_eval
import label_root_children_teacher
import make_training_mix
import sample_pgn_positions
import check_epd_sf_agreement
import label_epd_children_sf
import label_stockfish_fens
import stockfish_label
import blunder_scan
import wac_eval_nnue
import test_checkpoint_wac
import analyze_wac_nnue_disagreements
from uci_client import UCIClient


FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"


def scripted_engine(module, searches, stopped=()):
    cls = module if isinstance(module, type) else module.UCIEngine
    engine = cls.__new__(cls)
    engine.lines = queue.Queue()
    commands = []
    searches = iter(searches)

    def send(command):
        commands.append(command)
        if command.startswith("go "):
            for line in next(searches):
                engine.lines.put(line)
        elif command == "stop":
            for line in stopped:
                engine.lines.put(line)
        elif command == "isready":
            engine.lines.put("readyok")
        elif command == "eval":
            engine.lines.put("info string eval blended 23")

    engine.send = send
    engine._send = send
    engine.quit = lambda: commands.append("quit")
    return engine, commands


class TrainingToolTests(unittest.TestCase):
    def test_wac_epd_parser_respects_operation_boundaries_and_quoted_ids(self):
        epd = " ".join(FEN.split()[:4]) + ' bm e4; am d4; id "opening position one"; hmvc 17; fmvn 42;'
        expected_fen = " ".join(FEN.split()[:4]) + " 17 42"
        self.assertEqual(wac_eval_nnue.parse_epd(epd), (expected_fen, ["e4"], "opening position one"))
        self.assertEqual(test_checkpoint_wac.parse_epd(epd), (expected_fen, ["e4"]))

    def test_wac_tools_skip_blank_lines_and_close_engines_on_failure(self):
        modules = (wac_eval_nnue, test_checkpoint_wac, analyze_wac_nnue_disagreements)
        with tempfile.TemporaryDirectory() as directory:
            epd = Path(directory) / "positions.epd"
            row = " ".join(FEN.split()[:4]) + ' bm e4; id "opening";\n'
            epd.write_text("\n" + row + "\n" + row)
            for module in modules:
                for failure in (False, True):
                    with self.subTest(tool=module.__name__, failure=failure):
                        engine = Mock()
                        engine.bestmove.return_value = "e2e4"
                        engine.static_eval.return_value = "info string eval blended 0"
                        if failure:
                            engine.bestmove.side_effect = RuntimeError("search failed")
                        name = "EvalEngine" if module is analyze_wac_nnue_disagreements else "UCIEngine"
                        argv = ["wac"]
                        if module is test_checkpoint_wac:
                            argv.append("fixture.nnue")
                        argv += ["--epd", str(epd)]
                        with patch.object(module, name, return_value=engine), \
                             patch.object(sys, "argv", argv), patch("builtins.print"):
                            if failure:
                                with self.assertRaisesRegex(RuntimeError, "search failed"):
                                    module.main()
                            else:
                                module.main()
                        if not failure:
                            expected = 4 if module is analyze_wac_nnue_disagreements else 2
                            self.assertEqual(engine.bestmove.call_count, expected)
                        self.assertTrue(engine.quit.called)

    def test_checkpoint_wac_accepts_an_empty_dataset(self):
        with tempfile.TemporaryDirectory() as directory:
            epd = Path(directory) / "empty.epd"
            epd.write_text("")
            engine = Mock()
            with patch.object(test_checkpoint_wac, "UCIEngine", return_value=engine), \
                 patch.object(sys, "argv", ["wac", "fixture.nnue", "--epd", str(epd)]), \
                 patch("builtins.print"):
                test_checkpoint_wac.main()
            engine.quit.assert_called_once()

    def test_wac_clients_stop_and_drain_expired_searches(self):
        for module in (wac_eval_nnue, test_checkpoint_wac):
            with self.subTest(tool=module.__name__):
                engine, commands = scripted_engine(module, [[], ["bestmove d2d4"]], ["bestmove e2e4"])
                ticks = chain([0, 0], repeat(100))
                clock = lambda: next(ticks)
                with patch("time.time", clock), patch("time.monotonic", clock):
                    first = engine.bestmove(FEN, 1)
                self.assertEqual(first, "e2e4")
                self.assertIn("stop", commands)
                self.assertEqual(engine.bestmove(FEN, 1), "d2d4")

    def test_wac_static_eval_consumes_a_ready_barrier(self):
        engine, commands = scripted_engine(analyze_wac_nnue_disagreements.EvalEngine, [])
        self.assertEqual(engine.static_eval(FEN), "info string eval blended 23")
        self.assertEqual(commands[-1], "isready")
        self.assertTrue(engine.lines.empty())

    def test_blunder_comparison_keeps_both_root_scores_in_the_same_perspective(self):
        engine = Mock()
        engine.go.return_value = (500, "e2e4")
        engine.go_searchmoves.return_value = (500, "e2e4")
        with patch.object(blunder_scan, "UCIEngine", return_value=engine), \
             patch.object(sys, "argv", ["scan", "e2e4"]), patch("builtins.print") as output:
            blunder_scan.main()
        report = next(call.args[0] for call in output.call_args_list if str(call.args[0]).startswith("ply"))
        self.assertRegex(report, r"delta\s+\+0 OK$")

    def test_blunder_scan_uses_the_primary_variation_score(self):
        engine, _ = scripted_engine(blunder_scan, [[
            "info depth 2 multipv 1 score cp 100 pv e2e4",
            "info depth 2 multipv 2 score cp -500 pv a2a3",
            "bestmove e2e4",
        ]])
        self.assertEqual(engine.go(1), (100, "e2e4"))

    def test_stockfish_collectors_stop_and_drain_expired_searches(self):
        for cls in (check_epd_sf_agreement.Stockfish, label_epd_children_sf.Stockfish,
                    label_stockfish_fens.Stockfish, stockfish_label.StockfishEngine):
            with self.subTest(collector=cls.__module__):
                engine, commands = scripted_engine(
                    cls, [["info score cp 11"], ["info score cp 22", "bestmove d2d4"]],
                    ["info score cp 17", "bestmove e2e4"],
                )
                engine.depth = 1
                def evaluate():
                    if hasattr(engine, "eval"):
                        score = engine.eval(FEN)
                    else:
                        score = engine.evaluate(FEN, 1)
                    return score[0] if isinstance(score, tuple) else score
                ticks = chain([0, 0], repeat(100))
                clock = lambda: next(ticks)
                with patch("time.time", clock), patch("time.monotonic", clock):
                    first = evaluate()
                self.assertIn("stop", commands)
                self.assertEqual(first, 17)
                self.assertEqual(evaluate(), 22)

    def test_stockfish_collectors_do_not_label_a_mated_side_as_winning(self):
        for cls in (check_epd_sf_agreement.Stockfish, label_epd_children_sf.Stockfish,
                    label_stockfish_fens.Stockfish, stockfish_label.StockfishEngine):
            with self.subTest(collector=cls.__module__):
                engine, _ = scripted_engine(cls, [["info score mate 0", "bestmove 0000"]])
                engine.depth = 1
                score = engine.eval(FEN) if hasattr(engine, "eval") else engine.evaluate(FEN, 1)
                if isinstance(score, tuple):
                    score = score[0]
                self.assertLess(score, 0)

    def test_pgn_reservoir_uses_all_seen_positions(self):
        with tempfile.TemporaryDirectory() as directory:
            game = chess.pgn.Game()
            game.headers["Result"] = "1-0"
            node = game
            board = game.board()
            positions = []
            for san in "e4 e5 Nf3 Nc6 Bb5 a6 Ba4 Nf6 O-O Be7 Re1 b5 Bb3 d6 c3 O-O".split():
                move = board.parse_san(san)
                node = node.add_variation(move)
                board.push(move)
                positions.append(board.fen())
            pgn = Path(directory) / "game.pgn"
            pgn.write_text(str(game))
            all_positions = Path(directory) / "all.txt"
            all_positions.write_text("\n".join(positions) + "\n")
            output = Path(directory) / "sample.txt"
            argv = ["sample", str(pgn), "--output", str(output), "--max-positions", "2",
                    "--skip-plies", "0", "--stride", "1", "--seed", "42"]
            with patch.object(sys, "argv", argv), patch("builtins.print"):
                sample_pgn_positions.main()
            sampled = {line.split(" | ")[0] for line in output.read_text().splitlines()}
            self.assertEqual(sampled, set(make_training_mix.reservoir_sample(str(all_positions), 2, 42)))

    def test_pgn_game_limit_includes_unfinished_games(self):
        with tempfile.TemporaryDirectory() as directory:
            pgn = Path(directory) / "games.pgn"
            pgn.write_text('[Result "*"]\n\n1. e4 *\n\n[Result "1-0"]\n\n1. d4 d5 1-0\n')
            output = Path(directory) / "sample.txt"
            argv = ["sample", str(pgn), "--output", str(output), "--max-games", "1",
                    "--skip-plies", "0", "--stride", "1"]
            with patch.object(sys, "argv", argv), patch("builtins.print"):
                sample_pgn_positions.main()
            self.assertEqual(output.read_text(), "")

    def test_blank_lines_do_not_bias_reservoir_sampling(self):
        with tempfile.TemporaryDirectory() as directory:
            plain = Path(directory) / "plain.txt"
            padded = Path(directory) / "padded.txt"
            plain.write_text("a\nb\nc\nd\ne\nf\n")
            padded.write_text("a\n" + "\n" * 100 + "b\nc\nd\ne\nf\n")
            self.assertEqual(
                make_training_mix.reservoir_sample(str(plain), 2, 42),
                make_training_mix.reservoir_sample(str(padded), 2, 42),
            )

    def test_search_timeout_stops_and_drains_before_the_next_position(self):
        engine, commands = scripted_engine(
            label_positions_engine,
            [["info depth 1 score cp 11"], ["info depth 1 score cp 22", "bestmove d2d4"]],
            ["info depth 2 score cp 17", "bestmove e2e4"],
        )
        ticks = chain([0, 0], repeat(61))
        clock = lambda: next(ticks)
        with patch("time.time", clock), patch("time.monotonic", clock):
            first = engine.score(FEN, 1, 0)
        self.assertIn("stop", commands)
        self.assertEqual(first, 17)
        self.assertEqual(engine.score(FEN, 1, 0), 22)

    def test_label_uses_only_exact_primary_variation_scores(self):
        engine, _ = scripted_engine(label_positions_engine, [[
            "info depth 3 multipv 1 score cp 80 pv e2e4",
            "info depth 3 multipv 2 score cp -50 pv a2a3",
            "info depth 4 multipv 1 score cp 200 lowerbound pv e2e4",
            "info string old score cp 999",
            "info depth 4 string diagnostic score cp 1000",
            "bestmove e2e4",
        ]])
        self.assertEqual(engine.score(FEN, 4, 0), 80)

    def test_mate_labels_keep_the_winning_and_losing_sign(self):
        for mate, sign in [(0, -1), (500, 1), (-500, -1)]:
            with self.subTest(mate=mate):
                engine, _ = scripted_engine(label_positions_engine, [[
                    f"info score mate {mate}", "bestmove 0000",
                ]])
                self.assertGreater(sign * engine.score(FEN, 1, 0), 0)

    def test_null_bestmove_is_not_exported_as_a_training_move(self):
        engine, _ = scripted_engine(collect_hard_pairs, [["bestmove 0000"]])
        self.assertIsNone(engine.bestmove(FEN, 1))

    def test_static_eval_consumes_a_ready_barrier(self):
        engine, commands = scripted_engine(label_positions_static_eval, [])
        self.assertEqual(engine.static_eval(FEN), 23)
        self.assertEqual(commands[-1], "isready")
        self.assertTrue(engine.lines.empty())

    def test_teacher_keeps_the_deeper_score_when_a_move_changes_slots(self):
        engine, _ = scripted_engine(label_root_children_teacher, [[
            "info depth 2 multipv 1 score cp 100 pv e2e4",
            "info depth 2 multipv 2 score cp 90 pv d2d4",
            "info depth 3 multipv 1 score cp 120 pv d2d4",
            "bestmove d2d4",
        ]])
        scores = engine.multipv_scores(FEN, 3, 0)
        self.assertEqual({move.uci(): score for move, score in scores.items()}, {"d2d4": 120})

    def test_teacher_accepts_equal_scores_for_distinct_moves(self):
        engine, _ = scripted_engine(label_root_children_teacher, [[
            "info depth 2 multipv 1 score cp 0 pv e2e4",
            "info depth 2 multipv 2 score cp 0 pv d2d4",
            "bestmove e2e4",
        ]])
        self.assertEqual(len(engine.multipv_scores(FEN, 2, 0)), 2)

    def test_closed_engine_output_fails_without_waiting_for_the_deadline(self):
        engine, _ = scripted_engine(label_positions_engine, [])
        engine.lines.put(None)
        for _ in range(2):
            with self.assertRaisesRegex(RuntimeError, "closed its output"):
                engine.read_line(float("inf"))

    def test_handshake_failure_reaps_the_engine_and_closes_pipes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "closed_engine"
            path.write_text(f"#!{sys.executable}\nimport sys\nsys.exit(0)\n")
            path.chmod(0o700)
            engine = UCIClient.__new__(UCIClient)
            with self.assertRaisesRegex(RuntimeError, "closed its output"):
                engine.__init__(str(path), [])
            self.assertIsNotNone(engine.proc.poll())
            self.assertFalse(engine.reader_thread.is_alive())
            self.assertTrue(engine.proc.stdin.closed)
            self.assertTrue(engine.proc.stdout.closed)

    def test_collectors_can_be_imported_as_modules(self):
        root = str(Path(__file__).resolve().parents[1])
        with patch.object(sys, "path", [root, *sys.path]):
            for name in (
                "label_positions_engine", "label_positions_static_eval",
                "collect_hard_pairs", "label_root_children_teacher",
            ):
                module = importlib.import_module(f"scripts.{name}")
                self.assertTrue(callable(module.UCIEngine))


if __name__ == "__main__":
    unittest.main()
