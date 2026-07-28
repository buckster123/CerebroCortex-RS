# Backport queue — ApexOS-RS → CerebroCortex-RS

> Successor to [BACKPORT-FROM-APEXOS.md](BACKPORT-FROM-APEXOS.md) (the completed
> 45-commit reconciliation, waves 1–4): items landing in `ApexOS-RS/cerebro/crates/`
> since the mirror waves that should flow back here. Same porting discipline as
> before: hand-port with this repo's idioms, run the full suite, clippy zero.
> Delete entries as they land (or the whole file when empty).

## 1. Colony field findings: traversal stamping, link ratchet, dedup gate (ApexOS-RS #288)

Four colony nodes cross-checked `never_traversed_links_pct: 100.0` and root-caused
three port gaps (same genre as the auto-link gap). All three apply near-verbatim.

- [ ] `insert_link` → ON CONFLICT ratchet (weight = MAX, `last_traversed` stamp,
  `traversal_count`+1 — Python `add_link` IntegrityError-arm parity). The
  standalone still has the history-wiping `INSERT OR REPLACE`.
- [ ] `GraphStore::add_edge` → update the existing (src, tgt, type) edge in
  place instead of stacking a petgraph parallel edge (double-counted
  spreading conductance).
- [ ] `spread_traced` (spreading.rs) returning walked edges + `recall`
  batch-stamping them via `record_traversals` (one transaction; in-memory
  graph catches up on next rebuild). Documented deviation from Python.
- [ ] Thalamus gate 2 in `remember`: `find_exact_content` (same owner space,
  `agent_id IS ?`) → reinforce + return existing; messages exempt.
- [ ] Riding tests: `recall_stamps_walked_links` (the metric moves),
  `link_reassertion_ratchets_instead_of_wiping`,
  `remember_exact_duplicate_reinforces_not_duplicates`.
