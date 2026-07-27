"""Phase 0 driver — first barcodes of the real store (plan §3: just LOOK).

Inputs: the pinned snapshot (loader.DEFAULT_DB) + out/recall_<i>.json candidate
sets produced by `cerebro recall --json -n 50` against a scratch copy.
Outputs: out/phase0_report.json + out/figs/*.png.

Reproducibility: link decay is evaluated at a fixed NOW (below), not wall time.
"""

from __future__ import annotations

import glob
import json
import os
from datetime import datetime, timezone

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

import fixtures
from h0 import dominance, graph_sweep, h0_deaths_from_dist, top_gap
from loader import (
    angular_distance_matrix,
    conductance,
    embedding_matrix,
    load_store,
)

NOW = datetime(2026, 7, 27, 12, 0, tzinfo=timezone.utc)
OUT = os.path.join(os.path.dirname(__file__), "out")
FIGS = os.path.join(OUT, "figs")

# Reference dataviz palette (light mode) — see skill references/palette.md.
SURFACE = "#fcfcfb"
INK = "#0b0b0b"
INK2 = "#52514e"
BLUE = "#2a78d6"
ORANGE = "#eb6834"

plt.rcParams.update(
    {
        "figure.facecolor": SURFACE,
        "axes.facecolor": SURFACE,
        "savefig.facecolor": SURFACE,
        "text.color": INK,
        "axes.labelcolor": INK2,
        "xtick.color": INK2,
        "ytick.color": INK2,
        "axes.edgecolor": INK2,
        "axes.grid": True,
        "grid.color": "#e6e5e1",
        "grid.linewidth": 0.6,
        "axes.spines.top": False,
        "axes.spines.right": False,
        "font.size": 10,
    }
)


def barcode_ax(ax, deaths: np.ndarray, title: str, note: str | None = None):
    """Horizontal H0 barcode: every finite bar [0, death), longest at the top."""
    d = np.sort(deaths)[::-1]
    n = len(d)
    ax.hlines(np.arange(n), 0, d, color=BLUE, linewidth=max(0.4, min(2.0, 220 / n)))
    ax.set_ylim(-1, n)
    ax.invert_yaxis()
    ax.set_yticks([])
    ax.set_xlabel("angular distance ε")
    ax.set_title(title, fontsize=10, loc="left")
    if note:
        ax.text(
            0.98, 0.04, note, transform=ax.transAxes, ha="right", va="bottom",
            fontsize=8.5, color=INK2,
        )


