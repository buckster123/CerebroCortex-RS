# CerebroCortex-RS — Agent & Developer Guide

> Pure-Rust port of CerebroCortex. Same 63 MCP tools, same wire format, zero Python runtime.
> Drop-in for ApexOS `plugins.toml`. Pi 5 native. Single binary.

See also: [ARCHITECTURE.md](ARCHITECTURE.md) | [CONTRIBUTING.md](CONTRIBUTING.md)

---

## What this is

`cerebro-mcp` is a Cargo workspace of four crates:

```
crates/
  cerebro/        # library — all cognitive logic (types, models, activation, engines, storage)
  cerebro-mcp/    # MCP-over-stdio binary (63 tools) — the ApexOS drop-in
  cerebro-api/    # axum REST API + dashboard (optional, mirrors Python cerebro-api)
  cerebro-cli/    # clap CLI (mirrors Python cerebro CLI)
```

Full design and build order: [ARCHITECTURE.md](ARCHITECTURE.md).
Reference implementation: `../CerebroCortex` (Python — **do NOT modify**).

---

## Locked decisions

- **Language**: Rust, workspace of 4 crates
- **Storage**: single SQLite file — sqlite-vec (ANN), FTS5 (keyword fallback), petgraph (in-memory graph rebuilt from SQLite on init)
- **Embeddings**: fastembed (ONNX Runtime, 384-dim, ~33MB model, no GPU required)
- **MCP**: hand-rolled newline-delimited JSON-RPC over stdio, protocol `"2024-11-05"` — no SDK dependency
- **LLM (dream engine)**: reqwest → Anthropic API directly, same client pattern as agentd
- **Python original**: stays untouched until this port passes the full test suite

---

## Pi 5 target

| Detail | Value |
|--------|-------|
| SSH | `ssh apexos@192.168.0.158` (LAN only, pw: `abnudc1337`) |
| OS | Debian trixie headless |
| Storage | NVMe `/dev/sda2`, 458 GB |
| RAM | 8 GB |
| Binary | `/usr/local/bin/cerebro-mcp` |
| Data dir | `/var/lib/cerebro/` (`CEREBRO_DATA_DIR=/var/lib/cerebro`) |
| Service | `/etc/systemd/system/cerebro.service` (from `deploy/cerebro.service`) |
| Env file | `/etc/cerebro/env` — plain `KEY=VALUE`, no `export`, chmod 600 root-owned |

**Always build on the Pi — never cross-compile.**
The Pi is Cortex-A76 (arm64). An x86 binary gives "Exec format error". Pi 5 builds in ~2 min.

---

## Deploy workflow (commit → push → Pi)

```bash
# 1. Dev machine — code passes tests, then:
cargo test
git add -p
git commit -m "short imperative description"
git push

# 2. On Pi
cd ~/CerebroCortex-RS
git pull
cargo build --release -p cerebro-mcp

# 3. Hot-swap the binary (running binary = "text file busy" — always stop first)
sudo systemctl stop cerebro-mcp
sudo cp target/release/cerebro-mcp /usr/local/bin/cerebro-mcp
sudo systemctl start cerebro-mcp

# 4. Verify
sudo journalctl -u cerebro-mcp -n 20 --no-pager
```

**One-liner for a code-only change:**
```bash
sudo systemctl stop cerebro-mcp && \
cargo build --release -p cerebro-mcp && \
sudo cp target/release/cerebro-mcp /usr/local/bin/cerebro-mcp && \
sudo systemctl start cerebro-mcp && \
sudo journalctl -u cerebro-mcp -n 10 --no-pager
```

---

## ApexOS integration (the drop-in)

When `cerebro-mcp` is ready, one line in `/etc/agentd/plugins.toml` on the Pi:
```toml
[[plugin]]
id      = "cerebro"
cmd     = "/usr/local/bin/cerebro-mcp"   # was: python -m cerebrocortex.mcp
restart = "always"
```
`sudo systemctl reload agentd` (or `hot_reload_subsystem plugins` via the agent).
agentd never knows. Same 63 tools. Same MCP contract.

---

## Environment variables

