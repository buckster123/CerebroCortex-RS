# CerebroCortex-RS — Audit Plan (Methodology & Coverage Map)

> Companion to [ACTIONPLAN-REPORT.md](ACTIONPLAN-REPORT.md), which holds the findings.
> This document is the **map**: what was checked, how, and against what. It exists so the
> audit's coverage is itself auditable and reproducible.

- **Auditor**: Opus 4.8 (single-context deep pass)
- **Date**: 2026-06-12
- **Subject**: `CerebroCortex-RS` @ commit `c00ed86` (~9.7k LOC, 4 crates)
- **Reference**: `../CerebroCortex` (Python, ~18.7k LOC) — ground truth, never modified
- **Scope (locked with André)**: port internals + behavioral parity vs Python. ApexOS-RS
  integration is **out of scope** this pass. Live build/test allowed on x86 laptop.

---

## Method

1. **Baseline (live evidence)** — `cargo build/test` on this laptop; `clippy` after installing
   the toolchain; `cargo tree`. Captured pass/fail, warnings, and the broken bench.
2. **Parity sweep** — extracted the authoritative tool registry from both sides
   (Python `TOOL_SCHEMAS`, RS `TOOL_NAMES` + dispatch `route()` arms) and diffed names,
   routing, and stub status. Result in the [Parity appendix](#parity-appendix).
3. **Per-dimension deep read** — walked the 11 dimensions below, reading RS against the
   Python source file-by-file, logging each finding with a stable ID.
4. **Synthesis** — severities assigned, findings ordered into execution waves.

## Baseline results (observed)

| Check | Result |
|-------|--------|
| `cargo build --workspace` | ✅ clean, **5 warnings** (unused vars, 1 dead field) |
| `cargo test --workspace` | ✅ **118 passed**, 0 failed (68 lib + 42 integration + 8 mcp) |
| `cargo clippy --all-targets` | ⚠️ **37 warnings** + **1 error** (bench fails to compile) |
| `cargo build --benches -p cerebro` | ❌ **E0061 + E0308** — `activation_bench` stale vs API |
| Tool parity (names) | ✅ **66/66** advertised; 62 functional + 4 stubs |

## Severity rubric

- **Critical** — data loss, crash in normal operation, corrupted MCP stream, silent wrong
  answers to core remember/recall, migration data corruption.
- **High** — missing/divergent behavior a consumer relies on, robustness holes that panic
  under realistic input, security exposure.
- **Medium** — partial parity gaps, error-handling weakness, perf at Pi scale, untested core paths.
- **Low** — minor divergence, ergonomics, non-hot-path inefficiency.
- **Nit** — docs drift, naming, style, dead code.

## Dimensions walked

| # | Dimension | Primary files read | Headline outcome |
|---|-----------|--------------------|------------------|
| 1 | Parity & completeness | `tools.rs`, `dispatch.rs` vs `mcp_server.py` | 66/66 names; 4 stubs; ingestion/vision/watch/resources/prompts dropped |
| 2 | Activation math | `activation/{fsrs,actr,spreading,mod}.rs` vs `activation/*.py` | FSRS/ACT-R **faithful**; spreading **diverges** |
| 3 | Engines | `engines/*.rs` vs `engines/*.py` | thalamus/prefrontal faithful; dream missing resume/pre-phase |
| 4 | Storage & migration | `storage/{sqlite,vector,graph}.rs` | SQL **parameterized** (no injection); migration thorough, 2 tests |
| 5 | MCP protocol | `transport.rs`, `main.rs`, `dispatch.rs` | stdout sane; error code coarse; handshake unconditional |
| 6 | Robustness | dispatch loop, downstream `unwrap`/`panic` | **no panic isolation** → daemon crash risk |
| 7 | Concurrency | `cortex.rs`, `dream.rs` | StdRng-for-Send ok; read-lock-for-write smell (benign) |
| 8 | Security & input | `sqlite.rs`, `cortex.rs`, `cerebro-api/main.rs` | no SQLi; spreading scope bypass; 3 dead scope params in API |
| 9 | Performance & Pi fit | graph rebuild, spreading, recall | bench broken → unmeasured; no obvious hot-path blunders |
| 10 | Tests & coverage | all `#[cfg(test)]`, `tests/` | 118 green; stubs/dream-LLM/spreading-parity untested |
| 11 | Docs & ops | `CLAUDE.md`, `README`, `main.rs`, `deploy/` | "63 tools" drift in 3 places vs actual 66 |

---

## Parity appendix — tool registry diff

**Python `TOOL_SCHEMAS`: 66 tools. RS `TOOL_NAMES`: 66 tools. Name-level parity: 100%.**
(Python also exposes 3 *prompts* — `session_handoff`, `memory_review`, `context_briefing` —
and MCP *resources*; these are **not** tools and are not ported. See C-RS-011.)

Of the 66 RS tools, **62 are functionally routed** and **4 are stubs** that fall through to
`{"status":"not_yet_implemented"}` and advertise a generic `(stub)` schema:

| Stub tool | Python subsystem behind it | Status in RS |
|-----------|----------------------------|--------------|
| `ingest_file` | `ingestion/` (pdf/html/csv/md/json/image adapters, chunker, pipeline) | not ported |
| `describe_image` | `storage/vision_embeddings`, `ingestion/image_adapter` | not ported |
| `search_vision` | vision/CLIP embeddings | not ported |
| `cognitive_bootstrap` | CCBS bootstrap modules | not ported |

All other 62 tools have a `route()` arm (`memory_store`/`memory_search` alias to
`remember`/`recall` via a combined arm). No advertised tool is silently unrouted beyond
these 4. **Full 66-name list verified equal between `TOOL_NAMES` and `TOOL_SCHEMAS`.**

### Dropped Python subsystems (intentional vs silent — see findings)
- `ingestion/` (8 adapters + chunker + pipeline) — **dropped**, surfaced via 4 stubs above.
- `storage/vision_embeddings.py`, `storage/chroma_store.py` — **dropped** (RS uses sqlite-vec).
- `watch/watcher.py` (filesystem watch) — **dropped**, no tool exposes it (by-design).
- `migration/` text/markdown/json/neo importers — **dropped** (RS has DB auto-migration instead).
- MCP `resources` + `prompts` surfaces — **dropped** (correctly absent from advertised capabilities).

---

## What this pass did **not** cover (honest gaps)

- **ApexOS-RS / agentd integration** — out of scope by request. The recall-result wire shape
  (`{memory, score}`) is flagged (C-RS-014) for that future pass.
- **Runtime behavior on the Pi** (arm64) — all evidence is x86-laptop. fastembed model download,
  sqlite-vec `.so` load path, and systemd lifecycle are unverified here.
- **Performance numbers** — the benchmark does not compile (C-RS-005), so no measured baselines.
- **Live LLM dream phases** — require `ANTHROPIC_API_KEY`; the LLM-call paths (phases 2/3/6)
  were read but not executed.
