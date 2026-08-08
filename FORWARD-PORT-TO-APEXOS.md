# Forward-port queue → ApexOS-RS (`cerebro/crates/`)

> Cerebro-core changes born here that must mirror upstream (standing rule:
> cross-apply all cerebro work both ways; ALWAYS diff the real files first —
> the repos drift). Delete each entry when its port lands; delete the file
> when empty.

## 1. Spreading-activation seed-cap no-op fix (HIGH — affects APEX's live recall quality)

`crates/cerebro/src/activation/spreading.rs` (CC-RS PR #24, 2026-08-08):
`SPREADING_MAX_ACTIVATED` used to count the seeds (`activated.len() >= max`),
and recall over-fetches `k*5 = 50` = the cap — so on any store returning a
full candidate page the spread broke before hop 1. **Spreading activation was
silently a no-op on every mature brain**, association scores never contributed
to ranking, and `never_traversed_links_pct` could sit at exactly 100.0 (the
colony's 4/4 finding — the missing write half was only part of the story).
The budget is now `new_count` — growth beyond the seeds. Python inherits the
same flaw (spreading.py:155, reference only — do not modify). Regression test
`full_seed_page_still_spreads` rides. Measured on the dev brain: same query
went 0 walks → 75 walks, top score 0.415 → 0.62.

## 2. Traced recall (feature — port with or ahead of any Lucida upstreaming)

`spread_events` + `TraceEvent` (spreading.rs), `recall_traced` + `RecallTrace`
(cortex.rs; `recall` is now a thin wrapper), `POST /recall/trace` (cerebro-api).
The observable anatomy of a recall — seeds, per-hop walks, activation map —
same pipeline, same reinforcement. Wire shape in CC-RS PR #24.
