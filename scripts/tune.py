#!/usr/bin/env python3
"""Texel-style (simplified) tuning harness for EvalParams.

Improvements vs previous version:
 - Global multivariate linear regression (normal equations) over all parameters instead of per-parameter isolation.
 - L2 regularization (ridge) to handle multicollinearity (e.g., identical rook file feature for open & semi-open weights).
 - Optional integer rounding & clamping.
 - Stable Gaussian elimination with partial pivoting implemented without external deps.
 - Outputs to tuned_eval_params.rs by default.

Workflow:
 1. Generate labeled CSV (must include stockfish_cp). Example:
     ./target/debug/chess_engine selfplay-labeled-export 120 10 labeled.csv
 2. Run tuner:
     python scripts/tune.py --input labeled.csv --l2 0.25 --round
 3. Rebuild & inspect:
     cargo build && ./target/debug/chess_engine show-tuned-params

NOTE: The mapping here treats each parameter as a single scalar applied to combined side-differential counts for its associated raw features.
Future extensions could:
  - Separate midgame/endgame weights
  - Tune material / passed_per_rank
  - Add logistic (Texel) winprob fitting if result labels available.
"""
from __future__ import annotations
import argparse, csv, math, statistics, sys, pathlib, json
from dataclasses import dataclass
from typing import List, Dict, Tuple

# Map between feature columns and EvalParams fields.
# Some EvalParams entries (material values) are not tuned here; placeholder for future.
PARAM_FEATURE_MAP = {
    'mobility_knight': ['mob_kn_w','mob_kn_b_diff'],
    'mobility_bishop': ['mob_bi_w','mob_bi_b_diff'],
    'mobility_rook':   ['mob_r_w','mob_r_b_diff'],
    'mobility_queen':  ['mob_q_w','mob_q_b_diff'],
    'bishop_pair':     ['bishop_pair_feat'],
    'tempo':           ['tempo_feat'],
    'isolated_pawn':   ['isolated_w','isolated_b_diff'],
    'doubled_pawn':    ['doubled_w','doubled_b_diff'],
    'backward_pawn':   ['backward_w','backward_b_diff'],
    'passed_base':     ['passed_w','passed_b_diff'],
    'rook_open_file':  ['rook_files_feat'],
    'rook_semi_open':  ['rook_files_feat'],  # identical column; ridge regularization mitigates singularity
    'hanging_penalty': ['hanging_feat'],
    'king_ring_attack':['kingring_attacks_w','kingring_attacks_b_diff']
}

# Default starting values (should match Rust EvalParams::default)
DEFAULT_PARAMS = {
    'mobility_knight':4,'mobility_bishop':5,'mobility_rook':3,'mobility_queen':2,
    'bishop_pair':30,'tempo':8,'isolated_pawn':15,'doubled_pawn':12,'backward_pawn':10,
    'passed_base':12,'passed_per_rank':8,'rook_open_file':15,'rook_semi_open':7,
    'hanging_penalty':20,'king_ring_attack':4
}

# Per-parameter reasonable clamps (domain knowledge) to keep regression stable
DEFAULT_CLAMPS = {
    'mobility_knight': (0, 15),
    'mobility_bishop': (0, 15),
    'mobility_rook':   (0, 10),
    'mobility_queen':  (0, 6),
    'bishop_pair':     (0, 80),
    'tempo':           (0, 20),
    'isolated_pawn':   (0, 40),
    'doubled_pawn':    (0, 40),
    'backward_pawn':   (0, 40),
    'passed_base':     (0, 120),
    'rook_open_file':  (0, 40),
    'rook_semi_open':  (0, 30),
    'hanging_penalty': (0, 60),
    'king_ring_attack':(0, 24),
}

@dataclass
class Sample:
    features: Dict[str,float]
    target: float

