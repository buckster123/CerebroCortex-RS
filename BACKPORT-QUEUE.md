# Backport queue — ApexOS-RS → CerebroCortex-RS

> Successor to [BACKPORT-FROM-APEXOS.md](BACKPORT-FROM-APEXOS.md) (the completed
> 45-commit reconciliation, waves 1–4): items landing in `ApexOS-RS/cerebro/crates/`
> since the mirror waves that should flow back here. Same porting discipline as
> before: hand-port with this repo's idioms, run the full suite, clippy zero.
> Delete entries as they land (or the whole file when empty).

## `c84a5a9` wire view for memory-returning tools (ApexOS-RS #282)

*small / clean — pure token-efficiency win on the most-used tool surface*

Stop serde-dumping the internal `MemoryNode` storage model into the model's
context window. Field-measured (apex1, 2026-07-26, during a token-estimator
calibration against real `count_tokens`): a live 209k-char `list_procedures`
result was 26% agent-inert bookkeeping — `access_times` (up to 50 ns-precision
timestamps per node), FSRS `strength` internals, 16-decimal floats — and
escaped, timestamp-dense JSON tokenizes at ~2 chars/token, the worst rate
there is. Note the 64KiB thalamus gate (CB-029) does NOT bound this — it
guards the *store* path (`remember`), not tool outputs.

Port (applies near-verbatim — `dispatch.rs` has kept the same shape since the
mirror waves; this repo's copy still has all 15 serde-dump sites):

- `crates/cerebro-mcp/src/dispatch.rs` — add the helper block before
  `fn route(...)`: `wire_node` / `wire_nodes` / `round2` / `round3` +
  `wire_time` (second-precision RFC3339). Wire view keeps: id, content,
  memory_type, layer, salience (2dp), tags, agent_id, visibility,
  created_at/updated_at (second precision), access_count; thread_id +
  emotional fields only when set; metadata kept (provenance is
  agent-meaningful). Drops: `access_times`, `strength`.
- Convert the memory-returning arms: remember/get_memory/update_memory/
  session_save/send_message echoes → `wire_node`; recall/memory_search/
  session_recall hit maps → `{"memory": wire_node(&node), "score": round3(score)}`;
  memory_neighbors/common_neighbors/list_deleted/check_inbox/
  get_thread_memories/get_episode_memories → `wire_nodes`; list_intentions/
  list_procedures/list_schemas/find_matching_schemas `Ok(json!(filtered|nodes))`
  → `wire_nodes`; find_relevant_procedures `"procedures": matched` →
  `wire_nodes(&matched)`.
- **The one deliberate exception: `export_memories` stays RAW** (backup tool —
  a re-import must not lose access history or FSRS state). Comment + test lock
  this. `find_by_tags` already has its own hand-rolled slim view — untouched.
- `crates/cerebro-mcp/Cargo.toml`: `chrono.workspace = true` (for `wire_time`).
- Test: `wire_view_drops_bookkeeping_and_rounds` rides along (drop + rounding +
  timestamp precision + export exception).

**Evidence:** ApexOS-RS `cerebro/crates/cerebro-mcp/src/dispatch.rs` at
`c84a5a9` is the reference; grep this repo's `dispatch.rs` for
`serde_json::to_value(&node` / `"memory": node` to find the sites.

**Parked upstream (BACKLOG.md there), applies here identically when it lands:**
layer 3 — listing tools still return full content bodies (`list_procedures`
answered a browse question with 50 full procedure texts, 145k chars); the right
shape is `{id, content head, tags, salience}` + `get_memory` on demand, but
that changes tool contracts live agents may rely on, so it wants its own
considered slice in both repos.

---

*Post-port check: full suite + clippy zero, then live-verify on the dev MCP
(rebuild `target/release/cerebro-mcp` = the upgrade, `/mcp` reconnect): a
`recall` hit should carry no `access_times`/`strength` and 2dp salience, and
`export_memories` should still return the full struct.*
