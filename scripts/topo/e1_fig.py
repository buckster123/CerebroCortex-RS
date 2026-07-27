"""E1 summary figure: dominance z-scores by set kind — the whole verdict at a
glance (total overlap between tight, ambiguous, and even constructed unions)."""

import json
import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

OUT = os.path.join(os.path.dirname(__file__), "out", "e1")
SURFACE, INK, INK2 = "#fcfcfb", "#0b0b0b", "#52514e"
BLUE, ORANGE, AQUA = "#2a78d6", "#eb6834", "#1baf7a"

plt.rcParams.update({
    "figure.facecolor": SURFACE, "axes.facecolor": SURFACE,
    "savefig.facecolor": SURFACE, "text.color": INK,
    "axes.labelcolor": INK2, "xtick.color": INK2, "ytick.color": INK2,
    "axes.edgecolor": INK2, "axes.grid": True, "grid.color": "#e6e5e1",
    "grid.linewidth": 0.6, "axes.spines.top": False,
    "axes.spines.right": False, "font.size": 10,
})

e1 = json.load(open(os.path.join(OUT, "e1_report.json")))
un = json.load(open(os.path.join(OUT, "e1_union_report.json")))

groups = [
    ("tight\n(one topic)", [r["dominance_z_vs_random"] for r in e1["results"] if r["group"] == "tight"], BLUE),
    ("ambiguous\n(two topics, via recall)", [r["dominance_z_vs_random"] for r in e1["results"] if r["group"] == "ambiguous"], ORANGE),
    ("constructed union\n(two topics, bypassing recall)", [u["z"] for u in un["unions"]], AQUA),
]

rng = np.random.default_rng(3)
fig, ax = plt.subplots(figsize=(7.2, 4.2))
ax.axhspan(-1, 1, color="#efeeea", zorder=0)
ax.axhline(0, color=INK2, linewidth=0.8, zorder=1)
for gi, (name, zs, color) in enumerate(groups):
    x = gi + rng.uniform(-0.13, 0.13, size=len(zs))
    ax.scatter(x, zs, s=42, color=color, edgecolor=SURFACE, linewidth=1.0, zorder=3)
    ax.hlines(np.mean(zs), gi - 0.24, gi + 0.24, color=color, linewidth=2.4, zorder=4)
ax.set_xticks(range(len(groups)), [g[0] for g in groups])
ax.set_ylabel("merge-dominance z vs 200 random size-matched sets")
ax.set_title(
    "E1 verdict: candidate-set H0 shape does not detect query ambiguity\n"
    "(gray band = bootstrap null ±1σ; group means as thick ticks)",
    fontsize=10, loc="left",
)
ax.text(0.99, 0.02,
        "AUC — recall controls: 0.44 · constructed unions: 0.60 (chance = 0.5)",
        transform=ax.transAxes, ha="right", va="bottom", fontsize=8.5, color=INK2)
fig.tight_layout()
fig.savefig(os.path.join(OUT, "e1_verdict.png"), dpi=150)
print("saved", os.path.join(OUT, "e1_verdict.png"))