def parse_csv(path: pathlib.Path) -> List[Sample]:
    samples = []
    with path.open() as f:
        reader = csv.DictReader(f)
        for row in reader:
            if not row.get('stockfish_cp'):  # skip unlabeled
                continue
            try:
                target = float(row['stockfish_cp'])
            except ValueError:
                continue
            # Build differential features (white minus black) where needed
            def get_int(col):
                try: return float(row[col])
                except: return 0.0
            feats = {
                'mob_kn_w': get_int('mob_kn_w'),
                'mob_kn_b_diff': -get_int('mob_kn_b'),
                'mob_bi_w': get_int('mob_bi_w'),
                'mob_bi_b_diff': -get_int('mob_bi_b'),
                'mob_r_w': get_int('mob_r_w'),
                'mob_r_b_diff': -get_int('mob_r_b'),
                'mob_q_w': get_int('mob_q_w'),
                'mob_q_b_diff': -get_int('mob_q_b'),
                'bishop_pair_feat': 1.0 if (get_int('bishop_pair')!=0) else 0.0,  # placeholder
                'tempo_feat': 1.0 if get_int('tempo')>0 else -1.0,
                'isolated_w': get_int('isolated_w'),
                'isolated_b_diff': -get_int('isolated_b'),
                'doubled_w': get_int('doubled_w'),
                'doubled_b_diff': -get_int('doubled_b'),
                'backward_w': get_int('backward_w'),
                'backward_b_diff': -get_int('backward_b'),
                'passed_w': get_int('passed_w'),
                'passed_b_diff': -get_int('passed_b'),
                'rook_files_feat': 1.0 if get_int('rook_files')!=0 else 0.0,
                'hanging_feat': 1.0 if get_int('hanging')!=0 else 0.0,
                'kingring_attacks_w': get_int('kingring_attacks_w'),
                'kingring_attacks_b_diff': -get_int('kingring_attacks_b'),
            }
            samples.append(Sample(feats, target))
    return samples

def build_design_matrix(samples: List[Sample], params: List[str], intercept: bool):
    X = []
    y = []
    for s in samples:
        row = []
        if intercept:
            row.append(1.0)
        for p in params:
            feats = PARAM_FEATURE_MAP[p]
            row.append(sum(s.features.get(f,0.0) for f in feats))
        X.append(row)
        y.append(s.target)
    return X, y

def ridge_normal_equations(X, y, l2: float, intercept: bool):
    n = len(X)
    if n == 0:
        return []
    p = len(X[0])
    # Build XtX and XtY
    XtX = [[0.0]*p for _ in range(p)]
    XtY = [0.0]*p
    for row, target in zip(X,y):
        for i in range(p):
            Xi = row[i]
            XtY[i] += Xi * target
            for j in range(i, p):
                XtX[i][j] += Xi * row[j]
    # Symmetrize & regularize
    for i in range(p):
        for j in range(i):
            XtX[i][j] = XtX[j][i]
        if not (intercept and i == 0):  # don't regularize intercept
            XtX[i][i] += l2
    # Solve XtX w = XtY via Gaussian elimination with partial pivot
    # Augment
    for i in range(p):
        XtX[i].append(XtY[i])
    # Elimination
    for col in range(p):
        # Pivot
        pivot = max(range(col, p), key=lambda r: abs(XtX[r][col]))
        if abs(XtX[pivot][col]) < 1e-12:
            continue
        if pivot != col:
            XtX[col], XtX[pivot] = XtX[pivot], XtX[col]
        # Normalize pivot row
        pivval = XtX[col][col]
        inv = 1.0 / pivval
        for k in range(col, p+1):
            XtX[col][k] *= inv
        # Eliminate below
        for r in range(col+1, p):
            factor = XtX[r][col]
            if factor == 0: continue
            for k in range(col, p+1):
                XtX[r][k] -= factor * XtX[col][k]
    # Back substitution
    w = [0.0]*p
    for i in reversed(range(p)):
        acc = XtX[i][p]
        for j in range(i+1,p):
            acc -= XtX[i][j]*w[j]
        w[i] = acc / (XtX[i][i] if abs(XtX[i][i])>1e-12 else 1.0)
    return w

