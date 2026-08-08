# Lucida — the observatory over a living brain

> UI/dashboard design charter for CerebroCortex-RS. Web (embedded in `cerebro-api`)
> + Slint native, house pattern per Prefrontal-RS (`ui-web/` + `ui-slint/`).
> Drafted 2026-08-07. Status: **charter — not yet built**.

*Lucida* (astronomy): the brightest star in a constellation. The UI's job is to
show which memories are shining, which are fading, and what lights up when the
brain thinks.

---

## Why not a hairball

Every memory product renders the same force-directed bubble graph. It answers
"what is connected to what" and nothing else — and at 5k nodes it answers
nothing at all. Cerebro's data model is *richer than a graph*: every node has
current activation (ACT-R), a decay trajectory (FSRS), salience, emotional
valence, a layer, and every link has effective weight, traversal recency, and a
Hebbian ratchet count. The recall pipeline produces a literal trace of which
edges carried activation (`spread_traced`). None of that fits in a hairball —
all of it fits in a **night sky**.

## The core metaphor: a starfield, not a graph

Memories are stars on a dark field. The layout is **semantic**, not
force-directed: a cached 2D projection of the 384-dim embeddings, so
semantically close memories physically cluster — constellations *are* topics.
Stable across sessions (no force-sim jitter; your brain doesn't reshuffle
every time you look at it).

| Channel | Meaning | Source |
|---------|---------|--------|
| Position | semantic neighborhood | embedding projection (cached, server-side) |
| Brightness / glow | current activation / retrievability | ACT-R + FSRS live compute |
| Size | salience | `salience` |
| Hue | memory type (6-way categorical) | `memory_type` |
| Warm/cool tint lens | emotional valence × intensity | amygdala fields |
| Depth (parallax layer) | memory layer | `layer` (working floats foreground) |
| Link opacity | effective weight (decayed) | `decayed_link_weight` |
| Link temperature | traversal recency | `last_traversed` (cold = never walked) |

Links are **near-invisible by default** — the field reads as stars, and edges
fade in around whatever you hover/focus. Decay is visible truth: an unused
brain literally dims; a fresh recall leaves a warm constellation behind.

## The five lenses

Same field, five ways of looking:

1. **Atlas** (default) — pan/zoom the sky. Hover = memory card (content head,
   tags, activation sparkline via `activation_curve`, FSRS state). Click =
   focus: neighbors + edges light up, everything else recedes. Search box
   flies you to matches.
2. **Thought (recall replay)** — the flagship. Type a query: seed memories
   ignite, then the *actual* spreading-activation wavefront propagates hop by
   hop along the real walked edges with real conductances, and the final
   recalled set crystallizes as a ranked constellation. Powered by a traced
   recall endpoint (see below). This is "watch the brain think" — no other
   memory system can show this because no other system exposes its spread.
3. **Dream observatory** — dream reports on a timeline; scrub a cycle to see
   phase effects as overlay diffs: pruned memories collapse to embers, a new
   schema is born as a bright star with rays to its source memories,
   strengthened links pulse, the skill-competition champion gets a halo and
   its dominated rivals visibly dim toward the prune floor. The exo-evolution
   loop, watchable.
4. **Health** — the fragmentation watchdog made visible: components tinted,
   islands ringed, never-traversed links drawn cold blue, `activation_at_risk`
   memories guttering at the edge of visibility. `memory_health` as a place,
   not a JSON blob.
5. **Live (EEG)** — near-real-time: when an agent remembers/recalls/dreams,
   the event appears as a ripple within a second or two. Powered by tailing
   the audit log (see below) — no IPC between the MCP daemon and the API
   daemon needed; the shared SQLite is already the bus.

## The conventional half

An observatory still needs instruments. Right rail / drawer panels, all backed
by existing routes: stats header (`/stats`, `/graph/stats`), search-with-
explain (`/recall` through the real pipeline), memory inspector (card +
versions + neighbors + restore), episodes, procedures **with the fitness
ledger** (outcome counts, Wilson rank, champion badges — the ledger deserves a
UI), intentions, schemas with sources, tags, trash lifecycle, dream runs,
audit summary. Nothing modal; the sky stays visible behind a translucent rail.

**CRUD is in scope** — the API already supports all of it: create (a compose
box that stores through the real `remember` pipeline, thalamus gate included —
a rejection is shown honestly), edit (content/tags/salience/visibility; content
edits version-snapshot server-side), trash lifecycle (soft-delete → restore →
purge, purge behind a confirm), and link-drawing: in focus mode, select a
second star to `associate` them with a chosen link type. Destructive actions
never live on the field itself — the sky is for reading; the card is for acting.

**Settings drawer** — lens defaults, motion/glow intensity (and a reduced-
motion override), label density, Live poll cadence, API token. Persisted in
`localStorage`; the server stays stateless about UI preferences.

### Interaction rules (field feedback, 2026-08-07)

Locked after the U0 mockup review — the mockup's card-on-hover made sweeping
the cursor across the field thrash the card panel:

- **Hover** = highlight only: ring + a small name/type tooltip at the cursor.
  Nothing opens, nothing moves elsewhere on screen.
