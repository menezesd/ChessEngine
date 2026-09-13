"""Training/export contract tests; run with python3 -m unittest discover -s tests -p 'test_nnue_training.py'."""

import importlib.util
from contextlib import nullcontext
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
import torch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import train_nnue_ranking


def load_trainer(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


TRAINERS = [
    load_trainer("trainer_base", "scripts/train_nnue_256.py"),
    load_trainer("trainer_tactical", "scripts/train_nnue_improved.py"),
    load_trainer("package_base", "nnue_training_package/train_nnue_256.py"),
    load_trainer("package_improved", "nnue_training_package/train_nnue_improved.py"),
]
EXPANDER = load_trainer("nnue_expander", "scripts/expand_nnue_tactical.py")


class BiasModel(torch.nn.Module):
    def __init__(self, value=0.0, **_kwargs):
        super().__init__()
        self.bias = torch.nn.Parameter(torch.tensor(value))
        self.exports = []

    def forward(self, white, _black, _stm):
        return self.bias.expand(len(white), 1)

    def export_quantized(self, _path):
        self.exports.append(self.bias.item())


def training_batch(targets):
    count = len(targets)
    return (
        torch.zeros(count, 1), torch.zeros(count, 1), torch.ones(count, dtype=torch.bool),
        torch.tensor(targets).reshape(-1, 1), torch.full((count, 1), 0.5),
    )


def run_training_cli(trainer, directory, extra_args=()):
    path = Path(directory) / "positions.txt"
    path.write_text(
        "7k/8/8/8/8/8/3Q4/K7 w - - 0 1 | 400 | 1.0\n"
        "7k/8/8/8/8/8/4Q3/K7 w - - 0 1 | 400 | 1.0\n"
    )
    model = BiasModel()
    argv = ["train", "--data", str(path), "--output", str(Path(directory) / "out.nnue"),
            "--epochs", "2", "--batch-size", "8"]
    if hasattr(trainer, "ImprovedChessDataset"):
        argv += ["--val-split", "0.5"]
    argv += list(extra_args)
    real_loader = torch.utils.data.DataLoader
    datasets = []

    def loader(dataset, **kwargs):
        datasets.append(dataset)
        kwargs.update(num_workers=0, pin_memory=False)
        return real_loader(dataset, **kwargs)

    with patch.object(sys, "argv", argv), \
         patch.object(trainer, "NNUE256", return_value=model), \
         patch.object(trainer, "get_device", return_value=torch.device("cpu")), \
         patch.object(trainer, "DataLoader", loader), \
         patch.object(torch, "save"), \
         patch("builtins.print"):
        trainer.main()
    return model, datasets


class PassthroughScaler:
    def scale(self, loss):
        return loss

    def unscale_(self, _optimizer):
        pass

    def step(self, optimizer):
        optimizer.step()

    def update(self):
        pass


def toy_model(trainer):
    model = trainer.NNUE256().eval()
    with torch.no_grad():
        for parameter in model.parameters():
            parameter.zero_()
        model.feature_weights[0, 0] = 1.0
        model.output_weights_white[0] = 1.0
        model.output_weights_black[0] = -1.0
        model.output_bias[0] = 0.25
    return model


def exported_scores(path, trainer):
    """Evaluate one active white feature using the Rust file-format contract."""
    data = path.read_bytes()
    if data.startswith(b"RNQNNUE\0"):
        _, inputs, hidden = struct.unpack_from("<III", data, 8)
        data = data[20:]
    else:
        inputs, hidden = trainer.INPUT_SIZE, trainer.HIDDEN_SIZE
    values = np.frombuffer(data, dtype="<i2").astype(np.int64)
    count = inputs * hidden
    feature_weights = values[:count].reshape(inputs, hidden)
    bias, us_weights, them_weights = values[count:-1].reshape(3, hidden)
    white = np.clip(feature_weights[0] + bias, 0, trainer.QA) ** 2
    black = np.clip(bias, 0, trainer.QA) ** 2
    result = []
    for us, them in [(white, black), (black, white)]:
        output = us @ us_weights + them @ them_weights + values[-1] * trainer.QA
        result.append(int(output * trainer.SCALE / (trainer.QA**2 * trainer.QB)))
    return result


class TrainingContractTests(unittest.TestCase):
    def test_base_training_metric_weights_samples_in_uneven_batches(self):
        for trainer in (TRAINERS[0], TRAINERS[2]):
            with self.subTest(trainer=trainer.__name__):
                model = BiasModel()
                optimizer = torch.optim.SGD(model.parameters(), lr=0.0)
                whole = trainer.train_epoch(model, [training_batch([0.0, 0.1, 2.0])], optimizer, "cpu")
                split = trainer.train_epoch(
                    model, [training_batch([0.0, 0.1]), training_batch([2.0])], optimizer, "cpu",
                )
                self.assertAlmostEqual(whole, split, places=7)
                with self.assertRaises(ValueError):
                    trainer.train_epoch(model, [], optimizer, "cpu")

    def test_ranking_metric_weights_samples_in_uneven_batches(self):
        def batch(weights):
            features = training_batch([0.0] * len(weights))[:3]
            return (*features, *features, torch.tensor(weights))
        model = BiasModel()
        optimizer = torch.optim.SGD(model.parameters(), lr=0.0)
        whole = train_nnue_ranking.train_epoch(model, [batch([1.0, 1.0, 5.0])], optimizer, "cpu", 80)
        split = train_nnue_ranking.train_epoch(
            model, [batch([1.0, 1.0]), batch([5.0])], optimizer, "cpu", 80,
        )
        self.assertAlmostEqual(whole, split, places=7)
        with self.assertRaises(ValueError):
            train_nnue_ranking.train_epoch(model, [], optimizer, "cpu", 80)

    def test_ranking_rejects_nonfinite_and_negative_sample_weights(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pairs.txt"
            fen = "7k/8/8/8/8/8/3Q4/K7 b - - 0 1"
            path.write_text("".join(f"{fen} | {fen} | {weight}\n" for weight in [1, -1, "nan", "inf"]))
            pairs = train_nnue_ranking.load_pair_file(str(path), repeat=1)
            self.assertEqual([pair.weight for pair in pairs], [1.0])

    def test_accumulation_trains_on_short_and_uneven_final_groups(self):
        for trainer in (TRAINERS[1], TRAINERS[3]):
            for accumulation_steps in (2, 4):
                with self.subTest(trainer=trainer.__name__, steps=accumulation_steps):
                    model = BiasModel()
                    trainer.train_epoch(
                        model, [training_batch([0.1, 0.2]), training_batch([0.3])],
                        torch.optim.SGD(model.parameters(), lr=0.1), torch.device("cpu"),
                        accumulation_steps=accumulation_steps, wdl_lambda=0.0, use_huber=False,
                    )
                    self.assertAlmostEqual(model.bias.item(), 0.04, places=6)

    def test_mixed_precision_path_includes_anchor_regularization(self):
        trainer = TRAINERS[1]
        model = BiasModel(0.5)
        with patch.object(torch.cuda.amp, "autocast", nullcontext):
            trainer.train_epoch(
                model, [training_batch([0.5])],
                torch.optim.SGD(model.parameters(), lr=0.1), torch.device("cpu"),
                wdl_lambda=0.0, scaler=PassthroughScaler(),
                anchors={"bias": torch.tensor(0.0)}, anchor_lambda=1.0,
            )
        self.assertAlmostEqual(model.bias.item(), 0.4, places=6)

    def test_validation_loss_is_independent_of_batch_partition(self):
        for trainer in (TRAINERS[1], TRAINERS[3]):
            with self.subTest(trainer=trainer.__name__):
                model = BiasModel()
                kwargs = {"wdl_lambda": 0.0, "use_huber": False}
                whole = trainer.validate(model, [training_batch([0.0, 0.1, 0.2])], "cpu", **kwargs)
                split = trainer.validate(
                    model, [training_batch([0.0, 0.1]), training_batch([0.2])], "cpu", **kwargs,
                )
                self.assertAlmostEqual(whole, split, places=7)

    def test_training_cli_keeps_datasets_smaller_than_one_batch(self):
        with tempfile.TemporaryDirectory() as directory:
            for trainer in TRAINERS:
                with self.subTest(trainer=trainer.__name__):
                    model, _ = run_training_cli(trainer, directory)
                    self.assertGreater(model.bias.item(), 0.0)

    def test_augmented_position_stays_in_the_same_split_as_its_original(self):
        with tempfile.TemporaryDirectory() as directory:
            for trainer in (TRAINERS[1], TRAINERS[3]):
                with self.subTest(trainer=trainer.__name__):
                    _, (train, validation) = run_training_cli(trainer, directory)
                    original_count = len(train.dataset.positions)
                    train_positions = {i % original_count for i in train.indices}
                    validation_positions = {i % original_count for i in validation.indices}
                    self.assertFalse(train_positions & validation_positions)
                    self.assertEqual(train_positions | validation_positions, set(range(original_count)))

    def test_disabled_validation_selects_by_training_loss(self):
        with tempfile.TemporaryDirectory() as directory:
            for trainer in (TRAINERS[1], TRAINERS[3]):
                with self.subTest(trainer=trainer.__name__):
                    model, _ = run_training_cli(trainer, directory, ["--val-split", "0"])
                    self.assertEqual(model.exports[-1], model.bias.item())

    def test_missing_requested_checkpoint_is_not_silently_ignored(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = str(Path(directory) / "missing.pt")
            for trainer in TRAINERS:
                with self.subTest(trainer=trainer.__name__):
                    with self.assertRaises(FileNotFoundError):
                        run_training_cli(trainer, directory, ["--checkpoint", missing])

    def test_nonfinite_labels_are_rejected_before_training(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "positions.txt"
            path.write_text("".join(
                f"7k/8/8/8/8/8/3Q4/K7 w - - 0 1 | {score} | 1.0\n"
                for score in [400, "nan", "inf", "-inf"]
            ))
            for trainer in TRAINERS:
                with self.subTest(trainer=trainer.__name__), patch("builtins.print"):
                    if hasattr(trainer, "ImprovedChessDataset"):
                        dataset = trainer.ImprovedChessDataset([str(path)], augment=False)
                    else:
                        dataset = trainer.ChessDataset([str(path)])
                    self.assertEqual(len(dataset), 1)

    def test_nonfinite_network_export_preserves_the_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "existing.nnue"
            for trainer in TRAINERS:
                for field in ("feature_weights", "output_bias"):
                    with self.subTest(trainer=trainer.__name__, field=field):
                        model = toy_model(trainer)
                        with torch.no_grad():
                            getattr(model, field).flatten()[0] = float("nan")
                        path.write_bytes(b"previous valid network")
                        with self.assertRaises(ValueError):
                            model.export_quantized(str(path))
                        self.assertEqual(path.read_bytes(), b"previous valid network")

    def test_packaged_trainers_match_the_engine_hidden_size(self):
        for trainer in TRAINERS:
            with self.subTest(trainer=trainer.__name__):
                self.assertEqual(trainer.HIDDEN_SIZE, 256)

    def test_pst_initialization_scores_material_for_the_side_to_move(self):
        for trainer in TRAINERS:
            with self.subTest(trainer=trainer.__name__), patch("builtins.print"):
                model = trainer.NNUE256().eval()
                model.init_from_pst()
                for white_to_move in (True, False):
                    side = "w" if white_to_move else "b"
                    fen = f"7k/8/8/8/8/8/3Q4/K7 {side} - - 0 1"
                    white, black, stm = trainer.parse_fen_features(fen)
                    white_tensor = torch.zeros(1, trainer.INPUT_SIZE)
                    black_tensor = torch.zeros_like(white_tensor)
                    white_tensor[0, white] = 1
                    black_tensor[0, black] = 1
                    score = model(white_tensor, black_tensor, torch.tensor([stm])).item()
                    self.assertGreater(score * (1 if white_to_move else -1), 0)

    def test_side_to_move_selects_accumulators_without_swapping_output_roles(self):
        for trainer in TRAINERS:
            with self.subTest(trainer=trainer.__name__):
                model = toy_model(trainer)
                white = torch.zeros((2, trainer.INPUT_SIZE))
                white[:, 0] = 1
                black = torch.zeros_like(white)
                with torch.no_grad():
                    output = model(white, black, torch.tensor([True, False])).flatten()
                torch.testing.assert_close(output, torch.tensor([1.25, -0.75]))

    def test_export_preserves_forward_scores_for_both_turns(self):
        with tempfile.TemporaryDirectory() as directory:
            for trainer in TRAINERS:
                with self.subTest(trainer=trainer.__name__):
                    path = Path(directory) / f"{trainer.__name__}.nnue"
                    model = toy_model(trainer)
                    model.export_quantized(str(path))
                    self.assertEqual(exported_scores(path, trainer), [500, -300])

    def test_base_datasets_convert_white_labels_to_the_side_to_move(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "positions.txt"
            path.write_text("7k/8/8/8/8/8/3Q4/K7 b - - 0 1 | 400 | 1.0\n")
            for trainer in [TRAINERS[0], TRAINERS[2]]:
                with self.subTest(trainer=trainer.__name__):
                    dataset = trainer.ChessDataset([str(path)])
                    _, _, stm, target, result = dataset[0]
                    self.assertFalse(stm.item())
                    self.assertEqual(target.item(), -1.0)
                    self.assertEqual(result.item(), 0.0)

    def test_quantized_loaders_preserve_the_runtime_bias_scale(self):
        tactical = TRAINERS[1]
        with tempfile.TemporaryDirectory() as directory:
            base_path = Path(directory) / "base.nnue"
            toy_model(TRAINERS[0]).export_quantized(str(base_path))
            expanded = tactical.NNUE256().eval()
            expanded.load_piece_square_quantized(str(base_path))
            self.assertEqual(expanded.output_bias.item(), 0.25)

            tactical_path = Path(directory) / "tactical.nnue"
            expanded.export_quantized(str(tactical_path))
            loaded = tactical.NNUE256().eval()
            loaded.load_quantized(str(tactical_path))
            self.assertEqual(loaded.output_bias.item(), 0.25)
            self.assertEqual(exported_scores(tactical_path, tactical), [500, -300])

    def test_expansion_preserves_scores_and_rejects_unsupported_layouts(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "weights.nnue"
            toy_model(TRAINERS[0]).export_quantized(str(path))
            payload, inputs = EXPANDER.read_payload(path)
            expanded = EXPANDER.expand_payload(payload, inputs)
            EXPANDER.write_headered(path, expanded)
            self.assertEqual(exported_scores(path, TRAINERS[1]), [500, -300])
            with self.assertRaises(ValueError):
                EXPANDER.expand_payload(payload[:-2], inputs)
            with self.assertRaises(ValueError):
                EXPANDER.expand_payload(payload, inputs + 1)
            data = path.read_bytes()
            data = data[:12] + struct.pack("<I", inputs + 1) + data[16:]
            path.write_bytes(data)
            with self.assertRaises(ValueError):
                EXPANDER.read_payload(path)

    def test_malformed_quantized_files_do_not_partially_replace_model_weights(self):
        tactical = TRAINERS[1]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "weights.nnue"
            for loader, source in [
                ("load_quantized", tactical),
                ("load_piece_square_quantized", TRAINERS[0]),
            ]:
                toy_model(source).export_quantized(str(path))
                valid = path.read_bytes()
                for name, data in [
                    ("truncated payload", valid[:-2]),
                    ("trailing bytes", valid + b"xx"),
                    ("truncated header", b"RNQNNUE\0"),
                ]:
                    with self.subTest(loader=loader, corruption=name):
                        path.write_bytes(data)
                        model = tactical.NNUE256().eval()
                        before = {key: value.clone() for key, value in model.state_dict().items()}
                        with self.assertRaises(ValueError):
                            getattr(model, loader)(str(path))
                        for key, value in model.state_dict().items():
                            torch.testing.assert_close(value, before[key], rtol=0, atol=0)


if __name__ == "__main__":
    unittest.main()
