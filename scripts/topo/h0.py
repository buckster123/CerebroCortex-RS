"""H0 persistence: single-linkage merge tree via sorted edges + union-find.

For a point cloud under a distance matrix, every component is born at 0 and the
H0 barcode's finite deaths are exactly the single-linkage merge heights
(Kruskal's MST edge weights). ``h0_deaths_from_dist`` is cross-checked against
``scipy.cluster.hierarchy.linkage(method="single")`` in the fixture suite.

For a weighted graph swept from STRONG to WEAK (super-level filtration —
Cerebro's link sweep), ``graph_sweep`` reports components as strength drops.
"""

from __future__ import annotations

import numpy as np


class UnionFind:
    def __init__(self, n: int):
        self.parent = list(range(n))
        self.rank = [0] * n
        self.components = n

    def find(self, x: int) -> int:
        root = x
        while self.parent[root] != root:
            root = self.parent[root]
        while self.parent[x] != root:  # path compression
            self.parent[x], x = root, self.parent[x]
        return root

    def union(self, a: int, b: int) -> bool:
        ra, rb = self.find(a), self.find(b)
        if ra == rb:
            return False
        if self.rank[ra] < self.rank[rb]:
            ra, rb = rb, ra
        self.parent[rb] = ra
        if self.rank[ra] == self.rank[rb]:
            self.rank[ra] += 1
        self.components -= 1
        return True


def h0_deaths_from_dist(D: np.ndarray) -> np.ndarray:
    """Finite H0 death times (n−1 single-linkage merge heights), ascending."""
    n = D.shape[0]
    iu = np.triu_indices(n, k=1)
    order = np.argsort(D[iu], kind="stable")
    rows, cols, vals = iu[0][order], iu[1][order], D[iu][order]
    uf = UnionFind(n)
    deaths = []
    for r, c, v in zip(rows, cols, vals):
        if uf.union(int(r), int(c)):
            deaths.append(float(v))
            if uf.components == 1:
                break
    return np.asarray(deaths)


def top_gap(deaths: np.ndarray) -> tuple[float, int]:
    """(largest gap between consecutive sorted deaths, component count when the
    filtration sits inside that gap)."""
    if len(deaths) < 2:
        return 0.0, 1
    d = np.sort(deaths)
    gaps = np.diff(d)
    i = int(np.argmax(gaps))
    # After the (i+1)-th merge of n−1 total, components = n − (i+1); with
    # n = len(deaths)+1 that is len(deaths) − i.
    return float(gaps[i]), len(deaths) - i


def dominance(deaths: np.ndarray) -> float:
    """p2/p1 over the two longest finite bars (births are all 0, so persistence
    = death). →1: merges live at one scale (no dominant split); →0: the last
    merge towers over the rest (a two-cluster story). NOTE: the plan's E1 calls
    ``1 − p2/p1`` "coherence (→1 tight)" — formula and label disagree; see
    FINDINGS-0. We report the raw ratio and let the findings name it."""
    if len(deaths) < 2:
        return 1.0
    d = np.sort(deaths)
    return float(d[-2] / d[-1]) if d[-1] > 0 else 1.0


def graph_sweep(
    n_nodes: int, edges: list[tuple[int, int, float]]
) -> dict:
    """Sweep edges strong→weak; record each merge strength and the component
    count trajectory over the LINKED subgraph (nodes appearing in ≥1 edge).

    Returns dict with: merge_strengths (descending order of processing),
    thresholds/components curve arrays over linked nodes, coherence_threshold
    (strength at which the linked subgraph first becomes ONE component; None if
    it never does), n_linked, n_isolated.
    """
    linked = sorted({u for u, _, _ in edges} | {v for _, v, _ in edges})
    index = {node: i for i, node in enumerate(linked)}
    nl = len(linked)
    uf = UnionFind(nl)
    order = sorted(edges, key=lambda e: -e[2])
    merge_strengths: list[float] = []
    curve_t: list[float] = []
    curve_c: list[int] = []
    coherence_threshold = None
    for u, v, s in order:
        if uf.union(index[u], index[v]):
            merge_strengths.append(s)
            curve_t.append(s)
            curve_c.append(uf.components)
            if uf.components == 1 and coherence_threshold is None:
                coherence_threshold = s
    return {
        "merge_strengths": np.asarray(merge_strengths),
        "curve_thresholds": np.asarray(curve_t),
        "curve_components": np.asarray(curve_c),
        "coherence_threshold": coherence_threshold,
        "n_linked": nl,
        "n_isolated": n_nodes - nl,
        "final_components": uf.components,
    }