- **Click** = select: the memory card opens **pinned** until dismissed
  (background click / Esc / another selection). Sweeping the mouse can never
  change the selection.
- Focus (neighbors + edges lit) follows selection, not hover.

## Architecture

```
crates/cerebro-api/          # grows: static embed + ~5 new routes
ui-web/                      # vanilla HTML/JS/CSS — NO framework, NO build step
  index.html  app.js  field.js (canvas renderer)  style.css
ui-slint/                    # native mirror (own crate, workspace member)
```

- **Renderer**: hand-rolled Canvas2D with additive blending ("lighter") for
  the glow. No d3, no three.js, no build step — house style (Prefrontal
  ui-web is vanilla). Canvas2D comfortably draws 5–10k glowing sprites;
  WebGL is a later escape hatch, not a day-one dependency.
- **Serving**: `include_str!`/`include_bytes!` embed in `cerebro-api` — the
  single-binary property holds. `GET /` serves the app (bearer token as
  today).
- **Theme**: committed dark (it is a night sky). Palette anchored to the
  house #0d0d0d ground; memory-type hues from a colorblind-safe categorical
  set validated per the dataviz method.

### New API surfaces (the only backend work)

| Route | Purpose | Backing |
|-------|---------|---------|
| `GET /graph/export?scope&cap` | nodes + edges with all visual channels; LOD: top-N by activation + viewport expand | `list_memories_scoped` + `list_all_links` |
| `GET /graph/layout` | cached 2D projection of embeddings; recompute on demand / post-dream | new `layout` table; PCA (~50 lines, no new dep) |
| `POST /recall/trace` | recall + per-hop spread trace (seeds, walked edges with amounts, ranking) | extend `spread_traced` to return `Vec<TraceStep {hop, src, dst, amount}>` — additive, existing callers ignore it |
| `GET /events?since` | SSE: new audit rows as cortex events (remember/recall/dream) | tail `audit_log` (it already records every tool call) |
| `GET /dream/reports` | dream report list for the observatory timeline | `save_dream_report` table (list variant of `get_last_dream_report`) |

### Slint native (`ui-slint`)

Reading-surface mirror, Prefrontal charter-D4 style: dashboard panels +
**Atlas and Thought lenses only** (simplified field — few hundred brightest
stars, `Path`-drawn edges, timer-driven ripple). Dream observatory and Health
stay web-only until wanted. Same palette, same JSON API, bearer token from
env/config.

## Build order

| Step | Scope | Gate |
|------|-------|------|
| U0 | this charter | merged |
| U1 | `/graph/export` + `/graph/layout` + ui-web shell with Atlas lens (pan/zoom/hover/select/search) | field renders a real brain from a real DB — **shipped 2026-08-08; gate passed against the dev brain (369 memories / 5,885 links)** |
| U1b | settings drawer + memory CRUD + link-drawing (instruments over existing routes) | a memory created, edited, and trashed from the UI round-trips — **shipped 2026-08-08; the round-trip's first run caught the Python ghost-FK (see field notes)** |
| U2 | trace-carrying recall + Thought lens ripple | a typed query animates its real spread — **shipped 2026-08-08; and the lens's first real query exposed the seed-cap spread no-op (see field notes)** |
| U3 | SSE audit tail + Live lens glow | an MCP `remember` from another terminal ripples in ≤2s — **shipped 2026-08-08; gate passed (stdio `cerebro-mcp` remember → SSE tap in the same poll window); found the axum-0.7 brace-route 404 and the stale dev-MCP binary along the way** |
| U4 | Dream observatory + `/dream/reports` | a real dream cycle scrubbable |
| U5 | `ui-slint` mirror (dashboard + Atlas + Thought) | native app browses the same DB |
| U6 | Health lens + time-lapse + polish | watchdog metrics visible in-field |

Each step is one PR, tests riding. U1 is the bulk; everything after is
additive.

## U1 field notes (2026-08-08)

- **Edge LOD is not optional.** The dev brain averages 31 links/node (auto-link
  + Hebbian encoding); drawing all of them at overview zoom IS the forbidden
  hairball. Shipped rule: edges ranked by effective weight once at boot; at
  overview zoom only the strongest ~8% draw (floor 400), 35% at mid zoom, all
  when zoomed in — with alpha additionally dimmed by draw count. Health lens
  exempts cold links (they are the diagnosis).
- **Backfill is the layout's prerequisite.** Only embedded memories get PCA
  coords; the dev brain had 37/369 until `cerebro backfill` embedded the rest.
  Un-embedded memories render honestly on an outer rim, never faked into the map.
- **Deep links:** `?q=…` boot-runs a recall, `?lens=health` opens a lens,
  `?agent=NAME` scopes, `?token=…` is consumed into sessionStorage and
  scrubbed from the URL.
- The 6-type legend maps `affective` to the teal slot (the mockup's "message"
  — messages are affective memories with a `message` tag).

## U2 field notes (2026-08-08)

