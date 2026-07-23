# CerebroCortex-RS → ApexOS-RS — forward-port queue

> The reverse of [BACKPORT-FROM-APEXOS.md](BACKPORT-FROM-APEXOS.md): after the four
> backport waves reconciled all 45 post-fork ApexOS-RS commits into this repo
> (PRs #4–#7), development continued *here* — these are the commits that now need
> to flow **upstream** to `ApexOS-RS/cerebro/crates/`. Evidence verified against
> the ApexOS-RS tree on 2026-07-23.
>
> Path mapping: `crates/…` (here) → `cerebro/crates/…` (ApexOS-RS), i.e.
> `git apply -p2`-shaped, but hand-port with the target tree's idioms — the same
> lesson as the backport waves.

## `1829e0c` fix orphan-table reap: FK resolution killed the drop batch mid-way

*medium / clean — REAL BUG live on every ApexOS node that migrated from a Python `cerebro.db`*

The 8319f57 reap (which ApexOS-RS also carries) runs its DROP batch with
`foreign_keys=ON`. The Python schema's `episode_steps` declares
`FOREIGN KEY (memory_id) REFERENCES memory_nodes(id)`, so the implicit DELETE that
`DROP TABLE _py_episodes` performs tries to resolve that FK against the
already-dropped `memory_nodes` → "no such table: main.memory_nodes" mid-batch.
Result: `memory_nodes`/`associative_links`/`_py_agents` drop, but
`_py_episodes`/`_py_episode_steps`/`_py_audit_log` are stranded **forever** (the
`has_py` probe skips once `memory_nodes` is gone). Reproduced end-to-end on a copy
of the real dev DB before the fix; clean reap after.

Port (both hunks apply nearly verbatim):
- `cerebro/crates/cerebro/src/storage/sqlite.rs` — wrap the reap batch in
  `PRAGMA foreign_keys=OFF; … ; ` and restore `ON` even when the batch errors
  (mirror this repo's `migrate_from_python` already-migrated branch).
- `cerebro/crates/cerebro/tests/integration_test.rs` — the fixture gap that hid
  the bug is identical upstream: `PYTHON_SCHEMA`'s `episode_steps` (~line 1494)
  has **no** `REFERENCES` clauses, unlike the real Python DB. Add the two FK
  lines + the `second_open_reaps_all_python_orphan_tables` regression test
  (verified red-without/green-with here).

**Evidence:** ApexOS-RS `sqlite.rs` reap batch has no `foreign_keys` handling
(grep in the already-migrated branch); test fixture `episode_steps` has no
`REFERENCES` (grep REFERENCES over `cerebro/crates/cerebro/tests/` hits only a
`links` comment at :787). Any node whose DB shows `_py_*` tables after two boots
is already half-reaped — the fix makes the *remaining* tables droppable next
boot only if `memory_nodes` is recreated first; simpler: accept the stranded
three as dead weight on already-bitten nodes, or drop them by hand once.

## `f1f8430` implement ingest_file — the last Tier-7 stub

*medium / clean — first greenfield feature flowing upstream; makes ApexOS-RS 67/67 too*

New `crates/cerebro/src/ingest.rs` (~700 lines incl. tests): extension-routed
port of the Python `cerebro.ingestion` adapter pipeline. text/code + HTML →
paragraph chunks (≤500 words, sentence re-split, ≥10 chars); Markdown → `##`
sections with slug tags + simple frontmatter; JSON → string-or-record lists;
CSV → row-per-memory or schema summary >200 rows; PDF → `lopdf` text extraction;
images → tiered VLM caption + CLIP index. Everything tagged `source:<filename>`
(find_by_tags-reversible). `session_id` deliberately not advertised (C1 lesson).

Port checklist:
- `crates/cerebro/src/ingest.rs` → copy whole file (its deps — `remember`
  signature, `vision::describe/prepare_*`, `index_image`, `MemoryType` — are
  identical upstream since the mirror waves).
- Workspace `Cargo.toml`: `lopdf = "0.44"` (absent upstream; `base64` already
  there); `cerebro/crates/cerebro/Cargo.toml`: `lopdf.workspace = true`.
- `lib.rs`: `pub mod ingest;`
- `dispatch.rs`: the `ingest_file` arm (before `describe_image`); the deferred
  fallthrough comment at ~:1361 shrinks to unknown-names-only; the
  `dispatch_deferred_tool_errors_not_success` test (~:2199) uses `ingest_file`
  as its probe — re-point it at an unknown name (same rewrite as here).
- `tools.rs`: real `ingest_file` schema replacing the stub fallthrough entry;
  header comment "66 functional + the deferred ingest_file stub" → all wired.
  TOOL_NAMES unchanged (count stays 67).
- Audit whitelist: nothing to do — `ingest_file` is already in MUTATING
  (dispatch.rs:138 upstream), so imports get audit rows for free.
- Tests: 8 units + lopdf-built PDF round-trip + 3 dispatch end-to-end ride
  along in the copied files.

**Evidence:** upstream grep — `lopdf` absent from workspace Cargo.toml;
`ingest_file` still the deferred-stub fallthrough (dispatch.rs:1361) and the
deferred-test probe (dispatch.rs:2199); `base64` present (Cargo.toml:63);
audit MUTATING list contains "ingest_file" (dispatch.rs:138).

---

*Post-port check: `cargo test` upstream (their suite + the ~13 riding tests),
clippy clean, then the usual agentd smoke — `tools/list` should still count 67
and an `ingest_file` call should store + audit. Delete this file's entries as
they land (or the whole file when both have).*
