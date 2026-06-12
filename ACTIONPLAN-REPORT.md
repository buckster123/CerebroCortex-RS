# CerebroCortex-RS — Audit Findings & Action Plan

> Deep parity/correctness audit of the Rust port vs the Python reference.
> Methodology & coverage map: [AUDIT-PLAN.md](AUDIT-PLAN.md).
> Each finding is self-contained — a fresh human or worker-brain Claude can pick up any
> single ID and execute it without further context.

- **Subject**: `CerebroCortex-RS` @ `c00ed86` · **Reference**: `../CerebroCortex` (Python)
- **Auditor**: Opus 4.8 · **Date**: 2026-06-12
- **Verdict**: **Solid port, ships and passes 118 tests, but not yet at parity.** One core
  algorithm (spreading activation) silently diverges from Python, and the daemon lacks the
  per-call crash isolation Python has. No data-loss or injection bugs found. The gaps are
  well-contained and individually fixable.

## Resolution log — 2026-06-12 (Opus 4.8, post-audit fix session)

Fixed and pushed to `main` (commits after `c00ed86`):

| ID | Status | Notes |
|----|--------|-------|
| C-RS-001 | ✅ Fixed | `spread()` re-ported faithfully (seed weights, undirected BFS, sublinear accumulation, normalisation). **+5th divergence found & fixed**: was outgoing-only, Python is undirected (`mode="all"`). Verified by 7-graph fixture suite generated from the real Python `spreading_activation`, matched within 1e-4. |
| C-RS-002 | ✅ Fixed | Per-call `tokio::spawn` panic boundary; a panicking handler → `-32603`, daemon keeps serving. Test trips a panic then succeeds on the next call. |
| C-RS-003 | ✅ Fixed | Real scope-visibility map via new `SqliteStore::get_visibility_meta`; global scope short-circuits. Folded into the C-RS-001 re-port. |
| C-RS-004 | ⚠️ Partial | Pre-phase stale-episode close (`close_stale_episodes`, 24h) ✅; `episodes_consolidated` populated from phase 1 ✅. **Resume-from-completed-phases deferred** (needs a persisted per-cycle phase table + a `cycle_id` on the `dream_run` tool surface). |
| C-RS-005 | ✅ Fixed | `activation_bench` compiles (`retrievability` call updated to the 2-arg API). |
| C-RS-006 | ✅ Fixed | `-32602` for arg-validation, `-32601` for not-implemented, `-32603` for internals; handshake guarded on `method == "initialize"`. |
| C-RS-007 | ✅ Fixed | The 4 deferred Tier-7 tools stay advertised (surface parity = 66) but now return an honest `-32601` not-implemented error instead of a success stub. |
| C-RS-008 | ✅ Fixed | Cross-phase LLM ceiling checks `effective_budget`, not the constant 20. |
| C-RS-009 | ✅ Fixed | `graph_neighbors` now scope-filters; `stats`/`dream_status` unused agent param dropped (global). |
| C-RS-010 | ✅ Fixed | `associate` validates both endpoints exist before writing — no more dangling orphan link rows. |
| C-RS-011 | ⏸️ Deferred (documented) | MCP `resources`/`prompts` surfaces. Capabilities are advertised honestly (not claimed), so no broken promise. Port the 3 prompts only if an ApexOS consumer needs them — revisit in the integration pass. |
| C-RS-012 | ✅ Fixed | "63 tools" drift reconciled to 66 (62 functional + 4 deferred) across doc, comment, and the renamed list test. |
| C-RS-013 | ✅ Fixed | `cargo clippy --workspace --all-targets` and `cargo build --workspace` both warning-free. |
| C-RS-014 | ⏸️ Deferred (documented) | Recall wire-shape vs consumers — a verification task explicitly scoped to the ApexOS-RS integration pass (out of scope this session, per the locked port-internals-only scope). |

**Net**: 10 of 14 fully fixed, 1 partial (C-RS-004), 2 deliberate deferrals (C-RS-011, C-RS-014) tied to the ApexOS integration pass. Test count 118 → 126; clippy 37 warnings + bench error → 0.

