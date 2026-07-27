"""E1 follow-up: where does the ambiguity signal die — metric or retrieval?

Construct KNOWN two-topic candidate sets directly (25+25 union of two distinct
tight queries' cached result sets, dedup'd), bypassing recall's mixture
handling. If merge-dominance separates these from single tight sets, the
metric works and the ambiguity signal dies in RETRIEVAL (a mixed query's
embedding is a midpoint — it never returns two clusters). If not, the metric
itself is blind under this store's concentration.

Run after e1.py (reads its cached recall JSONs).
"""

from __future__ import annotations

import glob
import json
import os

import numpy as np
from sklearn.metrics import roc_auc_score

from h0 import dominance, h0_deaths_from_dist
from loader import angular_distance_matrix, embedding_matrix, load_store

OUT = os.path.join(os.path.dirname(__file__), "out", "e1")
RNG = np.random.default_rng(11)
N_BOOT = 200


def main():
    memories, _ = load_store()
    E, ids = embedding_matrix(memories)
    idx = {m: i for i, m in enumerate(ids)}
    D = angular_distance_matrix(E)
    pool = np.arange(len(ids))

    tight_sets = []
    for path in sorted(glob.glob(os.path.join(OUT, "tight_*.json"))):
        hit_ids = [r["memory"]["id"] for r in json.load(open(path))]
        tight_sets.append([idx[h] for h in hit_ids if h in idx])

    def dom_z(rows):
        d = dominance(h0_deaths_from_dist(D[np.ix_(rows, rows)]))
        null = np.empty(N_BOOT)
        for b in range(N_BOOT):
            s = RNG.choice(pool, size=len(rows), replace=False)
            null[b] = dominance(h0_deaths_from_dist(D[np.ix_(s, s)]))
        return d, float((d - null.mean()) / max(null.std(), 1e-9))

    singles = []
    for i, rows in enumerate(tight_sets):
        d, z = dom_z(rows[:50])
        singles.append({"set": f"tight_{i:02d}", "dominance": d, "z": z})

    unions = []
    pairs = [(i, j) for i in range(len(tight_sets)) for j in range(i + 1, len(tight_sets))]
    RNG.shuffle(pairs)
    for i, j in pairs[:15]:
        rows = list(dict.fromkeys(tight_sets[i][:25] + tight_sets[j][:25]))
        d, z = dom_z(rows)
        unions.append({"set": f"union_{i:02d}+{j:02d}", "n": len(rows), "dominance": d, "z": z})
        print(f"union {i:02d}+{j:02d}  n={len(rows):2d}  p2/p1={d:.3f}  z={z:+.2f}")

    y = [0] * len(singles) + [1] * len(unions)
    score = [-s["z"] for s in singles] + [-u["z"] for u in unions]
    auc = roc_auc_score(y, score)
    report = {"singles": singles, "unions": unions, "auc_union_vs_single_negz": float(auc)}
    with open(os.path.join(OUT, "e1_union_report.json"), "w") as f:
        json.dump(report, f, indent=2)
    print(f"\nsingle tight sets: mean z {np.mean([s['z'] for s in singles]):+.2f}")
    print(f"constructed unions: mean z {np.mean([u['z'] for u in unions]):+.2f}")
    print(f"AUC (union vs single, by -z): {auc:.3f}")


if __name__ == "__main__":
    main()