- **The lens found a real bug on its first query.** `/recall/trace` returned
  zero events on the dev brain: `SPREADING_MAX_ACTIVATED` (50) counted the
  seeds, and recall over-fetches `k*5 = 50` candidates — so on any store
  returning a full page, the spread broke before hop 1 and spreading
  activation was silently a no-op. Python has the identical flaw
  (spreading.py:155); ours is now a documented deviation — the budget bounds
  growth beyond the seeds. Post-fix, the same query: 75 walks, 100 activated,
  top score 0.415 → 0.62 (association scores finally contribute). This is
  also why `never_traversed_links_pct` sat at exactly 100.0 for the colony.
- A traced recall is a REAL recall: same reinforcement (ACT-R/FSRS on the
  top-k, walk-stamping on used links). Watching a thought is thinking it —
  the meta line says "reinforced" so nobody is surprised.
- Trace events whose endpoints fall outside the export cap are dropped
  client-side; the meta line keeps the honest totals.

## U3 field notes (2026-08-08)

- **The EEG runs in every lens** — the stream connects at boot; a mutation
  pulses on the field wherever you are. The ticker panel is Live-only.
  Replay: `?since=<audit rowid>` fetches history as plain JSON
  (`GET /audit/since/{id}` — the REST surface's first audit read) before the
  `EventSource` opens; `?es=off` skips the stream (captures, demos).
- **Mutations only, by design**: reads deliberately don't audit (wave-3
  decision), so the EEG shows writes — remember, update, delete, associate,
  session_save, dream_run, the lot (28 actions). If recall-visibility is ever
  wanted, that's a deliberate audit-contract change, not a UI patch.
- **The axum find**: the whole cerebro-api router was written in 0.8 brace
  syntax while the workspace pinned 0.7 — every parameterized route
  (`/memory/{id}`, episodes, graph, trash…) was a silent 404 since the crate
  was born; the U1 card's full-body fetch was quietly falling back to the
  content head. Fixed by the 0.8 bump; `parameterized_routes_resolve` pins it.
- **There are two brains** (correction, same day: the U3 gate-run diagnosis
  said "stale binary, no audit writes" — wrong skull). The dev MCP runs with
  `CEREBRO_DATA_DIR=~/Projects/CerebroCortex/data` — the REAL FORGE brain
  (703 memories, 330 audit rows, fully embedded, auditing fine on its wave-5
  binary; it only lacked W-A + the spread fix until the 2026-08-08 rebuild).
  What the U1–U3 gate runs observed was `~/.cerebro-cortex/`, the DEFAULT
  data dir — a May-era snapshot brain. Lucida against the real brain:
  `CEREBRO_DATA_DIR=~/Projects/CerebroCortex/data cerebro-api`. A data-dir
  indicator in the UI header is U1b material (you should always know which
  skull you're inside).
- Headless-capture lesson: an open `EventSource` pins chromium's virtual-time
  forever — hence `?es=off` + REST replay for deterministic screenshots.

## U1b field notes (2026-08-08)

- **The instruments caught a live data bug within the hour.** The CRUD
  round-trip's first PUT failed "no such table: main.memory_nodes" — Python-
  migrated DBs (BOTH local brains, and every colony node) carry the Python-era
  `memory_versions` whose FK references the ghost `memory_nodes`, because
  SCHEMA_SQL's IF NOT EXISTS skipped the existing table. Latent until W-A's
  R-04 snapshots and R-06 purge cleanup started writing there. Fixed with
  `repair_ghost_fk_memory_versions` on every `SqliteStore::open()`
  (row-preserving rebuild, idempotent probe); queued HIGH for ApexOS.
- **API mutations now audit** (remember/update/delete/restore/purge/associate/
  bulk) — what you do in the observatory shows in its own EEG. Mirrors the
  MCP discipline: best-effort, never fails the call it records.
- API `POST /remember` + `PUT /memory/{id}` accept `visibility` (MCP-twin
  contract: strict parse, orphan guard); PUT passes the caller's scope as
  `edited_by`; the R-08 graph-eviction wrappers now back delete/restore/
  purge/bulk (deleted nodes stop spreading immediately, no restart needed).
- Settings live in `localStorage` (`lucida.settings`); the server stays
  stateless about UI preferences, as chartered. `?open=settings|compose`
  deep-links a drawer.

### Settings-drawer backlog for U1b (surfaced by U1+U2+U3, as predicted)

- Ripple speed (`HOP_MS`, currently 950ms) + replay behavior
- Edge-LOD thresholds (overview share, zoom breakpoints, alpha dim)
- Motion: twinkle on/off (beyond `prefers-reduced-motion`), glow floor
- Recall `top_k` for the query bar (currently 12)
- Default lens on load; default agent scope
- Layout recompute button (POST /graph/layout) — post-dream refresh
- Live: poll cadence (`?poll_ms` exists server-side), ticker depth (40),
  pause/resume stream, agent filter, born-since-load auto-refresh threshold

## Out-of-scope (recorded so we don't drift)

- No force-directed layout, ever, for the main field (focus-mode local
  ego-graph MAY use a tiny local relax).
- No frontend framework, no npm build step in this repo.
- No writes from the Slint app beyond what the web app can do (same API).
- 3D — a depth *cue* (parallax) yes; free-orbit 3D no. It demos well and
  reads badly.