def main():
    os.makedirs(FIGS, exist_ok=True)
    report: dict = {"now": NOW.isoformat()}

    memories, links = load_store()
    E, ids = embedding_matrix(memories)
    idx = {mid: i for i, mid in enumerate(ids)}
    norms = np.linalg.norm(E, axis=1)
    report["store"] = {
        "memories": len(memories),
        "with_embedding": len(ids),
        "links": len(links),
        "embedding_norms": {
            "mean": float(norms.mean()),
            "std": float(norms.std()),
            "min": float(norms.min()),
            "max": float(norms.max()),
        },
    }

    # ---- full-store point cloud -------------------------------------------
    D = angular_distance_matrix(E)
    iu = np.triu_indices(len(ids), k=1)
    pair = D[iu]
    deaths = h0_deaths_from_dist(D)
    gap, comps_at_gap = top_gap(deaths)
    report["full_store"] = {
        "pairwise": {
            "mean": float(pair.mean()),
            "p5": float(np.percentile(pair, 5)),
            "median": float(np.median(pair)),
            "p95": float(np.percentile(pair, 95)),
        },
        "h0_deaths": {
            "median": float(np.median(deaths)),
            "p95": float(np.percentile(deaths, 95)),
            "top10": [float(x) for x in np.sort(deaths)[-10:][::-1]],
        },
        "largest_gap": float(gap),
        "components_at_largest_gap": comps_at_gap,
        "p2_over_p1": dominance(deaths),
    }

    fig, axes = plt.subplots(1, 2, figsize=(10, 4.2))
    barcode_ax(
        axes[0],
        deaths,
        f"Full store — H0 barcode ({len(ids)} memories, {len(deaths)} finite bars)",
        note=f"largest gap {gap:.3f} at {comps_at_gap} components",
    )
    ds = np.sort(deaths)
    axes[1].plot(ds, np.arange(len(ds), 0, -1), color=BLUE, linewidth=2)
    axes[1].set_xlabel("angular distance ε")
    axes[1].set_ylabel("components remaining")
    axes[1].set_title("Merge curve", fontsize=10, loc="left")
    fig.tight_layout()
    fig.savefig(os.path.join(FIGS, "full_store_h0.png"), dpi=150)
    plt.close(fig)

    # ---- recall candidate sets --------------------------------------------
    recall_files = sorted(glob.glob(os.path.join(OUT, "recall_*.json")))
    qreports = []
    fig, axes = plt.subplots(1, len(recall_files), figsize=(3.2 * len(recall_files), 3.4))
    for ax, path in zip(np.atleast_1d(axes), recall_files):
        data = json.load(open(path))
        rows = [idx[r["memory"]["id"]] for r in data if r["memory"]["id"] in idx]
        Dq = D[np.ix_(rows, rows)]
        dq = h0_deaths_from_dist(Dq)
        g, c = top_gap(dq)
        p21 = dominance(dq)
        name = os.path.basename(path).replace(".json", "")
        qreports.append(
            {
                "set": name,
                "n": len(rows),
                "median_death": float(np.median(dq)),
                "max_death": float(dq.max()),
                "p2_over_p1": p21,
                "largest_gap": float(g),
                "components_at_gap": c,
            }
        )
        barcode_ax(ax, dq, name, note=f"p2/p1 {p21:.2f} · gap→{c} comps")
        ax.set_xlim(0, 0.5)
    report["recall_sets"] = qreports
    fig.suptitle("Recall candidate sets (top-50 ranked) — H0 barcodes", fontsize=11, y=1.02)
    fig.tight_layout()
    fig.savefig(os.path.join(FIGS, "recall_sets_h0.png"), dpi=150, bbox_inches="tight")
    plt.close(fig)

    # ---- link-graph sweep over spreading conductance ----------------------
    node_ids = [m.id for m in memories]
    nidx = {mid: i for i, mid in enumerate(node_ids)}
    edges = [
        (
            nidx[l.source_id],
            nidx[l.target_id],
            conductance(l.weight, l.last_traversed, l.link_type, NOW),
        )
        for l in links
    ]
    sweep = graph_sweep(len(node_ids), edges)
    strengths = np.array([s for _, _, s in edges])
    frozen = np.array([l.last_traversed is None for l in links])
    raw_sweep = graph_sweep(
        len(node_ids), [(nidx[l.source_id], nidx[l.target_id], l.weight) for l in links]
    )
    report["link_sweep"] = {
        "edges": len(edges),
        "never_traversed_pct": float(frozen.mean() * 100),
        "n_linked": sweep["n_linked"],
        "n_isolated": sweep["n_isolated"],
        "final_components_linked_subgraph": int(sweep["final_components"]),
        "coherence_threshold_conductance": sweep["coherence_threshold"],
        "coherence_threshold_raw_weight": raw_sweep["coherence_threshold"],
        "conductance": {
            "median": float(np.median(strengths)),
            "p5": float(np.percentile(strengths, 5)),
            "p95": float(np.percentile(strengths, 95)),
        },
    }

    fig, axes = plt.subplots(1, 2, figsize=(10, 4.2))
    axes[0].plot(
        sweep["curve_thresholds"], sweep["curve_components"], color=BLUE, linewidth=2
    )
    axes[0].set_xlabel("conductance threshold (sweeping strong → weak)")
    axes[0].set_ylabel("components (linked subgraph)")
    axes[0].invert_xaxis()
    ct = sweep["coherence_threshold"]
    title = "Link graph — components vs conductance"
    if ct is not None:
        title += f" (coheres at {ct:.3f})"
    axes[0].set_title(title, fontsize=10, loc="left")

    bins = np.linspace(0, strengths.max() * 1.02, 40)
    axes[1].hist(
        [strengths[~frozen], strengths[frozen]],
        bins=bins,
        stacked=True,
        color=[BLUE, ORANGE],
        label=["traversed (decaying)", "never traversed (frozen)"],
        edgecolor=SURFACE,
        linewidth=0.4,
    )
    axes[1].set_xlabel("spreading conductance (decayed weight × type weight)")
    axes[1].set_ylabel("links")
    axes[1].set_title("Conductance distribution", fontsize=10, loc="left")
    axes[1].legend(frameon=False, fontsize=8.5)
    fig.tight_layout()
    fig.savefig(os.path.join(FIGS, "link_sweep.png"), dpi=150)
    plt.close(fig)

    # ---- stack sanity: ripser finds the planted H1 circle ------------------
    from ripser import ripser as ripser_run

    Xc = fixtures.noisy_circle()
    Dc = angular_distance_matrix(Xc)
    h1 = ripser_run(Dc, distance_matrix=True, maxdim=1)["dgms"][1]
    pers = h1[:, 1] - h1[:, 0]
    pers = np.sort(pers[np.isfinite(pers)])[::-1]
    report["ripser_h1_sanity"] = {
        "top_persistence": float(pers[0]),
        "second_persistence": float(pers[1]) if len(pers) > 1 else 0.0,
        "planted_circle_found": bool(pers[0] > 5 * (pers[1] if len(pers) > 1 else 1e-9)),
    }

    with open(os.path.join(OUT, "phase0_report.json"), "w") as f:
        json.dump(report, f, indent=2)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