| Var | Default | Purpose |
|-----|---------|---------|
| `CEREBRO_DATA_DIR` | `~/.cerebro-cortex/` | SQLite DB + exports root |
| `CEREBRO_EMBED_MODEL` | `BAAI/bge-small-en-v1.5` | fastembed model ID |
| `ANTHROPIC_API_KEY` | — | Required for dream engine LLM phases |
| `SQLITE_VEC_PATH` | system default | Path to `sqlite-vec` `.so` (TBD on Pi — `apt-cache show sqlite-vec`) |
| `RUST_LOG` | `info` | tracing filter — logs go to stderr, stdout is MCP JSON-RPC |
| `CEREBRO_VISION_BACKEND` | `auto` | describe_image VLM: `auto`\|`ollama`\|`anthropic`\|`off` (+ `_URL`/`_MODEL` for Ollama) |
| `CEREBRO_VISION_EMBED` | follows embed model | CLIP visual recall on/off (`search_vision`) |
| `CEREBRO_RETAIN_VERSIONS` / `_DREAM_REPORTS` / `_AUDIT_ROWS` | 10/90/20000 | retention caps, dream pre-phase sweep; 0 = keep forever |

---

## Build order (current progress)

| Step | Module | Gate | Status |
|------|--------|------|--------|
| 1 | `types.rs` + `models/` | Serde round-trips | ✓ 6 type tests pass |
| 2 | `activation/` | Values match Python fixtures within 1e-4 | ✓ 41 fixture tests pass |
| 3 | `storage/sqlite.rs` | Schema init, CRUD, scope filtering | ✓ 9 storage tests pass |
| 4 | `storage/vector.rs` | sqlite-vec loads, cosine search, FTS5 fallback | ✓ 7 vector tests pass |
| 5 | `storage/graph.rs` | petgraph rebuild + neighbor traversal | ✓ 5 graph tests pass |
| 6 | `engines/` (thalamus→neocortex) | All 8 deterministic engines pass | ✓ 34 engine unit tests pass |
| 7 | `cortex.rs` | `remember()` + `recall()` end-to-end | ✓ 6 cortex pipeline tests pass |
| 8 | `cerebro-mcp/` (core tools) | MCP handshake + remember/recall vs agentd | ✓ 8 dispatch tests pass |
| 9 | Remaining 61 MCP tools | Full tool surface | ✓ 66/67 wired (only `ingest_file` stubbed) |
| 10 | `engines/dream.rs` | All phases, live LLM calls | ✓ 8 phases (6 base + exo-evolution `variation`/`skill_competition`) + dream_run/dream_status wired |
| 11 | `cerebro-cli/` + `cerebro-api/` | CLI and REST parity | ✓ |
| 12 | DB compatibility | Rust reads a Python-generated `cerebro.db` | ✓ |

## Cerebro agent

