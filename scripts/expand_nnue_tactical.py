#!/usr/bin/env python3
"""Expand a legacy 768-input NNUE into the tactical-feature architecture.

The existing piece-square rows are copied exactly. Newly added tactical rows are
zero-initialized, producing a headered network that behaves like the source net
until the new rows are trained.
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


MAGIC = b"RNQNNUE\0"
VERSION = 1
BASE_INPUT_SIZE = 64 * 6 * 2
HIDDEN_SIZE = 256
NON_KING_FEATURES = 2 * 5 * 64
HANGING_OFFSET = BASE_INPUT_SIZE
PAWN_ATTACKED_OFFSET = HANGING_OFFSET + NON_KING_FEATURES
IN_CHECK_OFFSET = PAWN_ATTACKED_OFFSET + NON_KING_FEATURES
KING_PRESSURE_OFFSET = IN_CHECK_OFFSET + 2
PINNED_OFFSET = KING_PRESSURE_OFFSET + 2 * 4 * 8
MOBILITY_OFFSET = PINNED_OFFSET + NON_KING_FEATURES
TACTICAL_INPUT_SIZE = MOBILITY_OFFSET + 2 * 4 * 8


def payload_size(input_size: int) -> int:
    return input_size * HIDDEN_SIZE * 2 + HIDDEN_SIZE * 2 * 3 + 2


def read_payload(path: Path) -> tuple[bytes, int]:
    data = path.read_bytes()
    if data.startswith(MAGIC):
        if len(data) < len(MAGIC) + 12:
            raise ValueError("truncated NNUE header")
        version, input_size, hidden_size = struct.unpack_from("<III", data, len(MAGIC))
        if version != VERSION:
            raise ValueError(f"unsupported NNUE version {version}")
        if hidden_size != HIDDEN_SIZE:
            raise ValueError(f"expected hidden size {HIDDEN_SIZE}, got {hidden_size}")
        payload = data[len(MAGIC) + 12 :]
        if len(payload) != payload_size(input_size):
            raise ValueError("headered NNUE payload size does not match header")
        return payload, input_size

    if len(data) != payload_size(BASE_INPUT_SIZE):
        raise ValueError("raw NNUE size does not match legacy 768-input architecture")
    return data, BASE_INPUT_SIZE


def expand_payload(payload: bytes, input_size: int) -> bytes:
    if input_size == TACTICAL_INPUT_SIZE:
        return payload
    if input_size > TACTICAL_INPUT_SIZE:
        raise ValueError(f"cannot shrink unexpected input size {input_size}")

    row_bytes = HIDDEN_SIZE * 2
    base_weight_bytes = input_size * row_bytes
    copied_weights = payload[:base_weight_bytes]
    tail = payload[base_weight_bytes:]
    added_rows = TACTICAL_INPUT_SIZE - BASE_INPUT_SIZE
    added_rows = TACTICAL_INPUT_SIZE - input_size
    return copied_weights + bytes(added_rows * row_bytes) + tail


def write_headered(path: Path, payload: bytes) -> None:
    header = MAGIC + struct.pack("<III", VERSION, TACTICAL_INPUT_SIZE, HIDDEN_SIZE)
    path.write_bytes(header + payload)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path, help="Legacy 768-input .nnue")
    parser.add_argument("output", type=Path, help="Headered tactical .nnue to write")
    args = parser.parse_args()

    payload, input_size = read_payload(args.input)
    expanded = expand_payload(payload, input_size)
    write_headered(args.output, expanded)
    print(
        f"expanded {args.input} ({input_size} inputs) -> "
        f"{args.output} ({TACTICAL_INPUT_SIZE} inputs)"
    )


if __name__ == "__main__":
    main()