## Summary by severity

| Severity | Count | IDs |
|----------|-------|-----|
| Critical | 0 | — |
| High | 2 | C-RS-001, C-RS-002 |
| Medium | 5 | C-RS-003, C-RS-004, C-RS-005, C-RS-006, C-RS-007 |
| Low | 5 | C-RS-008, C-RS-009, C-RS-010, C-RS-011, C-RS-014 |
| Nit | 2 | C-RS-012, C-RS-013 |

## Execution waves

- **Wave 1 — Correctness & resilience (do first):** C-RS-001, C-RS-002, C-RS-003
- **Wave 2 — Parity & protocol:** C-RS-004, C-RS-006, C-RS-007, C-RS-011, C-RS-014
- **Wave 3 — Robustness & hygiene:** C-RS-005, C-RS-008, C-RS-009, C-RS-010
- **Wave 4 — Docs & lint:** C-RS-012, C-RS-013

---

## Wave 1 — Correctness & resilience

### C-RS-001 · [High] · Dim 2 · Spreading activation diverges from Python (and falsely claims it doesn't)
- **Location**: `crates/cerebro/src/activation/spreading.rs:12-58` vs `../CerebroCortex/src/cerebro/activation/spreading.py:102-236`
- **Evidence**: The RS doc comment says *"Mirrors Python spreading.py exactly."* It does not. Concrete divergences:
  1. **Seed weighting** — Python seeds activation with per-seed similarity (`seed_weights`, the vector scores). RS seeds **every** node at `1.0` (`seeds.iter().map(|&n| (n, 1.0, 0))`, line 31). Vector similarity is discarded from the spread.
  2. **Accumulation** — Python uses sublinear accumulation for already-activated nodes: `activated[id] = max(existing, existing + spread*0.5)` (py:217). RS takes a plain `a.max(activation)` (rs:39) — no diminishing-returns term.
  3. **Normalization** — Python normalizes final activations to `[0,1]` by dividing by max (py:230-234). RS returns **un-normalized** scores, which then feed `recall_score` (`activation/mod.rs:49`) on a different scale than Python.
  4. **Traversal** — Python is BFS hop-by-hop with `hop_decay = decay^(hop+1)` (py:158). RS is a LIFO stack (DFS) with compounding `activation * hop_decay * w` (rs:54). Different visitation order interacts with the 50-node cap to include different nodes.
- **Impact**: `recall()` ranking and scores differ from Python for any query that triggers spreading (i.e. any query hitting a connected memory). The port's stated gate is "values match Python within 1e-4" — spreading violates it. Silent: no error, just different ordering.
- **Recommended fix**: Re-port `spreading_activation` faithfully: thread the real seed weights from `cortex.rs:128` (the `sims_map`) into `spread()` instead of `1.0`; switch to BFS with `decay^(hop+1)`; add the `+ spread*0.5` sublinear accumulation; normalize to `[0,1]` before returning. Then add a fixture test (see C-RS-001-test below). Remove/qualify the "exactly" comment.
- **Verification**: Add a fixture generated from Python (`scripts/gen_activation_fixtures.py` pattern) for a small fixed graph; assert RS spread output matches within `1e-4`. Re-run `cargo test -p cerebro`.
- **Effort**: M (~half day incl. fixture).