def multivariate_fit(samples: List[Sample], l2: float, use_intercept: bool):
    params = list(PARAM_FEATURE_MAP.keys())
    X, y = build_design_matrix(samples, params, use_intercept)
    w = ridge_normal_equations(X, y, l2, use_intercept)
    preds = [sum(a*b for a,b in zip(row, w)) for row in X]
    if len(y) > 1:
        mse = sum((py-ty)**2 for py,ty in zip(preds,y))/len(y)
        rmse = mse**0.5
        my = sum(y)/len(y)
        mp = sum(preds)/len(preds)
        num = sum((py-mp)*(ty-my) for py,ty in zip(preds,y))
        den = (sum((py-mp)**2 for py in preds)*sum((ty-my)**2 for ty in y))**0.5
        corr = num/den if den else 0.0
    else:
        rmse = corr = 0.0
    offset = 1 if use_intercept else 0
    weights = {p: w[i+offset] for i,p in enumerate(params)}
    return weights, rmse, corr

RUST_TEMPLATE = """// Auto-generated by scripts/tune.py (multivariate ridge fit)
// Do not edit manually.
use crate::board::EvalParams;

pub fn tuned_params() -> EvalParams {{ EvalParams {{
    material: [100,325,330,500,950,0],
    mobility_knight: {mobility_knight},
    mobility_bishop: {mobility_bishop},
    mobility_rook: {mobility_rook},
    mobility_queen: {mobility_queen},
    bishop_pair: {bishop_pair},
    tempo: {tempo},
    isolated_pawn: {isolated_pawn},
    doubled_pawn: {doubled_pawn},
    backward_pawn: {backward_pawn},
    passed_base: {passed_base},
    passed_per_rank: 8, // unchanged
    rook_open_file: {rook_open_file},
    rook_semi_open: {rook_semi_open},
    hanging_penalty: {hanging_penalty},
    king_ring_attack: {king_ring_attack},
    endgame_threshold: 2000,
}} }}
"""

def write_rust(params: Dict[str,float], path: pathlib.Path):
    merged = DEFAULT_PARAMS.copy()
    merged.update(params)
    # Ensure all values are integers for now (final rounding already happened if requested)
    fmt_ready = {k: int(round(v)) for k,v in merged.items() if isinstance(v,(int,float))}
    with path.open('w') as f:
        f.write(RUST_TEMPLATE.format(**fmt_ready))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--input', required=True)
    ap.add_argument('--output', default='src/tuned_eval_params.rs')
    ap.add_argument('--min-samples', type=int, default=100)
    ap.add_argument('--l2', type=float, default=0.10, help='L2 regularization lambda')
    ap.add_argument('--round', action='store_true', help='Round weights to nearest integer')
    ap.add_argument('--clamp', type=float, nargs=2, metavar=('MIN','MAX'), default=None, help='Clamp weights to range')
    ap.add_argument('--default-clamps', action='store_true', help='Apply built-in sane per-parameter clamps')
    ap.add_argument('--intercept', action='store_true', help='Include intercept term')
    args = ap.parse_args()
    samples = parse_csv(pathlib.Path(args.input))
    if len(samples) < args.min_samples:
        print(f"Not enough labeled samples: {len(samples)} < {args.min_samples}", file=sys.stderr)
        sys.exit(1)
    tuned, rmse, corr = multivariate_fit(samples, args.l2, args.intercept)
    # Post-process: clamp & round
    if args.clamp:
        mn,mx = args.clamp
        for k in tuned:
            tuned[k] = max(mn, min(mx, tuned[k]))
    if args.default_clamps:
        for k,(mn,mx) in DEFAULT_CLAMPS.items():
            if k in tuned:
                tuned[k] = max(mn, min(mx, tuned[k]))
    if args.round:
        for k in tuned:
            tuned[k] = round(tuned[k])
    write_rust(tuned, pathlib.Path(args.output))
    print(f"Wrote tuned params to {args.output} (l2={args.l2}, samples={len(samples)}, rmse={rmse:.2f}, corr={corr:.3f})")

if __name__ == '__main__':
    main()
