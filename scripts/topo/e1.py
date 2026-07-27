"""E1 — retrieval merge-dominance vs baselines (plan §4 E1, amended per FINDINGS-0).

Question: does the H0 merge-dominance ratio (p2/p1 over a recall candidate
set's two longest bars) flag multi-topic/ambiguous queries better than cheap
baselines (k=2 silhouette, mean pairwise distance)?

Protocol:
- 14 REAL queries mined from Claude Code transcripts (every distinct recall
  this machine has ever issued) — ecological validity, judged qualitatively.
- 30 SYNTHETIC control queries with ground-truth labels by construction:
  15 tight (one topic) + 15 ambiguous (two deliberately distant store topics
  jammed together). These carry the quantitative gate (AUC per metric).
- Every query runs through the REAL Rust recall pipeline (`cerebro recall
  --json -n 50`) against a fresh scratch copy of the pin (never the live DB —
  recall stamps access/FSRS state).
- Per candidate set: merge-dominance p2/p1, components at largest gap,
  silhouette (k=2 KMeans labels, angular-distance score), mean pairwise
  distance, and a bootstrap z-score of dominance vs 200 random size-matched
  subsets of the store (FINDINGS-0: absolute numbers are meaningless under
  concentration — everything relative).

Run: .venv/bin/python e1.py   (expects the scratch DB staged; ~3 min)
"""

from __future__ import annotations

import json
import os
import subprocess

import numpy as np
from sklearn.cluster import KMeans
from sklearn.metrics import roc_auc_score, silhouette_score

from h0 import dominance, h0_deaths_from_dist, top_gap
from loader import angular_distance_matrix, embedding_matrix, load_store

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CLI = os.path.join(REPO, "target", "release", "cerebro")
SCRATCH = (
    "/tmp/claude-1000/-home-andre-Projects-CerebroCortex-RS/"
    "49634682-73b3-4ccc-9011-c7ce55be6aa4/scratchpad/topo-recall"
)
OUT = os.path.join(os.path.dirname(__file__), "out", "e1")
RNG = np.random.default_rng(7)
N_BOOT = 200

REAL_QUERIES = [
    "CerebroCortex-RS build status step progress",
    "what was the plan for deploying cerebro onto the raspberry pi with a systemd service",
    "CerebroCortex-RS backport waves status progress",
    "ingestion reversible provenance",
    "Occipital-RS build status step progress",
    "CerebroCortex-RS forward port ApexOS ingest orphan reap",
    "ApexOS-RS wire view backport occipital status",
    "Occipital-RS web cortex phases status curation knowledge hub ingest embed sqlite-vec",
    "ApexOS-RS cerebro backport docs refactor adaptive UI",
    "ApexOS-RS build status",
    "ApexOS-RS next backlog item hotspot Occipital build status",
    "A1 namespace jail CLONE_NEWNET tools worker conclusion adds little http_fetch needs network deprioritized",
    "VIMANA teaser trailer video outro programmatic ffmpeg timelapse pre-pivot draft",
    "VIMANA P2 CLOSE-OUT TEASER EXISTS 60e8de5",
]

TIGHT = [
    "slint ui face rendering shader expression",
    "occipital web browsing politeness robots crawl delay",
    "cerebro memory recall spreading activation ranking",
    "pi 5 deploy systemd service binary swap",
    "dream consolidation phases schema pattern extraction",
    "mesh network hart protocol node discovery",
    "clip vision image embedding caption search",
    "fts5 sqlite full text search fallback",
    "agentd websocket plugin supervisor restart",
    "pdf markdown ingestion chunking sections tags",
    "fastembed bge model download cache onnx",
    "audit log retention sweep mutations",
    "procedure fitness wilson score competition champion",
    "hermes qwen local agent harness loop",
    "vimana teaser video ffmpeg render outro",
]

AMBIGUOUS = [
    "slint face shader blush and pdf ingestion chunking frontmatter",
    "vimana teaser ffmpeg outro and sqlite-vec extension load path",
    "mesh hart protocol discovery and dream emotional reprocessing valence",
    "pi systemd binary swap and clip vision caption embedding",
    "occipital robots crawl politeness and fsrs stability difficulty decay",
    "notes app slint editor and wilson lower bound procedure fitness",
    "grok tui websocket connection and markdown frontmatter slug tags",
    "sketchpad canvas strokes tiny-skia and audit log retention caps",
    "agentd websocket reconnect supervisor and bge embedding unit norms",
    "cage kiosk boot wayland and rem recombination random pairs",
    "calculator app ordinal mapping and python orphan table reap",
    "sensor head camera tokens and backport queue wire view",
    "face blink saccades catchlight and fts5 implicit AND many terms",
    "self evolution subsystem phases and vimana video outro timelapse",
    "reqwest cookie jar sessions and salience prune floor decay",
]


