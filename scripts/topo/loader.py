"""Load Cerebro memories / links / embeddings from a pinned SQLite snapshot.

Phase-0 conventions (CEREBRO_TOPO_EXPLORATION_PLAN.md §2, locked here):

- **Distance**: angular, ``d = arccos(clip(cos_sim, -1, 1)) / pi`` in [0, 1].
  Used everywhere; parity fixtures for any Rust port depend on it.
- **Link strength**: spreading conductance ``decayed_link_weight × type_weight``
  — the exact quantity ``activation/spreading.rs`` multiplies per hop, ported
  byte-faithfully below (including the never-traversed ⇒ never-decays rule).

Read-only by design: open the pin, never the live DB.
"""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timezone

import numpy as np

DEFAULT_DB = "/home/andre/Projects/CerebroCortex/data/cerebro.db.topo-pin-20260727"

# types.rs LinkType::activation_weight — mirror exactly.
LINK_TYPE_WEIGHTS = {
    "causal": 0.9,
    "semantic": 0.8,
    "supports": 0.8,
    "part_of": 0.8,
    "contextual": 0.7,
    "derived_from": 0.7,
    "temporal": 0.6,
    "affective": 0.5,
    "contradicts": 0.3,
}

LINK_DECAY_HALFLIFE_DAYS = 30.0  # config.rs


def decayed_link_weight(
    weight: float,
    last_traversed: datetime | None,
    now: datetime,
    halflife_days: float = LINK_DECAY_HALFLIFE_DAYS,
) -> float:
    """Port of ``activation/spreading.rs::decayed_link_weight`` (exact semantics).

    Never-traversed links do NOT decay; ``halflife <= 0`` or ``age <= 0`` return
    the stored weight unchanged; otherwise hyperbolic ``w / (1 + age/(9·h))``
    (halving at 9·h days — 270 d at the default 30-day constant).
    Rust computes age from whole seconds (``num_seconds``); we match by
    truncating to whole seconds before dividing.
    """
    if last_traversed is None:
        return weight
    if halflife_days <= 0.0:
        return weight
    age_days = int((now - last_traversed).total_seconds()) / 86400.0
    if age_days <= 0.0:
        return weight
    return weight / (1.0 + age_days / (9.0 * halflife_days))


def conductance(weight, last_traversed, link_type, now) -> float:
    """decayed weight × link-type weight — spreading's per-link multiplier."""
    return decayed_link_weight(weight, last_traversed, now) * LINK_TYPE_WEIGHTS.get(
        link_type, 0.5
    )


@dataclass
class Memory:
    id: str
    memory_type: str
    layer: str
    salience: float
    created_at: datetime
    agent_id: str | None
    tags: list[str] = field(default_factory=list)
    embedding: np.ndarray | None = None  # 384-dim f32 or None


@dataclass
class Link:
    source_id: str
    target_id: str
    link_type: str
    weight: float
    created_at: datetime
    last_traversed: datetime | None
    traversal_count: int


def _ts(s: str | None) -> datetime | None:
    if s is None:
        return None
    return datetime.fromisoformat(s.replace("Z", "+00:00")).astimezone(timezone.utc)


def load_store(db_path: str = DEFAULT_DB) -> tuple[list[Memory], list[Link]]:
    """All live memories + all links from the snapshot (deleted rows excluded,
    links kept only when both endpoints are live — matching the graph rebuild)."""
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        mem_rows = conn.execute(
            "SELECT id, memory_type, layer, salience, created_at, agent_id, tags, embedding "
            "FROM memories WHERE deleted_at IS NULL"
        ).fetchall()
        memories = [
            Memory(
                id=r[0],
                memory_type=r[1],
                layer=r[2],
                salience=r[3],
                created_at=_ts(r[4]),
                agent_id=r[5],
                tags=json.loads(r[6] or "[]"),
                embedding=(
                    np.frombuffer(r[7], dtype="<f4").copy() if r[7] is not None else None
                ),
            )
            for r in mem_rows
        ]
        live = {m.id for m in memories}
        link_rows = conn.execute(
            "SELECT source_id, target_id, link_type, weight, created_at, "
            "last_traversed, traversal_count FROM links"
        ).fetchall()
        links = [
            Link(r[0], r[1], r[2], r[3], _ts(r[4]), _ts(r[5]), r[6])
            for r in link_rows
            if r[0] in live and r[1] in live
        ]
        return memories, links
    finally:
        conn.close()


def embedding_matrix(memories: list[Memory]) -> tuple[np.ndarray, list[str]]:
    """(n, 384) matrix + row-aligned ids for memories that have a vector."""
    rows = [(m.id, m.embedding) for m in memories if m.embedding is not None]
    ids = [r[0] for r in rows]
    E = np.stack([r[1] for r in rows]).astype(np.float64)
    return E, ids


def angular_distance_matrix(E: np.ndarray) -> np.ndarray:
    """Pairwise angular distance in [0, 1]. Rows are L2-normalized first
    (report raw norms separately — Phase 0 verifies bge arrives ~unit-norm)."""
    norms = np.linalg.norm(E, axis=1, keepdims=True)
    En = E / np.clip(norms, 1e-12, None)
    cos = np.clip(En @ En.T, -1.0, 1.0)
    D = np.arccos(cos) / np.pi
    np.fill_diagonal(D, 0.0)
    return D