### C-RS-002 · [High] · Dim 6 · No panic isolation in the MCP dispatch loop — one bad call crashes the daemon
- **Location**: `crates/cerebro-mcp/src/main.rs:35-58`; handler panics reachable via `crates/cerebro/src/{storage/sqlite.rs, engines/dream.rs, storage/vector.rs, engines/thalamus.rs, engines/temporal.rs}` (9 non-test `unwrap/expect/panic`/index sites).
- **Evidence**: `dispatch_tool(...)` is `.await`ed directly in the loop (`main.rs:52`) and `transport.write(&resp).await?` propagates with `?`. There is no `catch_unwind` and no per-request task boundary. Any panic in a handler or downstream engine unwinds through `main` and the process exits. Python's equivalent (`mcp_server.py:993` `call_tool`) wraps **every** handler in `try/except` and returns an error `TextContent`, so a single bad call degrades to an error response — the server stays up.
- **Impact**: A malformed tool call (or an unanticipated downstream panic — e.g. a slice index, a `serde` `unwrap`, a sqlite edge case) takes down the **entire shared memory subsystem** for agentd. This is a long-running daemon multiple agents depend on; availability matters.
- **Recommended fix**: Wrap the per-call dispatch in `tokio::spawn` + `JoinHandle` (catches unwind → return a JSON-RPC `-32603` error) **or** wrap `route()`'s body in `std::panic::catch_unwind` via `futures::FutureExt::catch_unwind` (requires `UnwindSafe`). On caught panic, log to stderr and return a proper error response echoing the request id. Independently, audit the 9 downstream `unwrap()`s and convert request-path ones to `?`/`anyhow`.
- **Verification**: Add a dispatch test that sends args designed to trip a downstream `unwrap` and assert the loop returns an error object and continues (reads a second message successfully).
- **Effort**: M.

### C-RS-003 · [Medium] · Dim 8 · Spreading activation does **zero** scope filtering (param is dead)
- **Location**: `crates/cerebro/src/cortex.rs:122-125` and `crates/cerebro/src/activation/spreading.rs:15-20,48`
- **Evidence**: `recall()` builds `visible_nodes` as **every** graph node mapped to `true` (cortex.rs:122-124), and passes a `scope` into `spread()` that the function **never reads** (the `scope` parameter is unused inside `spreading.rs`). Visibility is therefore enforced only by the final `get_memories_by_ids(&all_ids, &scope)` (cortex.rs:143). Python instead builds a real `_build_visibility_cache` and calls `_check_access` per neighbor (py:78-99,188), enforcing shared/private/thread during the spread.
- **Impact**: No **output** leak (the final SQLite filter still removes non-visible nodes), but another agent's private/thread memories participate in the spread and influence the activation scores of nodes that *are* returned — so a query's ranking can be shaped by memories the caller can't see. Divergence from Python and a subtle cross-agent coupling.
- **Recommended fix**: Fold into the C-RS-001 re-port: have `spread()` consult per-node `(agent_id, visibility, thread)` and apply Python's `_check_access` rules, or have `cortex.rs` populate `visible_nodes` with a real visibility computation instead of all-`true`. Remove the dead `scope` param or actually use it.
- **Verification**: Test: two agents, private memories linked across; assert agent A's recall scores are identical whether or not agent B's private linked node exists.
- **Effort**: S–M (largely subsumed by C-RS-001).

---

## Wave 2 — Parity & protocol

### C-RS-004 · [Medium] · Dim 3 · Dream engine missing Python features (resume, pre-phase cleanup, episode count)
- **Location**: `crates/cerebro/src/engines/dream.rs:85-140` vs `../CerebroCortex/src/cerebro/engines/dream.py:204-258`
- **Evidence**: All 6 phases are present and run **sequentially with per-phase error catching** (good). Missing vs Python:
  1. **Resume** — Python checks `get_completed_phases(cycle_id)` and skips already-done phases (py:230-256). RS always runs all 6 fresh.
  2. **Pre-phase cleanup** — Python auto-closes stale episodes before phase 1 (py:239-242). RS does not.
  3. **`episodes_consolidated`** is hardcoded `0` (dream.rs:119) — the report field is never populated.
- **Impact**: Dream cycles aren't resumable after interruption (re-does LLM work, re-spends API budget), stale episodes accumulate, and the report under-reports activity. Not a correctness bug in a single cycle; a robustness/parity gap across cycles.
- **Recommended fix**: Add a `completed_phases` check keyed by a persisted `cycle_id`; add a pre-phase stale-episode close (hippocampus already has episode logic); populate `episodes_consolidated` from phase 1.
- **Verification**: Run two `dream_run` calls; assert the second skips completed phases (needs a persisted cycle marker). Assert `episodes_consolidated > 0` when episodes exist.
- **Effort**: M.

