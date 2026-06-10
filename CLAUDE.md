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

---

## Build order (current progress)

| Step | Module | Gate | Status |
|------|--------|------|--------|
| 1 | `types.rs` + `models/` | Serde round-trips | ✓ 6 type tests pass |
| 2 | `activation/` | Values match Python fixtures within 1e-4 | ✓ 41 fixture tests pass |
| 3 | `storage/sqlite.rs` | Schema init, CRUD, scope filtering | ✓ 9 storage tests pass |
| 4 | `storage/vector.rs` | sqlite-vec loads, cosine search, FTS5 fallback | ⬜ |
| 5 | `storage/graph.rs` | petgraph rebuild + neighbor traversal | ⬜ |
| 6 | `engines/` (thalamus→neocortex) | All 8 deterministic engines pass | ⬜ |
| 7 | `cortex.rs` | `remember()` + `recall()` end-to-end | ⬜ |
| 8 | `cerebro-mcp/` (core tools) | MCP handshake + remember/recall vs agentd | ⬜ |
| 9 | Remaining 61 MCP tools | Full tool surface | ⬜ |
| 10 | `engines/dream.rs` | All 6 phases, live LLM calls | ⬜ |
| 11 | `cerebro-cli/` + `cerebro-api/` | CLI and REST parity | ⬜ |
| 12 | DB compatibility | Rust reads a Python-generated `cerebro.db` | ⬜ |

## Cerebro agent

All Cerebro MCP calls in this project use agent `FORGE` (agent_id=`"FORGE"`, ⚒, #B7410E).
Pass `agent_id: "FORGE"` to any Cerebro tool that accepts it so memories stay isolated from other projects.

---

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
- Vision extras (image, PDF, CLIP) — Phase 3 roadmap

---

## Meta — when to update this file

- A locked decision changes → update `## Locked decisions`
- A build-order step completes → tick it in the table, note what changed
- A Pi gotcha is discovered → add to `## Gotchas`
- A deferred item resolves → move it out of `## Deferred` with the answer
- Keep this file under ~120 lines of content (excluding this Meta section)
- No task progress, session logs, or version pins here — those go in Cerebro + git history
