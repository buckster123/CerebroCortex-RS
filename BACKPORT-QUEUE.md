# Backport queue — ApexOS-RS → CerebroCortex-RS

> Successor to [BACKPORT-FROM-APEXOS.md](BACKPORT-FROM-APEXOS.md) (the completed
> 45-commit reconciliation, waves 1–4): items landing in `ApexOS-RS/cerebro/crates/`
> since the mirror waves that should flow back here. Same porting discipline as
> before: hand-port with this repo's idioms, run the full suite, clippy zero.
> Delete entries as they land (or the whole file when empty).

*Queue empty (2026-07-26 — wire view landed here as `feat/backport-wire-view`).*

**Watch item, not yet portable:** layer 3 of the token-efficiency arc is parked
in ApexOS-RS `BACKLOG.md` — listing tools (`list_procedures` et al.) still
return full content bodies; the right shape is `{id, content head, tags,
salience}` + `get_memory` on demand. That changes tool contracts live agents
rely on, so it wants its own considered slice in both repos. When it ships
upstream, it becomes the next entry here.