### C-RS-006 · [Medium] · Dim 5 · MCP error/handshake conformance is coarse
- **Location**: `crates/cerebro-mcp/src/dispatch.rs:50-54` and `crates/cerebro-mcp/src/main.rs:30-32`
- **Evidence**: (a) **Every** tool error returns `-32603` (Internal error), including pure argument-validation failures (e.g. `"content is required"`, dispatch.rs:74) which per JSON-RPC should be `-32602` (Invalid params). (b) The handshake reads the first message and calls `handle_initialize` **unconditionally** (main.rs:30-31) without checking `method == "initialize"` or validating params — a non-initialize first message still gets an init response.
- **Impact**: Clients can't distinguish "you sent bad args" from "the server broke." Handshake is brittle to non-conforming clients. Low functional risk with agentd today, but it's protocol drift.
- **Recommended fix**: Map validation errors to `-32602` (introduce a typed error or inspect the message), keep `-32603` for genuine internals. Guard the handshake on `init_req["method"] == "initialize"`, else return `method_not_found`.
- **Verification**: Dispatch test asserting a missing-required-arg call returns `-32602`.
- **Effort**: S.

### C-RS-007 · [Medium] · Dim 1 · Four tools are advertised but stubbed — clients get a non-error "not_yet_implemented"
- **Location**: `crates/cerebro-mcp/src/dispatch.rs:921` (fallthrough) and `crates/cerebro-mcp/src/tools.rs:831-835` (generic `(stub)` schema)
- **Evidence**: `ingest_file`, `describe_image`, `search_vision`, `cognitive_bootstrap` appear in `tools/list` with a generic `(stub) <name>` description and empty schema, and return `{"status":"not_yet_implemented"}` as a **success** result (not an MCP error). The entire Python `ingestion/` + vision subsystem sits behind these.
- **Impact**: An agent calling these gets a 200-style stub blob it may treat as success. Discoverable-but-broken tools are worse than absent ones.
- **Recommended fix**: Decide per tool — either (a) implement, or (b) **hide** them from `tools/list` until implemented (filter `TOOL_NAMES`), or (c) return a proper MCP error (`-32601`/`-32603` with a clear "not implemented" message) instead of a success payload. Recommended interim: (b) hide + keep a `DEFERRED_TOOLS` list, documented.
- **Verification**: `tools/list` returns 62 (if hidden) and any direct call returns an error, not a success stub.
- **Effort**: S.

### C-RS-011 · [Low] · Dim 1 · MCP `resources` and `prompts` surfaces not ported
- **Location**: `crates/cerebro-mcp/src/main.rs:50-54` vs `../CerebroCortex/src/cerebro/interfaces/mcp_server.py:2472-2574`
- **Evidence**: Python exposes `list_resources`, `list_resource_templates`, `read_resource`, `list_prompts`, `get_prompt` (3 prompts: `session_handoff`, `memory_review`, `context_briefing`). RS handles only `tools/list` and `tools/call`; everything else → `method_not_found`. RS correctly does **not** advertise resource/prompt capabilities in `initialize` (dispatch.rs:20), so conforming clients won't call them.
- **Impact**: Low — capabilities are advertised honestly, so no broken promise. But any consumer using Python's prompts/resources loses them.
- **Recommended fix**: Document as a deliberate deferral, or port the 3 prompts + resource templates if a consumer needs them. Confirm agentd doesn't rely on them (deferred integration pass).
- **Effort**: S (doc) / M (port).

