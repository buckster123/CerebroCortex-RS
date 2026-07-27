# CEREBRO_TOPO_EXPLORATION_PLAN.md

> Persistent homology & shape-aware memory for CerebroCortex-RS — an *exploration* plan, not an integration plan.
> Written 2026-07-27 for execution by Claude Code against `buckster123/CerebroCortex-RS`.
> Drafted web-side against assumed internals; **reconciled against the actual code and live store the same day** (§0.5).
> Execution happens in this repo first; gate-passers forward-port to ApexOS-RS per the established flow.

---

## 0. Purpose & origin

Two threads converge here:

1. **Topological data analysis (TDA / persistent homology).** Cerebro already maintains the two native habitats for PH: a 384-dim embedding point cloud (`memory_vectors`, cosine/angular geometry) and a weighted association graph (`links`, petgraph cache). PH offers multiscale, threshold-free, noise-stable structure measurements on both.
2. **The skill-invalidation challenge** (from an X reply): *"The real test is whether it can recognize when a previously successful skill becomes wrong as the context changes — not just reinforce what worked before."* Cerebro's activation math (ACT-R + FSRS) is momentum-based: it answers "did this work, how often, how recently." The challenge asks for the orthogonal signal: "is the world still shaped like the world in which it worked." Today the only mechanism is reactive — `record_procedure_outcome` failure drops salience −0.15 / raises FSRS difficulty +0.5, tagging `prune_candidate` at the 0.25 floor — the system pays in failures before it learns. This plan adds a proactive track.

**Deliverable of the whole plan:** evidence-backed go/no-go decisions per feature, a findings report, and (only for features that pass their gates) a minimal Rust integration.

---

## 0.5 Reconciliation against the code (2026-07-27) — READ FIRST

The executor read the dream engine, procedural path, storage schema, recall pipeline, and the live FORGE store. Corrections that bind everything below:

1. **There is no recall-query corpus in the DB.** Recall is deliberately never audited (`dispatch.rs::audit_action` — mutations only). E1's replay source is instead: mine Claude Code session JSONLs (`~/.claude/projects/*/`), which hold months of real `recall` calls with arguments; and/or add opt-in prospective query logging during the exploration window.
2. **Procedure outcome mechanics** (plan originally said "failure → S × 0.5"): actual = salience ±(+0.1/−0.15), FSRS difficulty ∓(−0.3/+0.5), `prune_candidate` tag at salience ≤ 0.25, plus the `metadata.outcomes` {successes, failures} ledger. `wilson_lower_bound()` **already exists** (`dream.rs`, used by the `skill_competition` phase) — S1 extends that machinery (windowed vs lifetime), it does not build parallel stats.
3. **The dream cycle has 8 phases**, not 6 (`variation` and `skill_competition` shipped with the exo-evolution layer). Phase 2's "clustering" is **tag-map grouping** (`CLUSTER_MIN_SIZE = 3`) — that, post-hygiene-fix, is E3's dumb baseline.
4. **The prune predicate can barely delete bridges.** Class A prunes only *stale, low-salience, link-isolated sensory* memories — an isolated node is never a bridge, and the live store has zero sensory rows. Only Class B (`prune_candidate` demoted procedures, pruned regardless of links) can fragment the graph, and it has zero candidates until grading data exists. E4 is reframed accordingly (see §4).
5. **Link decay is hyperbolic and mostly dormant.** `decayed_link_weight = w / (1 + age/(9·30d))` — halving takes **270 days**, and links with `last_traversed = NULL` never decay at all: **79% of live links** (7,453/9,424). "Days until fragmentation by decay" would mostly no-op; the real degradation axis is node-side ACT-R activation. E2's sweep is redefined over current spreading conductance (`decayed_weight × type_weight`), and its forecast mechanic needs a redesign before it means anything.
6. **Store shape (live FORGE brain, 2026-07-27):** 532 memories — 513 working / 19 long_term / 0 cortex / 0 sensory; 9,424 links (8,339 semantic); 122 isolated nodes; zero graded procedures ever. E5's "LongTerm+Cortex" sample frame doesn't exist here — sample the whole store.
7. **Embedding coverage was the blocker and is now fixed.** Only 28/532 memories had vectors (Python-migrated rows predate the Rust embedder). `cerebro backfill` shipped (PR #11) and ran: **532/532 embedded** (33 s), snapshot `cerebro.db.pre-backfill-20260727` retained. It doubles as the re-embed tool for any future embed-model migration (E6's use case; the bge-large/NPU migration itself is parked for now).
8. **Side-wins already shipped from this reading** (PR #10): Phase 2 now filters structural/bookkeeping tags (`session_note` 269 / `priority:HIGH` 228 / `session_type:technical` 189 were the three biggest "clusters" — grab-bag groups eating LLM budget), `is_structural_tag` extended with the bookkeeping namespaces (also cleans niche detection in `variation`/`skill_competition`), and the Class-A isolation check now sees both link directions (spreading is bidirectional; outgoing-only called incoming-only hubs "isolated").
9. Minor: tool surface is **67** (not 66); `memories` has 21 columns; graph edges are directed in petgraph but spreading traverses both directions — treat connectivity as undirected, which matches the union-find framing here.
10. **The S-track's real bottleneck is behavioral, not schema.** Nothing grades procedures today (0 ledger rows, 0 audit rows for `record_procedure_outcome`). The grading habit is now in CLAUDE.md's session protocol; apex1 already does this emergently and the colony prompts should lock it in when S0 lands. Audit rows do give grading *timestamps* (but not outcomes — `audit_details` captures neither `success` nor context), so S0's dedicated table remains the right call.

---

## 1. Ground rules

1. **Lego first.** All experiments prototype in Python against exported/live data. Rust only after a gate passes. The Python artifacts become the parity fixtures for the Rust port (house style: match within tolerance, like the ACT-R/FSRS 1e-4 fixtures).
2. **Every fancy method fights a dumb baseline.** If articulation points match PH bridge detection on real data, we ship articulation points. If centroid drift matches diagram distance, we ship centroids. PH must earn each slot.
3. **Decision gates are binding.** Each experiment defines its gate up front. A negative result is a valid, documented outcome (`FINDINGS-*.md`), not a failure of the exploration.
4. **No new dependencies in core crates** until Phase 4, and then only behind a `topo` feature flag. H0 machinery is hand-rolled (union-find + sorted edges — zero deps by design).
5. **Pi budgets are law.** Anything on the recall hot path must add < 1 ms p99 on Pi 5. Expensive computation lives in dream-time only. Nano tier (no embeddings) must keep working — the link-graph features (E2, E4) work on Nano since they need no embeddings at all.
6. **Visualization ≠ decisions.** UMAP/2-D projections are for human eyeballs only; no decision logic ever runs on projected coordinates (projection distorts topology).
7. **Read before writing.** Done 2026-07-27 — results in §0.5. Re-verify anything load-bearing at execution time; the store keeps moving.

---

## 2. Two-minute primer (for the executor)

- Grow a ball of radius ε around every point simultaneously; connect points whose balls overlap (Vietoris–Rips). Sweep ε from 0 upward.
- **H0** tracks connected components: each component is *born* at ε=0 and *dies* when it merges into an older one. The H0 barcode is exactly the single-linkage merge tree; computing it is Kruskal's MST + union-find. Cost: sort edges, near-linear after that.
- **H1** tracks loops: a ring of points enclosing an empty region is born when the ring closes and dies when the hole fills in. Needs real PH machinery (matrix reduction) — this is the expensive part.
- **Persistence** = death − birth. Long bars are structure; short bars are noise. Stability theorems: small perturbations of the input move the diagram only a little (bottleneck distance bound). This is why barcodes make good health metrics: they're comparable across time.
- Distance for embeddings: verify bge vectors arrive normalized from fastembed, then use angular distance `d = arccos(clamp(cos_sim))/π`, or `1 − cos_sim` at exploration level. Pick one in Phase 0, document it, use it everywhere.

---

## 3. Phase 0 — Harness (½ day)

**Goal:** frictionless access to real Cerebro data from Python, plus synthetic fixtures with known ground truth.

- **Data access.** The single SQLite file is the source of truth. **Pin a fresh post-backfill copy** (`VACUUM INTO`) at Phase 0 start and run all experiments against the pin, not the live file. Read:
  - `memories` (21 cols; `embedding` BLOB — now populated store-wide, 384-dim f32 LE; salience, layer, type, timestamps, `access_times` JSON)
  - `memory_vectors` (vec0; rowid-joined to `memories`; needs the sqlite-vec extension loaded, or just read the BLOB column)
  - `links` (`source_id`, `target_id`, `link_type`, `weight`, `created_at`, `last_traversed`, `traversal_count`) → port `decayed_link_weight` from `activation/spreading.rs` **exactly** (incl. the no-`last_traversed` ⇒ no-decay rule) and parity-check it
  - Recall-query corpus: **Claude Code transcripts**, not audit_log (§0.5.1)
- **Environment.** `python3 -m venv` (PEP 668), install: `numpy`, `scipy`, `ripser` (ripser.py), `persim` (diagram distances), `hdbscan`, `scikit-learn`, `matplotlib`. (`giotto-tda` optional; heavier.)
- **Synthetic fixtures** (ground truth for gates): (a) one Gaussian blob; (b) two well-separated blobs; (c) two blobs + thin bridge of points; (d) a noisy circle embedded in 384-dim (plant a real H1 feature); (e) a link graph with known articulation points and a controllable weight-decay schedule.
- **Deliverables:** `scripts/topo/` with a loader module + notebook or script; `FINDINGS-0.md` with: barcode of the full real store (532 points, H0 only), barcodes of ~5 real recall candidate sets, link-graph H0 barcode over spreading conductance. Just *look* before optimizing anything.

---

## 4. Phase 1 — Cheap experiments (H0 only)

Each experiment: **Method / Baseline / Metric / Gate**. All are union-find-cheap.

### E1 — Retrieval coherence score (recall / prefrontal)

- **Method:** For each replayed transcript query, take the post-spreading-activation candidate set (≤ 50 nodes — `SPREADING_MAX_ACTIVATED`). Compute pairwise angular distances, MST, H0 barcode. Coherence scalar: `C = 1 − p2/p1` where `p1`, `p2` are the two longest finite bar persistences (C→1: one tight idea; C→0: two rival clusters). Also record component count at the largest merge gap.
- **Baseline:** silhouette score with k=2 KMeans; mean pairwise distance.
- **Metric:** on ~100 replayed real queries, does low-C flag queries a human (André) or an LLM judge considers ambiguous / multi-topic? Small-sample precision, eyeball-honest.
- **Gate:** low-dominance flags are judged genuinely ambiguous in ≥ ~70% of cases AND the baseline doesn't match that.
- **Status: GATE FAILED (FINDINGS-1)** — chance AUC on labeled controls for the metric AND both baselines; constructed-union follow-up shows the metric is nearly blind at this store's concentration, with retrieval dissolving mixtures on top. Parked with a written re-test trigger (materially different store or embed model).

### E2 — Link-graph fragmentation watchdog (memory_health)

- **Method:** Sort links by descending **spreading conductance** (`decayed_link_weight × type_weight` — the exact quantity spreading multiplies; not the raw stored weight); union-find sweep = H0 barcode over association strength. Report: weight threshold at which the graph first coheres into one giant component ("coherence threshold"), number of long-lived islands, total persistence. **Forecast redesign needed** (§0.5.5): straight aging moves only the 21% of links ever traversed. Candidate honest forecast: sweep node-side ACT-R activation decay instead, or report both axes separately. Design in FINDINGS-1 before wiring anything.
- **Baseline:** plain connected-component count at a fixed weight cutoff.
- **Metric:** does the barcode reveal structure the fixed-cutoff count misses (e.g., a large sub-community about to detach)? Behavior on fixture (e) must be exactly as constructed.
- **Gate:** correct on fixtures + tells a legible story on the real store. Expected to pass — cheap, Nano-safe, instruments a *silent* failure mode (spreading activation quietly degrading as the graph shatters). Pass → `memory_health` extension in Phase 4.
- **Status: GATE PASSED (FINDINGS-0) and SHIPPED (FINDINGS-1)** — `memory_health` now returns a `graph` section (components, island roster with scope-honest previews, isolated count/pct, never-traversed-link share). The conductance sweep stayed exploration-side: weights are quantized + 79% frozen, so connectivity is the whole signal.

### E3 — Persistence-stable clustering for Dream Phase 2/3

- **Current mechanism (verified):** tag-map grouping, `CLUSTER_MIN_SIZE = 3`, structural/bookkeeping tags excluded since PR #10. That post-fix behavior is the baseline to beat.
- **Method:** HDBSCAN on angular distance (HDBSCAN's cluster-stability selection is persistence under the hood) on the same ≤500-memory samples Phase 2 sees.
- **Baseline:** filtered tag-grouping (above); plus KMeans with silhouette-chosen k.
- **Metric:** cluster stability under 90% resampling (adjusted Rand index between runs); silhouette; qualitative check — run the same pattern-extraction prompt over both clusterings for a few samples and judge summary tightness.
- **Gate:** stability improvement without pathological cluster counts, at dream-time-acceptable cost. Pass → Phase 2/3 clustering swap candidate. Note: embedding clusters would also catch untagged-but-related memories tag-grouping structurally cannot.

### E4 — Bridge immunity for Dream Phase 5 pruning — **reframed, parked**

- §0.5.4: Class A cannot prune a bridge (isolation is a precondition; direction bug fixed in PR #10). The only real exposure is **Class B**: a `prune_candidate` procedure that happens to be an articulation point gets deleted with all its links. Zero candidates exist until grading data accumulates (S0).
- **Parked until the fitness ledger is live.** When revisited: articulation-point check (undirected, live links) on Class-B candidates only, inside Phase 5, reported in the dream report — cheap enough to ship on first real candidate. The embedding-MST flavor is dropped unless Class-B bridges actually appear.

---

## 5. Phase 2 — Expensive experiments (gated on Phase 1 signal)

### E5 — H1 hole hunting for REM recombination

- **Honesty first:** high-dim embedding clouds often carry few meaningful H1 features; this experiment is allowed to fail, and a documented negative result is valuable.
- **Method:** landmark subsample (256–512 points, maxmin sampling) of the whole store (532 points today — §0.5.6; the original LongTerm+Cortex frame is empty here); `ripser.py` with `maxdim=1`, `do_cocycles=True`. Take the top ~5 most persistent H1 classes; extract member memories via representative cocycles; print them.
- **Metric:** human + LLM judgment: is the loop semantically coherent, and is the enclosed "hole" nameable as a missing concept/abstraction?
- **Gate:** ≥ 1 clearly meaningful hole in real data → prototype the REM wiring: Phase 6 currently samples **random pairs** (`REM_SAMPLE_SIZE = 20`, `REM_PAIR_CHECKS = 10`, 70% same-type skip) — the insertion point is clean: feed the cycle's memories to a Phase-6-style prompt — *"these memories form a closed loop of association; what concept sits in the middle?"* — write the answer as a Schematic memory linked to the cycle members, and verify on the next run that the corresponding bar shortens. Fail → park H1 entirely; H0-only integration proceeds regardless.

### E6 — Longitudinal drift monitoring

- **Method:** periodic (weekly or per-dream) H0 diagrams on a fixed-size sample; compare consecutive diagrams via bottleneck and 1-Wasserstein (`persim`). Calibrate a null distribution by bootstrap resampling within one snapshot — alarms fire only above null spread.
- **Concrete use case to design for:** any future embed-model migration (bge-large / NPU-served bge — parked for now, but the mechanism is: clear vectors → `cerebro backfill` re-embeds). Cosine values aren't comparable across models; diagram *shape* comparison pre/post re-embed is the check that the memory's structure survived the swap.
- **Gate:** alarms fire on injected synthetic drift (fixture (b) morphing) and stay quiet across normal snapshots. Pass → dream-time `topo_health` job in Phase 4.

---

## 6. Phase 3 — Skill invalidation track (the X question)

Deliberately its own track: the core here is statistics and logging; topology is the shape-sensitive upgrade, not the foundation. S1 ships on its own merits even if all PH work is parked. **Cold-start reality (§0.5.10): zero grading history exists — S0 and the grading habit gate everything downstream.**

### S0 — Prerequisite: per-invocation context logging (schema addition)

- Verified: `record_procedure_outcome` keeps only aggregate `metadata.outcomes` counters (+ an audit row with timestamp but no outcome, no context).
- Add `procedure_invocations` (id, procedure_id, ts, outcome, context_embedding BLOB, optional episode_id/step ref). Migration in house style; Nano tier stores rows without embeddings (S1 still works there).
- **Open design question — context capture.** The server doesn't know "what the agent was doing" unless told. Options: (a) optional `context` string arg on `record_procedure_outcome` (agent passes one line; wire-compatible, needs protocol habit); (b) server-side proxy: last recall query seen from that agent within N minutes (cheap in-memory map); (c) open-episode linkage when present. Decide at S0 implementation; (a)+(b) compose well.
- Nothing downstream is testable on real data until this has accumulated — so land S0 early and let it record while E-experiments run. Grading habit: in CLAUDE.md here; colony prompts when S0 lands.

### S1 — Windowed outcome statistics (non-topological, ships regardless)

- **Method:** per procedure, Beta-posterior / Wilson interval on last-N-window success rate vs lifetime rate; simple trend test. **Reuse `wilson_lower_bound()`** (dream.rs) — same statistic the skill_competition phase already ranks by.
- **Signal:** "this skill's recent reality has detached from its reputation" — catches degradation *earlier* than the −0.15 salience nudge, which reacts per-failure with no memory of pattern.
- **Gate:** correct behavior on synthetic outcome streams (stationary, sudden-break, slow-drift). Near-certain ship: cheap, no embeddings, pure win.

### S2 — Context drift: "skill applied out-of-distribution"

- **Method:** compare the context cloud where the procedure historically *succeeded* vs the last-N invocation contexts. Baselines: centroid angular shift; mean pairwise distance; MMD (RBF kernel). Shape version: Wasserstein distance between the two clouds' H0 diagrams.
- **Why shape can matter:** design the synthetic test that kills centroids — success contexts split into two islands while recent usage falls in the void *between* them. Centroid of usage ≈ centroid of success (looks fine); the diagrams differ loudly.
- **Gate:** topo metric flags the island/void case the centroid misses, on synthetic AND at least one real procedure once S0 data exists. If MMD alone catches everything real, ship MMD — rule 2 applies.

### S3 — Failure-cluster detection → boundary-condition memories

- **Method:** label invocation contexts by outcome. (a) Baseline: can a k-NN / logistic classifier separate success from failure contexts (AUC)? If yes, failures are *contextual*, not random — already a headline. (b) Shape version: H0 on failure-labeled points; a persistent failure component (vs shuffled-label bootstrap null) = "the context changed in a coherent, nameable region."
- **The payoff mechanic:** feed the failure cluster's contexts to a dream-phase prompt — *"describe what these failure contexts have in common"* — and write the answer as a boundary-condition Schematic memory linked to the procedure. `find_relevant_procedures` can then surface the caveat alongside the skill: trust becomes context-conditional instead of scalar.
- **Gate:** stable failure clusters under resampling on real data (needs S0 history) → wire the dream prompt prototype.

### S4 — Revalidation loop (design sketch; gated on S2)

- Trust decays without revalidation: reinterpret the FSRS clock so a procedure's effective trust reflects time since last *validated-in-the-current-context-regime*, not merely last use. When S2's drift score crosses θ, schedule a `store_intention` — "re-test procedure X in current context" — closing the loop through prospective memory. Optionally route the drift alert through the Amygdala path with elevated arousal so agents actually *notice*: the system dreams its own skepticism.
- **Gate:** paper design + agent-in-the-loop trial on one real procedure. No schema surgery beyond S0 until S2 has passed.

---

## 7. Phase 4 — Rust integration map (only gate-passers land)

- **Module layout:** `crates/cerebro/src/topo/` → `mod.rs`, `union_find.rs`, `h0.rs` (barcode from an edge list — shared by E1/E2/S2), `coherence.rs`, `drift.rs`. Procedural-track stats (S1) live in the procedural engine, not `topo/`.
- **Dependencies:** none for anything H0 (hand-rolled). If and only if E5 passed: H1 behind feature `topo-h1`, evaluating `ripser` bindings vs `lophat` vs `oat_rust` — re-verify versions and maintenance status at execution time.
- **Surface:** optional `coherence` field on recall (feature `topo`, default off); `memory_health` gains fragmentation fields (coherence threshold, island count); new MCP tools only if warranted: `topo_health`, `procedure_health` (S1/S2/S3 outputs). Keep the **67**-tool surface stable unless a tool genuinely earns a slot.
- **Perf budgets (Pi 5 / Standard tier):** recall-path additions < 1 ms p99; dream-time topo jobs < 5 s per cycle; RSS overhead negligible; Nano tier: link-graph features only, embedding-dependent features cleanly disabled with `CEREBRO_EMBED_MODEL=""`.
- **Tests, house style:** Python-prototype parity fixtures (barcodes match within tolerance); property tests — barcode invariant under point permutation; jittered fixtures move diagrams within the stability bound; E2 sweep monotone in its decay axis. Wire into the existing suite structure.

---

## 8. Gate summary

| ID | Feature | Gate | Status / cost if shipped |
|----|---------|------|--------------------------|
| E1 | Recall coherence scalar | flags real ambiguity ≥ ~70%, beats silhouette baseline | **FAILED — parked** (FINDINGS-1: chance AUC, two-layer cause; re-test trigger written) |
| E2 | Fragmentation watchdog | fixture-exact + legible on real store | **SHIPPED** — `memory_health.graph` (FINDINGS-1) |
| E3 | Persistence-stable dream clustering | resampling stability ↑, sane counts | baseline = post-#10 filtered tag-grouping; dream-time |
| E4 | Bridge immunity in pruning | Class-B bridge exists on real data | **parked** until grading data (§4) |
| E5 | H1 hole hunting → REM targeting | ≥ 1 nameable hole in real data | dream-time, feature-gated |
| E6 | Drift monitoring / migration check | alarms on synthetic drift, quiet otherwise | migration parked; dream-time |
| S0 | Invocation context logging | schema lands + accumulates | **first mover** — one table + context-capture decision |
| S1 | Windowed outcome stats | correct on synthetic streams | reuse wilson_lower_bound; ships regardless |
| S2 | Context OOD detection | catches island/void case baselines miss | needs S0 history; dream-time |
| S3 | Failure clusters → boundary memories | stable clusters on real history | needs S0 history; dream-time + 1 LLM call |
| S4 | Revalidation intentions | design + one live trial | after S2 only |

Done before the exploration even starts: backfill (PR #11, run — 532/532), Phase-2 tag hygiene + Phase-5 direction fix (PR #10).

---

## 9. Risks & honest unknowns

- **Distance concentration in 384-dim:** cosine gaps get subtle; merge-gap statistics may be mushy on real data. Mitigation: gates demand real-data legibility, not just fixture wins.
- **H1 may be semantically empty** on this store. Planned for: E5's negative result is a documented outcome, and nothing else depends on it.
- **Small/young store:** 532 nodes today; features must no-op gracefully below a node-count floor, and barcode statistics carry small-sample noise — bootstrap everything.
- **S-track cold start is behavioral:** zero gradings exist; without the grading habit sticking (protocol + colony prompts), S2/S3 never become testable. Sequence S0 first, evaluate last.
- **Diagram-distance calibration:** raw bottleneck numbers are meaningless without the bootstrap null; never ship an alarm without one.
- **Angular vs 1−cos:** pick one in Phase 0, document it, use it everywhere; parity fixtures depend on it.

## 10. Non-goals

No Mapper, no persistence landscapes/images ML pipelines, no GPU paths, no changes to the recall score formula (0.35/0.30/0.20/0.15 stays — verified in `config.rs`), no UMAP-derived decisions, no new always-on background threads, no Python runtime dependencies in shipped binaries — exploration scripts stay in `scripts/topo/` and never block the build.

---

*Branch: `explore/topo`. Findings reports (`FINDINGS-0.md`, `FINDINGS-1.md`, …) live beside this plan. The executor amends this plan where code-reading contradicts it — loudly, in §0.5 and the findings.*
