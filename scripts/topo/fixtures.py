"""Synthetic fixtures with known ground truth (plan §3).

384-dim lesson baked in: noise is parameterized by total NORM (per-coordinate
sigma = norm/sqrt(dim)). Naive per-coordinate sigma=0.05 in 384-dim gives noise of
norm ~0.98 — as large as the unit center itself — and the "tight blob" dissolves
(distance concentration, plan §9). The first validate.py run caught exactly that.

(a) one Gaussian blob            — H0 deaths at one scale, no dominant gap
(b) two well-separated blobs     — one towering final merge, gap count = 2
(c) two blobs + thin bridge      — the bridge pulls the final merge far down
(d) noisy circle in 384-dim      — a real H1 feature planted for ripser
(e) link graph with known articulation point + controllable decay schedule
"""

from __future__ import annotations

import numpy as np


def blob(n=60, dim=384, noise_norm=0.12, seed=1) -> np.ndarray:
    rng = np.random.default_rng(seed)
    sigma = noise_norm / np.sqrt(dim)
    center = rng.normal(size=dim)
    center /= np.linalg.norm(center)
    X = center + sigma * rng.normal(size=(n, dim))
    return X / np.linalg.norm(X, axis=1, keepdims=True)


def two_blobs(n=60, dim=384, noise_norm=0.12, seed=2) -> tuple[np.ndarray, np.ndarray]:
    """Two tight blobs around near-orthogonal centers; returns (X, labels)."""
    rng = np.random.default_rng(seed)
    sigma = noise_norm / np.sqrt(dim)
    c1 = rng.normal(size=dim)
    c1 /= np.linalg.norm(c1)
    c2 = rng.normal(size=dim)
    c2 -= (c2 @ c1) * c1  # orthogonalize → angular distance ≈ 0.5
    c2 /= np.linalg.norm(c2)
    half = n // 2
    X1 = c1 + sigma * rng.normal(size=(half, dim))
    X2 = c2 + sigma * rng.normal(size=(n - half, dim))
    X = np.vstack([X1, X2])
    X /= np.linalg.norm(X, axis=1, keepdims=True)
    labels = np.array([0] * half + [1] * (n - half))
    return X, labels


def two_blobs_bridge(n=60, n_bridge=12, dim=384, noise_norm=0.12, seed=3) -> np.ndarray:
    """Fixture (b) plus a thin chain of points interpolating the two centers."""
    X, _ = two_blobs(n, dim, noise_norm, seed)
    rng = np.random.default_rng(seed + 100)
    sigma = noise_norm / np.sqrt(dim)
    c1 = X[: n // 2].mean(axis=0)
    c2 = X[n // 2 :].mean(axis=0)
    ts = np.linspace(0.08, 0.92, n_bridge)[:, None]
    B = (1 - ts) * c1 + ts * c2 + (sigma / 3) * rng.normal(size=(n_bridge, dim))
    X = np.vstack([X, B])
    return X / np.linalg.norm(X, axis=1, keepdims=True)


def noisy_circle(n=80, dim=384, radius=0.35, noise_norm=0.02, seed=4) -> np.ndarray:
    """A planted H1 feature: a circle in a random 2-plane, unit-normalized."""
    rng = np.random.default_rng(seed)
    sigma = noise_norm / np.sqrt(dim)
    base = rng.normal(size=dim)
    base /= np.linalg.norm(base)
    u = rng.normal(size=dim)
    u -= (u @ base) * base
    u /= np.linalg.norm(u)
    v = rng.normal(size=dim)
    v -= (v @ base) * base + (v @ u) * u
    v /= np.linalg.norm(v)
    theta = np.linspace(0, 2 * np.pi, n, endpoint=False)
    X = (
        base[None, :]
        + radius * (np.cos(theta)[:, None] * u + np.sin(theta)[:, None] * v)
        + sigma * rng.normal(size=(n, dim))
    )
    return X / np.linalg.norm(X, axis=1, keepdims=True)


def link_graph_fixture() -> tuple[int, list[tuple[int, int, float]], int]:
    """Two 5-cliques joined ONLY through node 4 (the articulation point).
    Returns (n_nodes, edges(u, v, strength), articulation_node). Clique edges
    are strong (0.8), the two joining edges weaker (0.4) — so a sweep sees two
    islands cohere into one exactly at 0.4."""
    edges = []
    for base in (0, 5):
        nodes = list(range(base, base + 5))
        for i, a in enumerate(nodes):
            for b in nodes[i + 1 :]:
                edges.append((a, b, 0.8))
    edges.append((4, 5, 0.4))
    edges.append((4, 9, 0.4))
    return 10, edges, 4