### C-RS-014 · [Low] · Dim 1 · Recall wire-shape parity unverified vs consumers
- **Location**: `crates/cerebro-mcp/src/dispatch.rs:92-95` vs Python `_handle_recall`
- **Evidence**: RS returns each recall hit as `{"memory": <node>, "score": <f32>}` serialized as JSON text. Python formats its own `TextContent`. The exact field shape consumers parse wasn't compared (agentd integration is out of scope this pass).
- **Impact**: If any consumer parses specific fields, shape drift could break it silently.
- **Recommended fix**: During the ApexOS-RS integration pass, diff RS tool outputs against Python for the tools agentd actually parses; align shapes.
- **Effort**: S (verification task).

---

## Wave 3 — Robustness & hygiene

### C-RS-005 · [Medium] · Dim 9/10 · `activation_bench` benchmark fails to compile
- **Location**: `crates/cerebro/benches/activation_bench.rs`
- **Evidence**: `cargo build --benches -p cerebro` → `E0061` (function takes 2 args but 3 supplied) and `E0308` (mismatched types). The bench is stale against the current activation API. This also makes `cargo clippy --all-targets` report an error.
- **Impact**: No performance baseline exists; `cargo bench` is broken in-tree; CI using `--all-targets` would fail.
- **Recommended fix**: Update the bench calls to the current `activation::` signatures (compare against `activation/mod.rs` + `fsrs.rs` + `spreading.rs`). If perf isn't tracked yet, at minimum make it compile.
- **Verification**: `cargo build --benches -p cerebro` clean; optionally `cargo bench` runs.
- **Effort**: S.

### C-RS-008 · [Low] · Dim 3 · `max_llm_calls < 20` is not honored as a global cap
- **Location**: `crates/cerebro/src/engines/dream.rs:93-108,237,332,556`
- **Evidence**: `effective_budget = max_llm_calls.min(20)` is computed, but the cross-phase ceiling check is `*calls_used >= MAX_LLM_CALLS` (the constant **20**), not `>= effective_budget`. Per-phase budgets are `effective_budget.min(12/4/4)`. So a request of e.g. `max_llm_calls=5` can still spend up to `5 + 4 + 4 = 13` calls across phases.
- **Impact**: Real API spend can exceed an explicit smaller request. In practice callers use the default 20 (where 12+4+4=20 binds exactly), so impact is low today.
- **Recommended fix**: Change the three break checks to `*calls_used >= effective_budget`.
- **Verification**: Unit test with a mocked/counted LLM and `max_llm_calls=5` asserting ≤5 calls.
- **Effort**: S.