def run_recall(query: str, path: str) -> list[str]:
    if not os.path.exists(path):
        res = subprocess.run(
            [CLI, "recall", query, "-n", "50", "--json"],
            capture_output=True, text=True, cwd=REPO,
            env={**os.environ, "CEREBRO_DATA_DIR": SCRATCH},
            timeout=120, check=True,
        )
        with open(path, "w") as f:
            f.write(res.stdout)
    return [r["memory"]["id"] for r in json.load(open(path))]


def set_metrics(rows: list[int], D: np.ndarray, E: np.ndarray, boot_pool: np.ndarray):
    Dq = D[np.ix_(rows, rows)]
    deaths = h0_deaths_from_dist(Dq)
    p21 = dominance(deaths)
    _, comps = top_gap(deaths)
    labels = KMeans(n_clusters=2, n_init=10, random_state=0).fit(E[rows]).labels_
    sil = float(silhouette_score(Dq, labels, metric="precomputed")) if len(set(labels)) > 1 else 0.0
    mean_d = float(Dq[np.triu_indices(len(rows), k=1)].mean())
    # Bootstrap null: dominance of random size-matched subsets of the store.
    null = np.empty(N_BOOT)
    for b in range(N_BOOT):
        sample = RNG.choice(boot_pool, size=len(rows), replace=False)
        Db = D[np.ix_(sample, sample)]
        null[b] = dominance(h0_deaths_from_dist(Db))
    z = float((p21 - null.mean()) / max(null.std(), 1e-9))
    return {
        "n": len(rows),
        "dominance_p2_over_p1": float(p21),
        "components_at_gap": comps,
        "silhouette_k2": sil,
        "mean_pairwise": mean_d,
        "dominance_z_vs_random": z,
    }


def main():
    os.makedirs(OUT, exist_ok=True)
    memories, _ = load_store()
    E, ids = embedding_matrix(memories)
    idx = {m: i for i, m in enumerate(ids)}
    D = angular_distance_matrix(E)
    boot_pool = np.arange(len(ids))

    groups = [("real", REAL_QUERIES, None), ("tight", TIGHT, 0), ("ambiguous", AMBIGUOUS, 1)]
    results = []
    for group, queries, label in groups:
        for qi, q in enumerate(queries):
            path = os.path.join(OUT, f"{group}_{qi:02d}.json")
            hit_ids = run_recall(q, path)
            rows = [idx[h] for h in hit_ids if h in idx]
            m = set_metrics(rows, D, E, boot_pool)
            results.append({"group": group, "label": label, "query": q, **m})
            print(f"[{group}:{qi:02d}] p2/p1={m['dominance_p2_over_p1']:.3f} "
                  f"z={m['dominance_z_vs_random']:+.2f} sil={m['silhouette_k2']:.3f} "
                  f"comps@gap={m['components_at_gap']:2d}  {q[:60]}")

    # AUC on the labeled controls: score orientation = "more split/ambiguous".
    ctrl = [r for r in results if r["label"] is not None]
    y = np.array([r["label"] for r in ctrl])
    aucs = {
        "dominance (1 - p2/p1)": roc_auc_score(y, [1 - r["dominance_p2_over_p1"] for r in ctrl]),
        "dominance z (negated)": roc_auc_score(y, [-r["dominance_z_vs_random"] for r in ctrl]),
        "silhouette_k2": roc_auc_score(y, [r["silhouette_k2"] for r in ctrl]),
        "mean_pairwise": roc_auc_score(y, [r["mean_pairwise"] for r in ctrl]),
    }
    report = {"n_boot": N_BOOT, "auc_on_controls": aucs, "results": results}
    with open(os.path.join(OUT, "e1_report.json"), "w") as f:
        json.dump(report, f, indent=2)
    print("\nAUC on synthetic controls (ambiguous=positive):")
    for k, v in aucs.items():
        print(f"  {k:24s} {v:.3f}")


if __name__ == "__main__":
    main()
