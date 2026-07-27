"""Phase-0 gate: the hand-rolled pieces must match references exactly.

1. H0 deaths == scipy single-linkage merge heights on fixtures (a)-(d).
2. decayed_link_weight matches hand-computed values (incl. the never-traversed
   and 270-day-halving cases).
3. Fixture (b) shows the two-cluster signature; (e) coheres exactly at 0.4.
Run: .venv/bin/python validate.py
"""

from datetime import datetime, timedelta, timezone

import numpy as np
from scipy.cluster.hierarchy import linkage
from scipy.spatial.distance import squareform

import fixtures
from h0 import graph_sweep, h0_deaths_from_dist, top_gap, dominance
from loader import angular_distance_matrix, decayed_link_weight

ok = 0


def check(name, cond):
    global ok
    assert cond, f"FAIL: {name}"
    ok += 1
    print(f"  ok  {name}")


print("[1] H0 vs scipy single-linkage")
for name, X in [
    ("blob", fixtures.blob()),
    ("two_blobs", fixtures.two_blobs()[0]),
    ("two_blobs_bridge", fixtures.two_blobs_bridge()),
    ("noisy_circle", fixtures.noisy_circle()),
]:
    D = angular_distance_matrix(X)
    deaths = np.sort(h0_deaths_from_dist(D))
    Z = linkage(squareform(D, checks=False), method="single")
    check(f"{name}: deaths == scipy merge heights", np.allclose(deaths, np.sort(Z[:, 2]), atol=1e-12))

print("[2] decayed_link_weight parity")
now = datetime(2026, 7, 27, tzinfo=timezone.utc)
check("never traversed → unchanged", decayed_link_weight(0.7, None, now) == 0.7)
check(
    "270 days → exactly half",
    abs(decayed_link_weight(0.8, now - timedelta(days=270), now) - 0.4) < 1e-9,
)
check(
    "30 days → w/(1+1/9) = 0.9w",
    abs(decayed_link_weight(1.0, now - timedelta(days=30), now) - 0.9) < 1e-9,
)
check("future timestamp → unchanged", decayed_link_weight(0.5, now + timedelta(days=1), now) == 0.5)

print("[3] fixture signatures")
Db = angular_distance_matrix(fixtures.two_blobs()[0])
deaths_b = h0_deaths_from_dist(Db)
gap, comps = top_gap(deaths_b)
check("two_blobs: components at largest gap == 2", comps == 2)
check("two_blobs: final merge dominates (p2/p1 < 0.5)", dominance(deaths_b) < 0.5)

Da = angular_distance_matrix(fixtures.blob())
check("blob: no dominant split (p2/p1 > 0.5)", dominance(h0_deaths_from_dist(Da)) > 0.5)

Dc = angular_distance_matrix(fixtures.two_blobs_bridge())
check(
    "bridge pulls the final merge down vs clean two_blobs",
    h0_deaths_from_dist(Dc).max() < deaths_b.max() * 0.6,
)

n, edges, art = fixtures.link_graph_fixture()
sweep = graph_sweep(n, edges)
check("link fixture: coheres exactly at 0.4", sweep["coherence_threshold"] == 0.4)
check("link fixture: no isolated nodes", sweep["n_isolated"] == 0)

print(f"\nall {ok} checks passed")