All Cerebro MCP calls in this project use agent `FORGE` (agent_id=`"FORGE"`, ⚒, #B7410E).
Pass `agent_id: "FORGE"` to any Cerebro tool that accepts it so memories stay isolated from other projects.

## Cerebro session protocol (mandatory)

**Session START** — always call `session_recall` before diving in:
```
session_recall(query="CerebroCortex-RS build status step progress", agent_id="FORGE")
```

**Session END** — always call `session_save` plus any supporting saves:
```
session_save(session_summary="...", key_discoveries=[...], unfinished_business=[...], agent_id="FORGE", priority="HIGH")
```
Then as needed:
- `store_procedure` — non-obvious implementation patterns, gotchas, workarounds
- `store_intention` — next steps / deferred work (salience 0.8–0.95)
- `create_schema` — architectural insights derived from multiple memories
- `episode_start` / `episode_add_step` / `episode_end` — for multi-step implementation sequences

This feeds the knowledge graph. Dream cycles (`dream_run`) then consolidate across sessions — extracting schemas, strengthening links, surfacing connections. The graph compounds over time; consistent use is the only requirement.

---

**Backport waves 3+4 done (2026-07-23)** — all 45 post-fork ApexOS-RS cerebro commits now reconciled (see BACKPORT-FROM-APEXOS.md; waves 1–4 = PRs #4/#5/#6/#7). Wave 3 (PR #6): per-frame JSON-RPC parse isolation + 32 MiB frame cap, 64 KiB thalamus gate, bare-string arg coercion, memory_store true alias, update_memory visibility/set_agent_id, embed-model fallback, FSRS recall reinforcement (activation_at_risk live), undo_snapshot exclusion, find_relevant_procedures widening + honest empty result, audit-log write path + retention sweep, dream semantic-rediscovery reinforcement, cerebro-api CB-006/012/023/026. Wave 4 (PR #7): describe_image (tiered Ollama→Anthropic VLM, `cerebro::vision`) + search_vision (CLIP ClipVitB32, `vision_embeddings` table, caption/FTS fallback) — tool surface now 67 (66 wired + ingest_file stub); VisibilityScope::shared_only federation scope (recall `visibility:"shared"`, closes the CB-008 deferred clause); find_by_tags exact-tag AND lookup; dream-report span fix; Python orphan-table reap. 226 tests green, clippy-clean.

**Exo-evolution frontier done (2026-06-18)** — mirrored from ApexOS-RS `cerebro/crates/` (PRs #109/#111/#112/#113). The single-node Darwinian skill loop is now complete here too: **E1** niche competition + fitness ledger (`record_procedure_outcome` writes `metadata.outcomes:{successes,failures}`; new algorithmic `skill_competition` dream phase ranks same-topical-tag procedures by Wilson lower-bound, tags the fittest `skill_champion`, decays dominated rivals toward the 0.25 prune floor — novelty-exempt below 2 graded uses); **E2/E2b** the LLM `variation` dream phase (refine underperformers → `dream_mutated`; merge two strong distinct same-niche procedures → `dream_merged`, both inheriting niche tags + `derived_from`, starting un-graded); **champion-aware retrieval** (`find_relevant_procedures`/`cognitive_bootstrap` prefer the crowned procedure via shared `retrieval_rank`). `PRUNE_CANDIDATE_SALIENCE` moved to `config` (shared by dispatch + dream). Dream cycle is now 8 phases. `is_structural_tag` covers the new role markers. 168 tests green, clippy-clean (C-RS-013). See `docs/evolutionary-layer.md` in ApexOS-RS for the design charter.
**Step 12 done (2026-06-10)** — Auto-migration from Python `cerebro.db` in `SqliteStore::open()`. Detects Python schema (`memory_nodes` table present), runs a single-transaction migration: `memory_nodes→memories`, `associative_links→links`, renames+recreates `agents`/`episodes`/`episode_steps`/`audit_log` with Rust column names. Access timestamps converted from Unix floats to RFC3339 via `strftime`. Idempotent (schema_version=100 marks completion). 2 new migration tests; 42 cerebro + 8 mcp = 50 total tests green.
**Step 11 done (2026-06-10)** — `cerebro` (binary) and `cerebro-api` fully implemented. CLI: 9 top-level commands + 8 subcommand groups (episode, session, agents, intention, graph, schema, procedure, dream) covering all core operations. REST API: 40 routes (health/stats, memory CRUD, associate, episodes, sessions, agents, graph, tags, intentions, schemas, procedures, trash lifecycle, threads, dream). StdRng replaces thread_rng in DreamEngine (thread_rng is !Send, broke axum handlers). 116 tests still green.
**Step 10 done (2026-06-10)** — DreamEngine fully implemented: all 6 phases (SWS replay, pattern extraction, schema formation, emotional reprocessing, pruning, REM recombination). LLM phases 2/3/6 skip gracefully when `ANTHROPIC_API_KEY` unset. 3 new SqliteStore methods: `has_link_between`, `save_dream_report`, `get_last_dream_report`. `dream_run` and `dream_status` now wired (62/66 tools). 8 dispatch tests still green; total 116 tests pass.
**Step 9 done (2026-06-10)** — 60/66 tools wired. Added 13 more routes: store/list/resolve_intention, store/list/find_relevant_procedures/record_procedure_outcome, create/list/find_matching_schemas/get_schema_sources, get_memory_versions/restore_version. Added memory_versions table to SCHEMA_SQL + log_memory_version/get_memory_versions_raw/get_version_raw to SqliteStore. Intentions/procedures/schemas use existing MemoryType variants (Prospective/Procedural/Schematic) — no extra tables. Remaining 4 stubs: cognitive_bootstrap/ingest_file/describe_image/search_vision (Tier 7 deferred).
**Step 8 done** — `cerebro-mcp` dispatch wired. Notification handling (skip response for `notifications/*`), id echoing fixed. `remember`, `recall`, `associate`, `get_memory` routes fully wired; full `inputSchema` for all four. 8 dispatch tests. Tool count corrected: 66 (not 63 — Python server has 66 as of 2026-06-10).
**Step 7 done** — `cortex.rs` `remember()` + `recall()` + `associate()` fully wired. Pipeline: thalamus gate → amygdala → temporal → SQLite → vector embed → graph node; recall: FTS5/vec search → spreading activation → bulk SQLite load → prefrontal ranking. 6 new end-to-end tests. Total: 108 tests (68 unit + 40 integration).
**Step 6 done** — all 8 deterministic engines implemented. Pure logic tested: thalamus gating/classify/salience, amygdala valence/arousal, temporal concept extraction/bigrams/enrich_node, association find_path/common_neighbors, prefrontal rank_results, neocortex tag helpers. Hippocampus/cerebellum stubbed (DB-driven, wired in step 7). 34 new unit tests.
**Step 5 done** — `storage/graph.rs`: `rebuild_from_db()` loads all non-deleted memories as nodes and all live-endpoint links as edges. Soft-deleted nodes and links to deleted endpoints are excluded. 5 tests.
**Step 4 done** — `storage/vector.rs`: sqlite-vec auto-extension + FTS5 fallback. fastembed init is graceful (no model download in tests; pass `embed_model=""` to skip).
**Step 3 done** — `storage/sqlite.rs` full CRUD with scope filtering.
**Step 2 done.** Re-generate fixtures if Python source changes:
```bash
/home/andre/Projects/CerebroCortex/venv/bin/python3 scripts/gen_activation_fixtures.py
```

---

## Gotchas

- **MCP stdout is sacred.** All `tracing` output goes to `stderr`. The MCP client reads `stdout` as JSON-RPC — any stray `println!` will corrupt the stream and confuse agentd.
- **`text file busy`** — always `systemctl stop cerebro-mcp` before `cp`. A running binary cannot be overwritten.
- **fastembed first run** — downloads ~33 MB model to the model cache dir on startup. Allow extra time on first deploy; pre-warm with `cerebro stats` or a test `remember`.
- **sqlite-vec path on Pi** — TBD. Confirm: `apt-cache show sqlite-vec` or `find / -name "sqlite_vec.so"`. Set `SQLITE_VEC_PATH` in `/etc/cerebro/env` if non-standard. Falls back to FTS5 automatically if extension fails to load.
- **Debian trixie pip gotcha** (relevant for the Python reference, not Rust): PEP 668 enforced — use a venv. Not a Rust issue but good to know when running `scripts/gen_activation_fixtures.py`.
- **Never modify `../CerebroCortex`** — it's the running daily driver for ApexOS and the reference implementation. The Rust port runs in parallel.
- **Graph is cache, SQLite is truth** — the petgraph in-memory graph is rebuilt from SQLite on startup. Write to SQLite first; graph/vector are derived.
- **Python DB auto-migration** — `SqliteStore::open()` detects a Python `cerebro.db` (by checking for `memory_nodes` table) and migrates it in-place automatically. No user action needed; just point `CEREBRO_DATA_DIR` at the Python DB and start the Rust binary. Migration is marked complete with `schema_version=100`; subsequent opens are no-ops. Python's `access_timestamps_json` (Unix floats) → Rust `access_times` (RFC3339) via `strftime`. Old Python tables renamed to `_py_*` for safety.
- **Python DB column mapping** — `memory_nodes`→`memories`, `associative_links`→`links`, `tags_json`→`tags`, `valence`→`emotional_valence`, `arousal`→`emotional_intensity`, `conversation_thread`→`thread_id`, `stability`→`fsrs_stability`, `difficulty`→`fsrs_difficulty`, `metadata_json`→`metadata`, `last_accessed_at`→`updated_at`.

---

## Git discipline

- **Tests pass → commit immediately.** Each build-order step = at minimum one commit.
- **Commit format:** imperative, lowercase. `implement sqlite schema and crud operations`
- **Push after every commit.** No manual trigger needed.
- **Never amend a pushed commit. Never force-push.**
- **Docs travel with code.** Update CLAUDE.md and ARCHITECTURE.md in the same commit as the code they describe.

---

## Deferred / TBD

- `sqlite-vec` `.so` path on Pi — confirm before writing `storage/vector.rs`
- `fastembed` model cache location on Pi — confirm on first deploy
- Python ↔ Rust DB schema compat — verify same column names/types (step 12)
- CCBS (Cognitive Bootstrap modules) — defer until core is solid
- Vision PDF ingestion (`ingest_file`) — the one remaining Tier-7 stub. Image captioning
  (`describe_image`, tiered Ollama→Anthropic via `CEREBRO_VISION_BACKEND/_URL/_MODEL`) and
  CLIP visual recall (`search_vision`, gate `CEREBRO_VISION_EMBED`) shipped in the wave-4
  backport (PR #7)
- Dream-cycle **resume** (skip completed phases) — needs a persisted per-cycle phase table + a `cycle_id` on the `dream_run` tool (audit C-RS-004; pre-phase cleanup + `episodes_consolidated` already shipped)
- MCP `resources`/`prompts` surfaces (3 prompts) — port only if an ApexOS consumer needs them; capabilities are advertised honestly so nothing is broken (audit C-RS-011)
- Recall wire-shape parity vs agentd — verify during the ApexOS-RS integration pass (audit C-RS-014)

---

## Meta — when to update this file

- A locked decision changes → update `## Locked decisions`
- A build-order step completes → tick it in the table, note what changed
- A Pi gotcha is discovered → add to `## Gotchas`
- A deferred item resolves → move it out of `## Deferred` with the answer
- Keep this file under ~120 lines of content (excluding this Meta section)
- No task progress, session logs, or version pins here — those go in Cerebro + git history