### C-RS-009 · [Low] · Dim 8 · Three `cerebro-api` routes parse an agent scope they then ignore
- **Location**: `crates/cerebro-api/src/main.rs:216-222` (`stats`), `:514-522` (`graph_neighbors`), `:821-828` (`dream_status`)
- **Evidence**: Each takes `Query(q): Query<AgentQuery>` but never uses `q` (these are 3 of the build's 5 warnings). `stats` and `dream_status` are global anyway (low impact), but `graph_neighbors` returns a memory's neighbors **regardless of agent scope**.
- **Impact**: `graph_neighbors` ignores `agent_id` — neighbor IDs of any memory are returned to any caller. IDs only (not content), so limited, but it's an inconsistent scope story vs the recall routes which honor `scope_from(q.agent_id)`.
- **Recommended fix**: Either apply the scope (filter neighbors by visibility) or drop the param and document these as global endpoints. Prefix genuinely-unused params with `_` only after deciding intent.
- **Verification**: `cargo build` warning-free for these; a scoped `graph_neighbors` test if scope is applied.
- **Effort**: S.

### C-RS-010 · [Low] · Dim 4 · `associate` does not validate that endpoints exist
- **Location**: `crates/cerebro/src/cortex.rs:161-173`, `crates/cerebro-mcp/src/dispatch.rs:98-111`
- **Evidence**: `associate()` inserts the link into SQLite first, then `graph.add_edge` (which logs a warning if an endpoint node is missing). No prior existence check on `source_id`/`target_id`.
- **Impact**: A link with a typo'd/nonexistent endpoint persists in `links` as a dangling row (graph silently skips the edge). Recall won't traverse it, but the DB carries orphan links.
- **Recommended fix**: Check both IDs exist (and are visible to the scope) before insert; return a clear error otherwise. Mirror Python's behavior here.
- **Verification**: Test that `associate` with a bogus target returns an error and inserts no link row.
- **Effort**: S.

---

## Wave 4 — Docs & lint

### C-RS-012 · [Nit] · Dim 11 · "63 tools" drift (actual: 66 = 62 functional + 4 stubs)
- **Location**: `crates/cerebro-mcp/src/tools.rs:839` (comment "All 63 tool names"), `crates/cerebro-mcp/src/main.rs:13` (doc "all 63 ... tools"), test name `dispatch::tests::tools_list_echoes_id_and_contains_63_tools` (`dispatch.rs:~970`), `CLAUDE.md` ("62/66 wired").
- **Evidence**: `TOOL_NAMES` actually holds 66 (verified equal to Python's `TOOL_SCHEMAS`). The "63" predates later wiring.
- **Recommended fix**: Reconcile all references to **66 advertised (62 functional + 4 stubs)**. Rename the test; fix the two comments.
- **Effort**: XS.

### C-RS-013 · [Nit] · Dim 10/11 · 37 clippy warnings (incl. 3 `excessive_precision`)
- **Location**: workspace-wide; `cargo clippy --all-targets`
- **Evidence**: Top lints: `new_without_default` ×9 (all engine structs), `excessive_precision` ×3, `manual_range_contains` ×2, `unnecessary_map_or` ×2, `map_clone`, `manual_repeat_n`, `needless_question_mark`, `missing_transmute_annotations` (the sqlite-vec FFI cast — benign), unused imports/vars. Plus the 5 `rustc` warnings from `build`.
- **Evidence note**: The `excessive_precision` floats are worth a glance — confirm the literals still match Python constants within 1e-4 (the fixture tests pass, so likely cosmetic, but verify the 3 sites).
- **Recommended fix**: `cargo clippy --fix` for the mechanical ones; add `Default` impls (or `#[derive(Default)]`) to the engine structs; eyeball the 3 precision literals against Python `config.py`.
- **Verification**: `cargo clippy --workspace --all-targets` clean (after C-RS-005 fixes the bench).
- **Effort**: S.

---

## Positives worth recording (so they aren't re-litigated)

- **FSRS & ACT-R math is faithful** and well-tested (`fsrs.rs`, `actr.rs`, `mod.rs`) — matches Python within tested tolerances.
- **No SQL injection** — all 16 `format!`-built queries in `sqlite.rs` interpolate only internal fragments (`SELECT_COLS`, `scope_sql` which itself uses `?`); all values bind via `?`/`dyn_params`.
- **MCP stdout sanctity** holds — `tracing` goes to stderr (`main.rs:19`); no stray `println!` on the JSON-RPC stream.
- **Python→Rust DB migration** is thorough (Unix-float→RFC3339 via `strftime`, old tables preserved as `_py_*`, idempotent via `schema_version=100`) and has 2 passing tests.
- **Dream phases** run sequentially with per-phase error→`PhaseResult::failed` catching — a failing phase doesn't abort the cycle.
- **Thalamus gating** (`MIN_CONTENT_LENGTH=10`, salience bands 200/30) matches Python exactly.
- **118 tests green, build clean.** The port is genuinely close.

## Suggested order of attack (TL;DR for a worker-brain)
1. **C-RS-001 + C-RS-003** together — re-port `spread()` faithfully with seed weights, BFS, normalization, and real scope filtering; add a Python-derived fixture. *This is the headline fix.*
2. **C-RS-002** — wrap dispatch in panic isolation so one bad call can't kill the daemon.
3. **C-RS-007 + C-RS-006** — make stubs honest (hide or error) and split `-32602`/`-32603`.
4. **C-RS-004** — dream resume + pre-phase cleanup + `episodes_consolidated`.
5. Mop up Wave 3 + Wave 4 with `clippy --fix` and the small targeted edits.
