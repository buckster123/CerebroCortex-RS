# FINDINGS-1 — Phase 1 experiments

Companion to `CEREBRO_TOPO_EXPLORATION_PLAN.md` §4. One section per experiment
as each concludes. E2 first (as sequenced); E1/E3 to follow.

## E2 — Link-graph fragmentation watchdog: GATE PASSED → SHIPPED (2026-07-27)

**Gate recap.** Both halves landed in Phase 0: the union-find sweep is
fixture-exact (fixture (e) coheres at exactly its constructed 0.4 threshold,
`validate.py`), and the real store told a *very* legible story — the FINDINGS-0
headline: 3 permanent components, 23% isolated, 79% frozen links.

**Ship.** `memory_health` now returns a `graph` section:

```json
"graph": {
  "live_memories": 534, "linked_memories": 410,
  "isolated_memories": 124, "isolated_pct": 23.2,
  "live_links": 9424, "never_traversed_links_pct": 79.1,
  "components": 3, "largest_component": 405,
  "islands": [ { "size": 3, "members": [ {"id": "...", "preview": "..."} ] }, ... ],
  "islands_truncated": false
}
```

(The numbers above are the live FORGE brain at ship time — the watchdog names
the Occipital-RS deploy island and the GL-face procedure island by content
preview, exactly the islands Phase 0 identified.)

Design decisions, recorded:

- **Whole-store connectivity, deliberately.** Links carry no scope and
  spreading crosses agents; a scope-filtered component count would report fake
  fragmentation. Island member **previews** are scope-filtered instead —
  out-of-scope members appear as bare ids (test-locked).
- **Computed from SQLite truth, not the petgraph cache** (union-find over live
  memories + live-endpoint links via `petgraph::unionfind`, already a dep) —
  no staleness coupling, works in every front-end (`cerebro-mcp`,
  `cerebro-api`, CLI all share the store method).
- **Islands capped** at 8 islands × 3 member previews (`islands_truncated`
  flags overflow); the largest component never gets a roster — islands are the
  actionable part.
- **The conductance sweep / coherence threshold did NOT ship** into the health
  surface. FINDINGS-0 showed link weights are quantized into a few spikes and
  79% never move — connectivity is the whole signal today. The sweep stays an
  exploration tool (`scripts/topo/`); revisit only if weight dynamics ever go
  live (traversal updates / dream strengthening at real volume).
- Cost: one id scan + one link scan + union-find, only when `memory_health` is
  called. Nano-safe (no embeddings). No new dependencies.

**What the watchdog is for.** The known failure mode is topic-batch accretion:
new work areas link among themselves and never bridge to the older graph, so
spreading activation silently can't reach them from the main mass. Agents (and
dream-phase consumers) now see `components > 1` + named islands and can act —
`associate` a bridging link, or let a future dream phase consume the roster.

**Follow-up candidates** (not scheduled): a dream-phase consumer that proposes
bridges for islands (REM recombination already random-pairs; islands are
better targets); isolated-node triage (23% is a lot of unreachable memory).
