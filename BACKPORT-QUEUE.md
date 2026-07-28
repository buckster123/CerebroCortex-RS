# Backport queue — ApexOS-RS → CerebroCortex-RS

> Successor to [BACKPORT-FROM-APEXOS.md](BACKPORT-FROM-APEXOS.md) (the completed
> 45-commit reconciliation, waves 1–4): items landing in `ApexOS-RS/cerebro/crates/`
> since the mirror waves that should flow back here. Same porting discipline as
> before: hand-port with this repo's idioms, run the full suite, clippy zero.
> Delete entries as they land (or the whole file when empty).

## 1. Listing tools → summary rows (ApexOS-RS #286)

Layer 3 of the token-efficiency arc (follows the wire view already landed here).
`list_procedures` answered a browse with 50 full texts (145k chars on a live
node); a listing should be an index, not a dump.

- [ ] Port `wire_summary` + `wire_summaries` (next to `wire_node` in
  `dispatch.rs` — anchors match near-verbatim): `{id, content_head (200 chars),
  content_chars, memory_type, tags, salience, agent_id, created_at}`.
- [ ] Switch the four listing arms to it: `list_procedures`, `list_intentions`,
  `list_schemas`, `find_by_tags` (the last replaces its inline truncating map —
  that shape cut silently and leaked raw floats/ns timestamps).
- [ ] Fetchers stay full-body BY DESIGN: `get_memory`,
  `find_relevant_procedures`, `find_matching_schemas`, `session_recall`,
  `check_inbox`, thread/episode getters. **`list_deleted` keeps `wire_node`** —
  `get_memory` can't see deleted rows; that listing is the only pre-restore
  window.
- [ ] Tool descriptions teach the contract (listing = browse → `get_memory(id)`
  for bodies).
- [ ] Riding test: port `listing_tools_return_summaries_not_bodies` (multibyte
  200-char head, honest `content_chars`, no `content`/`strength` keys,
  `get_memory` round-trips, `find_relevant_procedures` full texts).


**Watch item, not yet portable:** layer 3 of the token-efficiency arc is parked
in ApexOS-RS `BACKLOG.md` — listing tools (`list_procedures` et al.) still
return full content bodies; the right shape is `{id, content head, tags,
salience}` + `get_memory` on demand. That changes tool contracts live agents
rely on, so it wants its own considered slice in both repos. When it ships
upstream, it becomes the next entry here.
