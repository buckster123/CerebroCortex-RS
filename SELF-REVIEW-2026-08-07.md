# SELF-REVIEW-2026-08-07 — six-dimension review, adversarially verified

> Produced by a 53-agent review workflow (6 dimension reviewers + 1 adversarial
> verifier per finding; every finding below survived a refutation attempt against
> the code, the Python reference, and the documented-deviations list; 1 finding was
> refuted and dropped, 2 duplicates found independently by two dimensions were merged).
> Baseline: main @ 3597c86, 250 tests green, clippy clean.
>
> 44 findings: **6 high · 21 medium · 17 low**. IDs R-01..R-44, ordered by severity.
> Wave grouping below is the suggested fix order; each finding is self-contained
> (file:line + failure scenario) so a worker-brain can take a wave without this context.

## Dimension health (the good news is real)

**Python parity** — The numeric core is a genuinely faithful port: every constant (ACT-R, FSRS, spreading, score weights, link-type weights), every formula, and the thalamus/amygdala/temporal heuristics match Python exactly, spreading activation reproduces seed weighting, undirected hop-decay BFS, sublinear accumulation and normalization byte-for-byte, and the documented deviations (traversal stamping, per-owner dedup, listing summaries) are real improvements honestly recorded. The drift lives at the integration seams rather than in the math: remember's ignored/inverted visibility handling is the one high-severity item, and the recall pipeline is missing three Python lifecycle steps (recall-time Hebbian weight strengthening, layer promotion/decay sweeps, context-seeded activation) plus an FSRS wiring slip that pins never-recalled memories at retrievability 1.0 in ranking. Thread visibility and the outgoing-only graph-exploration tools are the remaining scope/traversal gaps. None of these corrupt data; they make the Rust brain rank and consolidate measurably differently from the Python reference on identical stores, and all are fixable surgically without touching the verified core.

**Storage integrity** — Storage integrity is fundamentally well-engineered: single mutex-guarded connection with WAL + busy_timeout for the two-daemon case, a genuinely clever data_version staleness probe wired into the hot paths so the petgraph cache rebuilds exactly when a foreign process commits, an insert_link ratchet that correctly mirrors GraphStore's in-place edge update (including the petgraph swap-remove index repair), and careful CB-numbered reasoning around scope enforcement, rowid reuse, and vec0's OR-REPLACE quirk — with regression tests to match. The two serious gaps are both completeness misses in otherwise-thought-through mechanisms: the purge transaction cleans links and vectors but forgot the memory_versions and vision_embeddings FK children (so purging an image memory or emptying a trash containing one restore_version'd row hard-fails, verified empirically), and the Python-migration FTS rebuild indexes soft-deleted rows, breaking the CB-020 trigger invariant so a later restore measurably corrupts the FTS index. Below those, an escaping bug silently disables tag auto-association for underscore tags, and the OR REPLACE/no-vec-rebuild/VACUUM items are latent hazards worth closing cheaply. Nothing found suggests ongoing corruption in normal (non-migrated, non-purging) operation.

**MCP wire contract** — The wire plumbing itself is in strong shape: 67 tools advertised, all 67 routed with a count-asserting test, per-frame parse isolation and the 32 MiB frame cap with boundary drain are implemented and unit-tested, per-call panic isolation is tested, stdout is clean (no stray print calls anywhere in the workspace; logs go to stderr), and the wire_node/wire_summary listing contract is coherent and thoroughly tested. The weaknesses are schema-to-handler honesty at the edges: remember's advertised `visibility` is accepted and ignored (privacy-relevant), update_memory silently skips the version snapshot its sibling tool promises, and a cluster of tools (session_save/recall, purge_all_deleted, nine destructive tools with hidden agent_id scoping, find_path) have quietly drifted from either the Python drop-in contract or their own advertised schemas. None of these are transport-level regressions — they are contract-truthfulness gaps, mostly fixable with small handler or schema edits.

**Dream engine** — The dream engine's newest layer is its best: the exo-evolution machinery (Wilson lower-bound math verified by hand against its documented values, novelty exemption, champion-never-demoted precedence, refine/merge candidate selection with pile-up guards, dream_mutated/dream_merged provenance via derived_from) is correct, pure-function factored, and thoroughly unit-tested, and the retention sweep, pre-phase episode cleanup, LLM-unavailable skip paths, and CB-024 persist-before-count discipline are all clean. The weak spot is the classic 6-phase Python port around episodes: the unconsolidated-episode gating that Python relies on was never ported, so phases 1/3/4 reprocess the same recent episodes every cycle — ratcheting link weights and emotional salience and minting duplicate schemas nightly — and one leftover byte-indexed slice in REM recombination can panic the whole cycle on multibyte content. Fix those two systemic items (plus the prune_candidate clear-on-recovery gap) and this phase pipeline would be in genuinely strong shape; nothing found suggests the persisted data model itself is wrong, only that repeated cycles drift it.

**API + CLI parity** — GOOD: the architecture does the heavy lifting for parity — the dedup gate, auto-link, recall-time traversal stamping, and FSRS reinforcement all live inside the library's remember()/recall(), so all three front-ends inherited the backport waves automatically, and the API carries its own CB fixes (CB-006 re-embed, CB-012 priority, CB-023 panic-catch, CB-026 session filters) with tests. The concern is that the CLI is now the lagging surface: it missed the CB-006 and CB-012 twins, its `delete --force` inverts the Python flag's meaning into an unprompted permanent purge, its outcome grading bypasses the fitness ledger entirely, and its agents listing reads nonexistent keys. The REST API is also a narrower surface than the Python one it claims to mirror (no procedure-outcome, activation, audit, share, or ingest routes, and the startup log advertises a dashboard at '/' that is never routed), and its trash lifecycle and graph endpoints skip the graph-eviction/refresh discipline the MCP surface follows.

**Tests + docs** — Tests are in genuinely good shape: 250 green, and the wave-5 features are mostly guarded by sharp, behavior-naming regression tests — traversal stamping, the insert_link ratchet (weight/stamp/count all asserted), the dedup gate including cross-space isolation and the message exemption, auto-link both at encoding and retrofit, the wire_summary contract with a multibyte head-cut trap, and the exo-evolution logic (Wilson bounds, champion demotion rules, 8-phase cycle asserted at two levels). The real gaps are narrow but specific: the GraphStore::add_edge parallel-edge fix and the entire CSV ingest path could regress with the suite still green, and list_deleted's deliberate full-body exception is unpinned. Docs are the weaker half: README's front page is current but its test counts lag a wave, while ARCHITECTURE.md lags two (63/66 tools, 6-phase dream, missing ingest/vision modules) and CLAUDE.md's own header contradicts its build table — the code moved faster than the "docs travel with code" rule was enforced.

## Wave plan

| Wave | Findings | Theme |
|------|----------|-------|
| W-A | R-02 R-04 R-05 R-06 | data-integrity + panic + contract-lie fixes (do first) |
| W-B | R-01 R-03 | destructive-UX + dream compounding |
| W-C | R-20 R-21 R-22 R-23 R-24 R-33 R-34 | recall/consolidation parity (the brain ranks differently than Python) |
| W-D | R-07..R-12 R-28 | API/CLI catch-up with the dispatch surface |
| W-E | R-13..R-19 R-25 R-29..R-32 R-35..R-39 | remaining mediums/lows, storage polish |
| W-F | R-26 R-27 R-43 R-44 | test-coverage debt |
| W-G | R-40 R-41 R-42 | doc drift — **applied in the PR that carries this report** |


---

## HIGH severity

### R-01 [api-cli] CLI `delete --force` silently repurposed from 'skip confirmation' to irreversible purge

`crates/cerebro-cli/src/main.rs:445`

In the Python reference CLI, `cerebro delete <id> --force` means 'skip the confirmation prompt' on a SOFT delete (CerebroCortex/src/cerebro/interfaces/cli.py:269 — help text 'Skip confirmation'). The Rust CLI reuses the same flag name but maps it to `purge_memory` — a permanent hard delete of the row and its dependents that bypasses the trash/restore lifecycle entirely, with no confirmation prompt anywhere (main.rs:442-451). It also ignores `--agent` scope (uses `VisibilityScope::global()`), unlike Python's access check.

**Failure scenario:** A user with Python muscle memory runs `cerebro delete mem_abc --force` expecting a recoverable soft delete without a prompt; the memory is instead permanently destroyed — it never appears in `list_deleted`, `restore_memory` cannot bring it back, and version history is gone.

**Verification note:** Confirmed at main.rs:442-451: --force maps to purge_memory (verified hard delete at sqlite.rs:1042-1069 — deletes row, links, and vector in one transaction), while Python's --force (cli.py:269) only skips the confirmation prompt on a soft delete (cortex.py delete_memory defaults hard=False). No confirmation mechanism exists anywhere in the CLI crate, and the command's own help text says 'Soft-delete a memory' with no help text on --force, so even --help gives no warning. Recovery is impossible: list_deleted/restore_memory require a soft-deleted row, and restore_version (dispatch.rs:1295-1305) requires the live memory row — orphaned memory_versions rows survive (minor inaccuracy in the finding) but cannot be used. No test covers the CLI Delete path. The deviation is undocumented — CLAUDE.md claims CLI parity with Python and no docs note the repurpose.

### R-02 [dream] REM recombination uses byte-indexed slicing that panics on multibyte content

`crates/cerebro/src/engines/dream.rs:1310`

Phase 6 builds the LLM prompt with `&node_a.content[..node_a.content.len().min(300)]` (and the same for node_b on line 1311) — raw byte-index slicing. The file's own `truncate_chars` helper (line 1437) exists precisely because, as its doc comment says, byte-indexed slicing 'panics when n lands mid-multibyte-char (emoji, CJK, smart quotes)', and every other phase uses it — REM is the one site that was missed. The MCP daemon survives via per-call panic isolation (dispatch.rs:67-74), but the dream cycle aborts mid-flight: all earlier-phase work is done yet the DreamReport is never persisted (save_dream_report is after all phases), and in cerebro-cli/cerebro-api the panic aborts the process/handler outright.

**Failure scenario:** A memory whose content exceeds 300 bytes with a multibyte character straddling byte offset 300 (e.g. a session summary containing emoji or CJK text) is sampled by REM recombination; the slice panics, dream_run returns a JoinError-derived JSON-RPC error, and the night's dream report is lost.

**Verification note:** Verified verbatim: dream.rs:1310-1311 uses raw byte slicing `&node_a.content[..node_a.content.len().min(300)]` while the file's own truncate_chars helper (line 1437, doc comment explicitly naming the mid-multibyte panic) is used at every other prompt site (476, 519, 614, 716, 918, 985-986, 1003, 1636) — REM is the one missed site. The slice runs before llm_call, so it fires whenever a sampled node has >300 bytes with a multibyte char straddling offset 300; nothing upstream normalizes content, and the project's own session summaries contain emoji/arrows/checkmarks. run_cycle maps phase Errs to PhaseResult::failed (line 279) but a panic is not an Err — it unwinds past save_dream_report (line 298), losing the report while earlier phases' DB writes persist, exactly as claimed.

### R-03 [dream] No unconsolidated-episode gating: phases 1/3/4 reprocess all episodes every cycle, compounding weight and salience drift

`crates/cerebro/src/engines/dream.rs:325`

Python gates SWS replay, schema formation, and emotional reprocessing on `episodes.get_unconsolidated()` and marks episodes consolidated at cycle end (dream.py:262-266, 355, 494, 580). The Rust port has no consolidation concept at all (no `consolidated` column or method anywhere in storage) — it fetches the most recent 100/50 episodes by `started_at DESC` every cycle (dream.rs:325, 586, 818). Three effects compound nightly on the same episodes: (a) SWS adds +0.08 to every consecutive-pair link each cycle (line 345), saturating all episodic-chain weights toward 1.0 and inflating traversal counts via the insert_link ratchet; (b) emotional reprocessing re-applies `apply_emotion` whose `salience = salience + salience_adj` (amygdala.rs:76) is a per-call ratchet, so emotionally-worded episode memories inflate to the 1.0 clamp — and salience is the ranking currency for prefrontal ranking and `procedure_fitness`; (c) schema formation re-submits the same recent episodes to the LLM every cycle. This is not in the documented deliberate-deviations list (the deferred 'dream-cycle resume' item is about skipping completed phases within one interrupted cycle, not cross-cycle episode marking).

**Failure scenario:** A brain dreaming nightly: after ~10 cycles every link in the recent 100 episodes is saturated at weight 1.0, episode memories containing words like 'success'/'failed' have salience pinned near 1.0 regardless of true importance, and spreading-activation recall ranks stale episodic chains above genuinely salient memories.

**Verification note:** Verified on all four checks. (1) Cited code matches: dream.rs:325/586/818 fetch episodes via list_episodes (started_at DESC LIMIT 100/50, no filter — sqlite.rs:1992); no `consolidated` column or method exists anywhere in Rust storage, and the Python-DB auto-migration (sqlite.rs:383-398) actually DROPS Python's consolidated column, making even previously-consolidated episodes re-processable.

### R-04 [mcp-wire] update_memory never snapshots prior content — get_memory_versions' advertised contract is false

`crates/cerebro-mcp/src/dispatch.rs:422`

The get_memory_versions schema (tools.rs:826) promises "Each content change creates a snapshot", and Python saves a version before every content edit (cortex.py:1055-1058, _graph.save_version). The Rust update_memory route (dispatch.rs:378-427) detects content_changed only to re-embed; it never calls log_memory_version. The only production call site of log_memory_version in the whole workspace is restore_version's auto-snapshot (dispatch.rs:1306). So content edits leave no version row.

**Failure scenario:** An agent edits a memory via update_memory(content:...), then wants to undo: get_memory_versions returns an empty list and restore_version has nothing to restore — the pre-edit content is silently and permanently lost, despite the tool schema explicitly promising a snapshot per content change.

**Verification note:** Confirmed on all four checks. (1) tools.rs:826 promises "Each content change creates a snapshot", but the update_memory route (dispatch.rs:378-427) uses content_changed only to re-embed (lines 423-425) and never calls log_memory_version; SqliteStore::update_memory (sqlite.rs:754-785) is a plain UPDATE with no internal snapshot. Workspace grep shows the sole production call site of log_memory_version is restore_version's auto-snapshot (dispatch.rs:1306); the gap also exists in cerebro-api's PUT /memory/:id (main.rs:318-341) and the CLI update paths. (2) The failure scenario is unguarded: a content edit overwrites in place, get_memory_versions returns [], restore_version has nothing to restore — pre-edit content is permanently lost. (3) Not a documented deviation: nothing in CLAUDE.md Deferred/TBD, AUDIT-PLAN.md, ACTIONPLAN-REPORT.md, BACKPORT-FROM-APEXOS.md, or docs/.

### R-05 [parity] remember advertises `visibility` but silently ignores it; agent-scoped stores are forced Private (Python defaults SHARED) *(independently found by MCP wire contract)*

`crates/cerebro-mcp/src/dispatch.rs:308`

The `remember` tool schema advertises a `visibility` param with enum private/shared/thread (crates/cerebro-mcp/src/tools.rs:30-34), but the dispatch handler (dispatch.rs:308-319) never reads args["visibility"], and cortex.rs:96-99 derives visibility purely from scope: agent_id present => Visibility::Private, absent => Shared. Python's remember() takes an explicit visibility parameter defaulting to SHARED regardless of agent_id (cortex.py:271, mcp_server.py:1447-1461). The `memory_store` alias (dispatch.rs:430-447) has the same gap. This is exactly the accept-and-ignore pattern the project rejects elsewhere (CLAUDE.md's ingest_file note: 'no accept-and-ignore'), and it is not in the documented-deviations list.

**Failure scenario:** Agent FORGE calls remember(content=..., agent_id="FORGE", visibility="shared"). The memory is stored Private-to-FORGE with the shared request silently dropped. Any other agent's recall/get_memory then fails to return it (sql_filter excludes other agents' private rows), so cross-agent knowledge sharing that works on the Python server silently returns nothing on the Rust port — mislabeled data persisted, no error to the caller.

**Verification note:** Confirmed on every material point. tools.rs:30-34 advertises `visibility` (private/shared/thread) on `remember`; the dispatch handler (dispatch.rs:308-319) never reads args["visibility"]; the library API (cortex.rs:85-99) has no visibility parameter and derives it purely from scope (agent_id present => Private, absent => Shared). Python reference verified: src/cerebro/cortex.py:271 defaults `visibility=Visibility.SHARED` regardless of agent_id, and src/cerebro/interfaces/mcp_server.py:1447-1461 parses args["visibility"] and passes it through. Failure scenario traces cleanly: FORGE's remember(..., visibility="shared") persists Private-to-FORGE; any other agent's scope sql_filter "(visibility='shared' OR (visibility='private' AND agent_id=?))" excludes it; no error is raised (the wire response does show "visibility":"private", the only faint signal).

### R-06 [storage] purge_memory / purge_all_deleted fail with FOREIGN KEY constraint when memory_versions or vision_embeddings rows exist

`crates/cerebro/src/storage/sqlite.rs:1066`

The purge transaction deletes dependent memory_vectors and links rows (CB-005/CB-022) but not memory_versions (FK at schema line 2515) or vision_embeddings (FK at line 2530), both declared REFERENCES memories(id) with no CASCADE while foreign_keys=ON. Verified empirically: DELETE FROM memories with a version child fails with 'FOREIGN KEY constraint failed (19)'. memory_versions rows are written by restore_version's auto-snapshot (cerebro-mcp/src/dispatch.rs:1306); vision_embeddings rows by the image-caption path (cerebro/src/cortex.rs:522). Nothing in the codebase ever deletes vision_embeddings, and memory_versions is only trimmed by the cap-based retention sweep.

**Failure scenario:** Any image memory indexed via describe_image/ingest_file, or any memory that went through restore_version, can never be hard-deleted: purge_memory returns Err. Worse, purge_all_deleted is a single transaction — one such row in the trash makes the whole DELETE fail and roll back, so 'empty trash' is permanently bricked and soft-deleted rows accumulate forever.

**Verification note:** Confirmed at every level. (1) Code: purge_memory (sqlite.rs:1042-1069) and purge_all_deleted (:1075-1109) clean only memory_vectors and links before DELETE FROM memories; memory_versions (FK sqlite.rs:2515) and vision_embeddings (FK :2530) both REFERENCE memories(id) with no CASCADE, and PRAGMA foreign_keys=ON is set at open() (:457). (2) Empirically reproduced: DELETE of a parent with a child row in either table fails with 'FOREIGN KEY constraint failed (19)'.


---

## MEDIUM severity

### R-07 [api-cli] CLI `procedure outcome` bypasses the fitness ledger; REST has no outcome route at all

`crates/cerebro-cli/src/main.rs:791`

MCP `record_procedure_outcome` (cerebro-mcp/src/dispatch.rs:1151-1202) adjusts salience asymmetrically, updates FSRS difficulty, manages the `prune_candidate` tag, and writes `metadata.outcomes {successes, failures}` via `record_outcome_ledger` (dispatch.rs:24) — the evidence base for the dream engine's skill_competition phase. The CLI's `ProcedureCmd::Outcome` (main.rs:791-802) instead just swaps an `outcome:<string>` tag: no ledger, no salience/difficulty change, no prune flag, and it accepts an arbitrary string instead of a success bool. `record_outcome_ledger` lives only in the MCP crate, so no other front-end can grade correctly. The REST API is worse: Python's API exposes `POST /procedures/{id}/outcome` (api_server.py:1519) but the Rust API has no outcome route.

**Failure scenario:** A procedure graded exclusively through the CLI (or never gradeable via REST) stays at 0 recorded outcomes: skill_competition treats it as novelty-exempt/ungraded forever, `retrieval_rank` falls back to salience, and chronically failing procedures are never demoted or flagged prune_candidate — the Darwinian loop CLAUDE.md mandates ('the fitness ledger only exists if we feed it') silently starves.

**Verification note:** Verified verbatim at both cited sites. CLI ProcedureCmd::Outcome (cerebro-cli/src/main.rs:791-802) only swaps an `outcome:<string>` tag from a free-form string — no ledger, no salience/difficulty change, no prune_candidate — while MCP record_procedure_outcome (cerebro-mcp/src/dispatch.rs:1151-1200) does all of that via record_outcome_ledger (dispatch.rs:24), which exists only in the MCP crate; the cerebro library has no shared grading method. The Rust API has no outcome route (cerebro-api/src/main.rs:950 has only GET/POST /procedures) while Python exposes POST /procedures/{id}/outcome (api_server.py:1519).

### R-08 [api-cli] API trash lifecycle bypasses Storage graph-eviction wrappers — deleted nodes keep spreading

`crates/cerebro-api/src/main.rs:348`

MCP delete/restore/purge/bulk_delete go through the `Storage` write-guard wrappers that evict the node from the in-memory petgraph (or rebuild on restore) — storage/mod.rs:59-119, dispatch.rs:369-376, 583-618. The API handlers call `storage.read().await.sqlite.*` directly: delete_memory (main.rs:344-351, also using global scope while MCP honors agent scope), restore_trash (799-806), purge_trash (808-815), bulk_delete (824-831). Because `PRAGMA data_version` is unchanged by a connection's own commits (documented in sqlite.rs ~2050), the API process's own `refresh_graph_if_stale` never fires for these writes — the phantom node persists until a foreign-process commit or restart.

**Failure scenario:** Client DELETEs a memory via REST, then POSTs /recall: spreading activation still traverses the deleted node's edges, boosting its neighbors with conductance from content the user believes is gone; conversely a restored memory contributes no edges to recall until an unrelated MCP-side write forces a rebuild.

**Verification note:** Verified on all counts. The four API handlers (main.rs:344-351, 799-806, 808-815, 824-831) call storage.read().await.sqlite.* directly with VisibilityScope::global(), bypassing the StorageCoordinator wrappers (storage/mod.rs:59-117) that evict the node from petgraph on delete/purge/bulk_delete and rebuild+re-baseline on restore — the exact wrappers MCP dispatch uses under the write lock (dispatch.rs:369-376, 583-618). The staleness detector cannot save it: SqliteStore is one shared Arc<Mutex<Connection>> and data_version() (sqlite.rs:2054-2057) reads the pragma on that same connection, which sqlite.rs:2048-2053 documents as deliberately unchanged by own commits — the design assumes own writes maintain the graph incrementally (also stated at cortex.rs:342-344), which these handlers violate.

### R-09 [api-cli] CLI session save writes lowercase `priority:` tags — invisible to MCP/API priority filters (CB-012 missed)

`crates/cerebro-cli/src/main.rs:556`

The CB-012 backport added `normalize_priority` (uppercase canonical) to both the MCP dispatch (dispatch.rs:1659, applied at 527) and the API (main.rs:85-87, applied at 470), with tests asserting lockstep. The CLI was missed: `SessionCmd::Save` interpolates the raw flag value (`format!("priority:{priority}")`, main.rs:554-557) with a lowercase default "medium" (main.rs:172). CLI `session recall` (main.rs:568-584) also lacks the priority/session_type filters both twins support, so the gap can't even be observed from the CLI itself.

**Failure scenario:** `cerebro session save "..." --priority high` stores tag `priority:high`; a later MCP `session_recall(priority="high")` filters for `priority:HIGH` and returns nothing — CLI-authored session notes are permanently invisible to every priority-filtered recall.

**Verification note:** Confirmed at every cited line: the CLI (main.rs:554-557) interpolates the raw --priority flag (default "medium", main.rs:172) into the tag with no normalization, while both twins canonicalize to uppercase (dispatch.rs:1659/527/560, api main.rs:85/470/498) and filter by exact tag equality. Nothing in the store path (cortex/thalamus) normalizes tags, so `priority:medium`/`priority:high` is stored verbatim and can never match the `priority:MEDIUM`/`priority:HIGH` filters — even the flag-less CLI default is affected. The CB-012 backport commit (a11a76b) touched only cerebro-mcp and cerebro-api, never cerebro-cli; no deferred/deviation note exists in CLAUDE.md, BACKPORT-FROM-APEXOS.md, or docs/; cerebro-cli has zero tests. The Python reference CLI (interfaces/cli.py:644) uses click.Choice(["HIGH","MEDIUM","LOW"]) so it structurally cannot write lowercase — this is also a parity regression.

### R-10 [api-cli] CLI `update --content` never re-embeds — vector index keeps pre-edit text (CB-006 missed)

`crates/cerebro-cli/src/main.rs:462`

CB-006 fixed the API and MCP update paths to call `vector.embed_and_store` when content changes, because `sqlite.update_memory` only refreshes the content column + FTS5 trigger (api main.rs:334-339, dispatch.rs:422-426). The CLI's `Command::Update` (main.rs:454-468) applies the content change and calls `sqlite.update_memory` with no re-embed. The Python CLI explicitly documents `--content` as 'triggers re-embedding' (cli.py:292).

**Failure scenario:** `cerebro update mem_x --content "entirely new topic"` leaves the vec0 row embedding the old text: semantic recall ranks the memory under its former meaning and misses queries about the new content (FTS5 keyword search masks the bug intermittently).

**Verification note:** Confirmed. cerebro-cli/src/main.rs:454-468 applies a content change and calls only sqlite.update_memory, which (verified at storage/sqlite.rs:754-785) updates the memories table + FTS5 trigger and never touches the vec0 memory_vectors row. The API (cerebro-api/src/main.rs:334-339) and MCP (dispatch.rs:422-425) paths both gained the CB-006 re-embed on content change; the CLI did not. The CLI initializes a full CerebroCortex from env config, so the embedder is live and shares the daemon's database — a `cerebro update --content` edit really does leave a stale embedding, and the CLI backfill command only fills missing vectors, never stale ones, so nothing self-heals. Not documented as deliberate (CLAUDE.md Deferred and BACKPORT-FROM-APEXOS.md CB-006 entry cover API/MCP only), and cerebro-cli has no tests.

### R-11 [api-cli] CLI `agents list` reads row keys that don't exist — prints '? ?' for every agent

`crates/cerebro-cli/src/main.rs:595`

`SqliteStore::list_agents` returns rows keyed `id`, `name`, `description`, `metadata` (nested object) — sqlite.rs:1339-1348. The CLI's human-readable branch reads `a["agent_id"]`, `a["display_name"]`, and `a["symbol"]` (main.rs:595-597), none of which exist in the row, so every agent renders as the fallback placeholders. The `--json` branch is unaffected, which is why this survives.

**Failure scenario:** `cerebro agents list` on a store with registered agents prints `  ?                    ?` per agent — id, name, and symbol all fall back to placeholders, making the command useless without --json.

**Verification note:** Confirmed at both cited sites: SqliteStore::list_agents (crates/cerebro/src/storage/sqlite.rs:1344-1348) emits rows keyed id/name/description/registered_at/last_seen/metadata, while the CLI human branch (crates/cerebro-cli/src/main.rs:595-597) reads agent_id/display_name/symbol — none present (symbol is additionally nested under metadata, per the Register arm at main.rs:603). No remapping exists anywhere (CLI calls sqlite.list_agents() directly), so serde_json Null indexing triggers the unwrap_or placeholders and every agent prints '   ?                    ?'. Not a documented deviation (nothing in CLAUDE.md Deferred/TBD or docs/), the Python reference CLI works via typed AgentProfile attributes (cli.py:764-766) so this breaks claimed Step-11 CLI parity, and cerebro-cli has zero tests to catch it. The --json branch passes rows through verbatim, which is why it's unaffected.

### R-12 [api-cli] API graph endpoints never refresh the stale in-memory graph (CB-003 missed)

`crates/cerebro-api/src/main.rs:569`

The MCP twins of every graph-reading tool call `brain.refresh_graph_if_stale()` first, explicitly to 'see the other front-end's links' (dispatch.rs:465 memory_neighbors, 479 find_path, 496 common_neighbors, 512 memory_graph_stats). The API's graph_neighbors (main.rs:569-584), graph_path (586-600), and graph_common (602-612) read `storage.graph` with no refresh. cerebro-api is a long-running process sharing the SQLite file with cerebro-mcp on the Pi, and only `recall()` refreshes internally — the graph endpoints serve a snapshot from process start.

**Failure scenario:** cerebro-mcp stores memories and links all day; the dashboard/API's /graph/neighbors and /graph/path return results missing every node and edge created since cerebro-api last restarted (or last happened to run a recall), with no error or staleness hint.

**Verification note:** Verified, not refuted. The four MCP graph arms (dispatch.rs:465/479/496/512) call refresh_graph_if_stale() explicitly for CB-003, while the API's graph_neighbors/graph_path/graph_common (cerebro-api/src/main.rs:569-612) — the only three storage.graph readers in the API — never refresh, despite Brain being Arc<CerebroCortex> which carries the method. The mechanism's own doc (cortex.rs:337-344) and the CB-003 integration test name cerebro-mcp + cerebro-api over one SQLite file as the real deployment shape; foreign commits only reach the API's graph via POST /recall or POST /associate (the latter a minor addition to the finding's wording), so GET graph routes serve a stale snapshot silently.

### R-13 [dream] Episode-schema pass has no dedup — Python reinforces an existing matching schema instead of re-creating

`crates/cerebro/src/engines/dream.rs:606`

Schema formation pass (a) (episodes → principles, dream.rs:597-678) stores a new Schematic memory for every episode with >=2 members and no dedup of any kind — Python checks `find_matching_schemas(tags)` first and reinforces the existing schema instead of creating a duplicate (dream.py:507-515). The Rust skill-distillation pass (b) does have a prefix dedup against existing schemas (line 737), so this is an omission specific to pass (a). Combined with the missing unconsolidated gating (previous finding), the same recent episodes mint near-identical schemas every cycle — and since the LLM rewords each time, even a prefix check would only partially help; the reinforce-existing pathway is what is missing. Python's schema promotion/demotion sweep (`evaluate_schema_candidates`, dream.py:545-548) is also unported.

**Failure scenario:** An agent with 5 stable recent episodes dreams nightly with an API key set: each cycle burns up to 2 schema-budget LLM calls on already-consolidated episodes and stores reworded duplicate schemas; after a month the schematic layer holds dozens of near-identical 'principles', diluting find_matching_schemas results.

**Verification note:** Confirmed. Rust dream.rs:597-678 episode-schema pass creates a new Schematic node per >=2-member episode with no dedup: no find_matching_schemas check, no reinforce path, no evaluate_schema_candidates sweep (zero hits in the Rust crate outside the MCP query tools), while pass (b) has its prefix dedup at :735-739 — so the omission is specific to pass (a). Python dream.py:506-515 checks find_matching_schemas(tags) and calls reinforce_schema per memory instead of creating, and :544-548 runs evaluate_schema_candidates; all three methods exist in neocortex.py (:98/:156/:216).

### R-14 [dream] skill_competition never clears prune_candidate on recovery — a re-crowned or no-longer-dominated procedure still gets pruned

`crates/cerebro/src/engines/dream.rs:1100`

The Demote arm sets `prune_candidate` at the salience floor (dream.rs:1116-1120), and the pruning phase retires any flagged memory older than 48h unconditionally — 'the flag IS the decision' (dream.rs:1187-1189). But neither the Champion arm (1100-1106) nor the Leave arm (1124-1131) removes a stale `prune_candidate` tag, even though Leave carefully removes a stale `skill_champion` tag. The only clear path is a recorded success (dispatch.rs:1174). The COMPETITION_PENALTY comment (dream.rs:104-108) promises 'room to recover if its win/loss record improves before then', but a procedure whose relative standing recovers without a new graded outcome of its own (e.g. the old champion's record deteriorates, closing the gap to within COMPETITION_MARGIN, or even making the flagged procedure the new champion) keeps the flag and is soft-deleted by phase 5 in the very same cycle — competition runs immediately before pruning.

**Failure scenario:** Procedure A was demoted to salience 0.25 and flagged while rival B dominated; B then fails repeatedly, so this cycle's verdict for A is Champion (or Leave). skill_competition stamps A `skill_champion` but leaves `prune_candidate`; minutes later the pruning phase soft-deletes the freshly crowned champion.

**Verification note:** Verified at the cited lines: the Demote arm (dream.rs:1116-1120) sets prune_candidate at the floor, the pruning phase (1187-1190) retires any flagged memory older than 48h unconditionally, and neither the Champion arm (1100-1106) nor the Leave arm (1124-1131) clears a stale flag — even though the Leave arm carefully clears a stale skill_champion tag, proving stale-marker hygiene was considered and this flag missed. compute_competition_verdicts (1717-1771) applies no salience/flag filter, so a flagged procedure with >=2 graded outcomes can be crowned Champion; competition runs immediately before pruning in the same cycle (267-271), so the freshly crowned champion is soft-deleted minutes later.

### R-15 [dream] Failed LLM calls consume no budget — a dead API key turns the cycle into an unbounded hammering run

`crates/cerebro/src/engines/dream.rs:541`

All six LLM loops increment `calls_used`/`budget_remaining` only in the Ok arm (e.g. dream.rs:484-487) and merely warn-and-continue on Err (541, 676, 790, 921, 989, 1350). Python decrements `_llm_calls_remaining` BEFORE the call and keeps the decrement on failure ('Was called, but failed', dream.py:799-803), so a broken key costs at most 20 attempts. In Rust, with a persistently failing API (401 invalid key, network outage, 429s), pattern extraction alone fires one doomed HTTPS round-trip per topical-tag cluster — potentially hundreds on a 500-memory brain — plus 50 episodes in schema formation, every refine/merge candidate in variation, and 10 REM pairs. Each attempt also constructs a fresh reqwest::Client (dream.rs:1368). The loops are all finitely bounded so this terminates, but the 20-call cap the constants promise does not hold for failures, and the phase reports still say success=true (the latter matches Python, so only the budget accounting is the divergence).

**Failure scenario:** ANTHROPIC_API_KEY is set but revoked; the nightly dream_run makes several hundred failed API requests over many minutes (each a full TLS handshake + 4xx), instead of giving up after the documented MAX_LLM_CALLS=20, and the report shows all phases 'success' with 0 llm_calls.

**Verification note:** Verified in dream.rs: all six LLM loops (lines 484/541, 622/676, 723/790, 919-925, 987-993, 1315/1350) consume budget only on Ok — Err arms warn-and-continue (or `continue` before the increments in variation), so loop break conditions on budget_remaining/calls_used never fire under persistent API failure. Python reference (src/cerebro/engines/dream.py:795-805, 440-442) decrements _llm_calls_remaining BEFORE the call and keeps the decrement plus phase_budget/report.llm_calls accounting on failure, capping a dead key at 20 attempts; Rust is bounded only by candidate counts (topical-tag clusters over 500 memories, 50 episodes, refine/merge candidates, 10 REM pairs — the latter iteration-capped). Each attempt builds a fresh reqwest::Client with no timeout (line 1368). PhaseResult::new defaults success:true (1848) and nothing flips it, so the report shows all-success with llm_calls:0.

### R-16 [mcp-wire] session_save/session_recall wire contract diverges from the Python drop-in (session_summary vs content)

`crates/cerebro-mcp/src/dispatch.rs:525`

Python's session_save requires `session_summary` and accepts key_discoveries/unfinished_business/if_disoriented (mcp_server.py:368-381); Python's session_recall takes lookback_hours/priority_filter/limit with nothing required (382-395). Rust requires `content` (dispatch.rs:525) and requires `query` on session_recall with params named priority/top_k (dispatch.rs:549-554). Not in the documented deliberate-deviation list, and CLAUDE.md's own mandated session protocol shows session_save(session_summary=..., key_discoveries=[...]). Tell-tale: audit_details (dispatch.rs:168) falls back to args["session_summary"] — dead code, since a session_save call carrying session_summary instead of content errors before audit runs.

**Failure scenario:** A caller following the Python contract (or CLAUDE.md's session-ritual snippet) calls session_save(session_summary:"...", key_discoveries:[...]) — the Rust server returns -32602 "content is required" and the session note is never stored; scripted session_recall({lookback_hours:168}) likewise fails with "query is required". The drop-in swap breaks the mandated session ritual until callers re-learn the schema.

**Verification note:** Verified on both sides: Rust dispatch.rs:525/549 requires content/query (tools.rs schemas match), while Python mcp_server.py:368-395 requires session_summary (with key_discoveries/unfinished_business/if_disoriented assembled by the handler) and session_recall requires nothing, using lookback_hours/priority_filter/limit time-window semantics. No aliasing exists — the only session_summary reference in the Rust workspace is the audit_details fallback at dispatch.rs:168, which is confirmed dead code (audit runs only on Ok results, and the call errors first).

### R-17 [mcp-wire] purge_all_deleted drops Python's `older_than_days` guard — silently over-purges

`crates/cerebro-mcp/src/dispatch.rs:601`

Python's purge_all_deleted advertises older_than_days (default 30) and only hard-deletes trash older than the threshold (mcp_server.py:661-670, 2288-2292). The Rust schema (tools.rs:282-286) advertises no parameters and the handler (dispatch.rs:601-606) purges every soft-deleted memory unconditionally — older_than_days, if passed, is silently ignored. Rust's own schema is at least self-consistent, but this is an undocumented divergence on an irreversible destructive tool.

**Failure scenario:** A Python-habituated caller runs purge_all_deleted(older_than_days:30) intending routine trash rotation while keeping the recent restore window: Rust ignores the argument and permanently destroys memories soft-deleted seconds ago, eliminating the list_deleted/restore_memory safety net with no warning in the response.

**Verification note:** Verified against both codebases. Rust: tools.rs:282-286 advertises an empty inputSchema; dispatch.rs:601-606 reads only agent scope, silently ignoring older_than_days; sqlite.rs:1075-1109 deletes on `deleted_at IS NOT NULL` with no age predicate (repo-wide grep for older_than_days in Rust code: zero hits). Python: mcp_server.py:661-670/2288-2292 defaults older_than_days=30 and cortex.py:1001-1025 enforces the cutoff in both agent-scoped and global paths. The divergence is worse than claimed: even a no-argument call differs — Python keeps the last 30 days of trash, Rust empties it entirely, destroying the list_deleted/restore window for fresh deletions.

### R-18 [mcp-wire] Nine mutating tools honor an undocumented `agent_id` scope their schemas omit

`crates/cerebro-mcp/src/tools.rs:285`

restore_memory (tools.rs:258), purge_memory (:270), purge_all_deleted (:282), bulk_delete (:288), prune_thread (:402), delete_tag (:440), rename_tag (:452), merge_tags (:465), and share_memory (:335) all call agent_scope(args) in dispatch (e.g. dispatch.rs:585, 595, 602, 614, 727, 757, 768, 780, 661) but none of their inputSchemas advertise agent_id. Python advertises agent_id ("Agent ID for access check") on its equivalents. Schema-reading callers cannot discover the scoping; conversely, the house convention of passing agent_id everywhere silently changes the blast radius of these destructive operations.

**Failure scenario:** FORGE, following the project rule 'pass agent_id to any Cerebro tool that accepts it', calls purge_memory(memory_id, agent_id:"FORGE") on a memory owned by another agent: the hidden scope filter makes the purge miss and the tool returns {"purged": false} with no explanation — while the schema gave no hint the extra argument would alter behavior at all.

**Verification note:** Verified all nine claims: the schemas in crates/cerebro-mcp/src/tools.rs (restore_memory :258, purge_memory :270, purge_all_deleted :282, bulk_delete :288, share_memory :335, prune_thread :402, delete_tag :440, rename_tag :452, merge_tags :465) omit agent_id, while their dispatch.rs arms (:586, :595, :602, :614, :661, :727, :757, :768, :780) all call agent_scope(args), which honors args["agent_id"] as a visibility filter. The failure scenario is mechanically real: sql_filter() for an agent scope is (visibility='shared' OR (visibility='private' AND agent_id=?)), purge_memory's pre-check returns Ok(false) for another agent's private memory, and dispatch returns {"purged": false} with no explanation; no argument validation strips the unadvertised param.

### R-19 [mcp-wire] find_path advertises `agent_id` but the handler ignores it entirely

`crates/cerebro-mcp/src/dispatch.rs:474`

tools.rs:181 advertises agent_id on find_path, but the handler (dispatch.rs:474-488) never calls agent_scope and runs the path search over the global in-memory graph, which contains every agent's non-deleted memories (graph rebuild loads all). Python's find_path advertises no agent_id at all (mcp_server.py:470-479) — the Rust schema added a parameter it never wired. Pure accept-and-ignore on an advertised param; contrast memory_neighbors and common_neighbors, which do scope their results.

**Failure scenario:** An agent calls find_path(source_id, target_id, agent_id:"FORGE") expecting agent-scoped results; the returned path includes the memory IDs of other agents' private memories as intermediate hops, disclosing their existence and linkage structure across the supposed visibility boundary.

**Verification note:** Verified: tools.rs:181 advertises agent_id on find_path but the handler (dispatch.rs:474-488) never calls agent_scope and path-searches the global petgraph, which rebuild_from_db (graph.rs:28-44) populates with ALL agents' non-deleted memories — returned path IDs are unfiltered, so other agents' private memory IDs and linkage can be disclosed as hops. Sibling arms memory_neighbors (dispatch.rs:464) and common_neighbors (495) do call agent_scope and filter via get_memories_by_ids, confirming the inconsistency. Python reference (src/cerebro/interfaces/mcp_server.py:470-479, handler 2029-2033) advertises no agent_id at all — the Rust schema added an accept-and-ignore param, a pattern this project explicitly treats as a defect (see the ingest_file session_id note in CLAUDE.md). Not in Deferred/TBD, not in BACKPORT-FROM-APEXOS.md, and no dispatch-level test covers it.

### R-20 [parity] Thread visibility is broken in both directions: SQL scope filter drops Python's thread clause, while can_access grants Thread nodes to every agent

`crates/cerebro/src/types.rs:161`

Python's _scope_sql (cortex.py:57-76) for an agent includes `OR (visibility='thread' AND agent_id=?)` (plus a conversation_thread match when present), and _can_access (cortex.py:212-215) grants THREAD only on matching thread or ownership. Rust sql_filter (types.rs:158-164) emits only `(visibility='shared' OR (visibility='private' AND agent_id=?))` — no thread clause at all — while can_access (types.rs:149) returns unconditional `true` for Visibility::Thread with the comment 'thread_id checked separately', but nothing downstream checks it. Thread memories are creatable via update_memory visibility:"thread" (dispatch.rs:405-419) and arrive via Python-DB auto-migration.

**Failure scenario:** A Python cerebro.db with visibility='thread' rows is migrated to the Rust binary. The owning agent's recall (agent_id set) can no longer return its own thread memories — get_memories_by_ids' sql_filter excludes them — so they silently vanish from results. Meanwhile, during recall's spreading step the can_access=true path marks those same thread nodes visible for ANY agent, so another agent's spread is influenced by (conducts activation through) thread memories Python would have hidden from it.

**Verification note:** Verified against both codebases. Rust sql_filter (types.rs:154-165) emits no thread clause for agent scopes while Python _scope_sql (src/cerebro/cortex.py:65-75) grants thread rows to the owner (and to matching conversation_thread); Rust can_access (types.rs:149) returns unconditional true for Visibility::Thread and grep confirms nothing downstream ever checks thread_id — the 'checked separately' comment is aspirational. The scenario is reachable: Python's memory_store alias creates visibility='thread' rows, the auto-migration copies visibility verbatim (MIGRATION_SQL), and Rust update_memory sets Thread (dispatch.rs:405-419).

### R-21 [parity] Ranking FSRS retrievability pinned at 1.0 for never-recalled memories (Python decays it from last access)

`crates/cerebro/src/engines/prefrontal.rs:54`

rank_results computes elapsed_days from strength.last_review with `.unwrap_or(0.0)` (prefrontal.rs:54-57), so any memory that has never been through record_recall_review gets retrievability(0.0, S) = 1.0 forever. Python's compute_current_retrievability (decay.py:16-24) measures elapsed time from the last access timestamp — and since Python records a first access at store time, R decays from creation. Note the inconsistency inside the Rust port itself: record_recall_review (models/memory.rs:78) correctly falls back to created_at, but the ranking path falls back to zero elapsed. The 41 activation fixture tests cover the pure functions, not this wiring.

**Failure scenario:** Memory A stored 60 days ago, never recalled (S=1.75): Python R≈0.23, Rust R=1.0 — a +0.15 score inflation at SCORE_WEIGHT_RETRIEVABILITY=0.20. Stale never-touched memories systematically outrank recently-recalled ones on the FSRS term, inverting the forgetting-curve ordering the ranking is supposed to encode; recall returns different top-k than the Python reference on the same data.

**Verification note:** Verified at every level. (1) prefrontal.rs:54-57 does exactly what's claimed: elapsed_days from strength.last_review with .unwrap_or(0.0), and fsrs.rs:13 returns 1.0 for elapsed <= 0. (2) The scenario is reachable: a repo-wide grep shows last_review is written ONLY by record_recall_review, called solely in recall's top-k reinforcement (cortex.rs:476); MemoryNode::new sets last_review=None and the store/dedup paths never stamp it, so never-recalled memories rank with R=1.0 forever. Python's remember() records a first access at store (cortex.py:322) and compute_current_retrievability (decay.py:16-24) decays from max(access_timestamps), giving R≈0.21 at 60d/S=1.75 vs Rust 1.0 — ~+0.16 score inflation at weight 0.20 (config.rs:48).

### R-22 [parity] Recall-time Hebbian strengthening (strengthen_co_activated) is missing — link weights never learn from co-recall

`crates/cerebro/src/cortex.rs:466`

Python recall step 5 (cortex.py:677-680) calls links.strengthen_co_activated(result_ids), which bumps the weight of every existing link between co-recalled top-k results by +0.05 (association.py:150-176). Rust recall has no equivalent: it stamps last_traversed/traversal_count on walked links (record_traversals, cortex.rs:432-437 — the documented deviation) and reinforces ACT-R/FSRS node strength (cortex.rs:473-492), but link WEIGHTS are never increased at recall time. The insert_link ratchet only fires on re-assertion at encoding, and dream SWS replay only strengthens temporal links between co-episode memories — neither replaces the recall-time Hebbian loop. CLAUDE.md documents the traversal-stamping deviation but says nothing about dropping weight strengthening.

**Failure scenario:** Two memories repeatedly co-recalled over weeks: in Python their connecting link climbs from 0.4 toward 0.9, so spreading activation increasingly binds them; in Rust the weight stays at its encoding value while the 30-day link decay erodes effective conductance, so frequently co-used paths spread progressively less activation than Python and the associative network never consolidates from use.

**Verification note:** Verified: Python recall step 5 (cortex.py:677-680) calls strengthen_co_activated(result_ids), which bumps every existing link between co-recalled top-k pairs by +0.05 (association.py:150-176, graph_store.py:563 strengthen_link with MIN(weight+boost, 1.0)). Rust recall (cortex.rs:423-494) has no equivalent — record_traversals (sqlite.rs:970-991) only stamps last_traversed/traversal_count, never weight, and git log -S finds strengthen_co_activated was never ported or mentioned. Not a documented deviation: the CLAUDE.md/spreading.rs:113-118 deviation note covers traversal stamping only; nothing in Deferred/TBD or any doc covers dropping weight strengthening.

### R-23 [parity] Layer promotion and decay sweeps are entirely absent — memories never advance sensory→working→long_term→cortex *(independently found by Dream engine)*

`crates/cerebro/src/engines/dream.rs:1162`

Python's ExecutiveEngine has check_and_promote/run_promotion_sweep (prefrontal.py:100-135) driven by check_promotion_eligibility (decay.py:38-72, LAYER_CONFIG access-count/age thresholds), and run_decay_sweep (prefrontal.py:212-235); the dream pruning phase runs both sweeps before pruning (dream.py:631-635). The Rust ExecutiveEngine (prefrontal.rs) contains only rank_results, and the Rust dream pruning phase (dream.rs:1162-1210) goes straight to pruning — grep finds no layer-promotion code anywhere in the workspace. LAYER_CONFIG itself was never ported (config.rs has no equivalent). Not listed in CLAUDE.md's Deferred/TBD section.

**Failure scenario:** A sensory-layer memory accessed 10 times over a week: Python promotes it to working at 2 accesses (then long_term at 5), moving it out of the dream pruner's sensory-only target set. In Rust it stays sensory forever; once it is >48h old with salience <=0.3 and happens to be link-isolated, the pruning phase soft-deletes a memory the Python system would have protected by promotion. All layer-based semantics (activation_at_risk layers, working-memory views) also stay frozen at encoding-time values.

**Verification note:** Verified against both codebases. Python has the full promotion/decay machinery: LAYER_CONFIG (config.py:104-129, sensory→working at 2 accesses, working→long_term at 5 accesses + 24h), check_promotion_eligibility (decay.py:36-70), check_and_promote/run_promotion_sweep (prefrontal.py:101-136) and run_decay_sweep (prefrontal.py:212-235), all invoked by the dream pruning phase before pruning (dream.py ~630-635). The Rust side has none of it: ExecutiveEngine (prefrontal.rs) contains only rank_results, dream.rs pruning (1162-1230) goes straight to prune, and workspace-wide grep finds no promotion/decay-sweep/LAYER_CONFIG code.

### R-24 [parity] Graph exploration is outgoing-only where Python is undirected: find_path, common_neighbors, memory_neighbors miss incoming links

`crates/cerebro/src/engines/association.rs:49`

Python treats the associative network as undirected for exploration: find_path uses igraph mode="all" (association.py:247) and GraphStore.get_neighbors defaults to direction="all" (graph_store.py:585-601). Rust uses a directed petgraph (storage/graph.rs:10, Graph::new()) and LinkEngine::find_path BFS iterates graph.neighbors(curr) — outgoing edges only (association.rs:49); get_common_neighbors is explicitly documented as 'outgoing neighbors' (association.rs:73-93); the memory_neighbors tool uses GraphStore::neighbors, also outgoing-only (graph.rs:95-104, dispatch.rs:461-472). Spreading activation correctly walks both directions (spreading.rs:159), so this drift is confined to the exploration tools — but auto-links are always stored new-memory→partner, so incoming links are the majority direction for older memories.

**Failure scenario:** Memory B was auto-linked from newer memory A (edge A→B). Python find_path(B, A) returns [B, A] and memory_neighbors(B) includes A; Rust find_path(B, A) returns None and memory_neighbors(B) omits A. On a real store where most links point newer→older, an older hub memory reports near-zero neighbors and paths that plainly exist in the network are reported as absent.

**Verification note:** Every element of the finding verifies. Rust: GraphStore uses a directed petgraph (graph.rs:10,23) and all three exploration paths use outgoing-only neighbors() — find_path BFS (association.rs:49, doc even says "directed path"), get_common_neighbors (association.rs:73-93, doc says "outgoing neighbors"), and the memory_neighbors tool via GraphStore::neighbors (graph.rs:95-104, dispatch.rs:461-472). Python: find_path uses igraph get_shortest_path(mode="all") (association.py:247), get_neighbors defaults to direction="all" (graph_store.py), and the memory_neighbors/common_neighbors handlers use those defaults — undirected exploration in all three.

### R-25 [storage] find_by_any_tag / find_by_any_concept strip '_' and '%' from LIKE patterns, so underscore tags never match — auto-association silently disabled for them

`crates/cerebro/src/storage/sqlite.rs:589`

The candidate query builds patterns via t.replace(['"', '%', '_'], "") — deleting LIKE metacharacters instead of escaping them. A stored tag ["bug_fix"] can never match pattern %"bugfix"%, so remember()'s tag-based auto-link pass (cortex.rs:246) finds zero candidates for any tag containing an underscore — a pervasive convention in this ecosystem (session_note, bug_fix, memory_system, ...). find_by_any_concept (line 633) has the same defect. The correct fix already exists in the same file: find_by_tags (line 890) escapes with backslash + ESCAPE '\'. This is a Rust-only regression vs Python's plain LIKE and is not among the documented deliberate deviations.

**Failure scenario:** A memory tagged ["error_handling", "retry_logic"] is stored; five older memories share those exact tags. find_by_any_tag returns nothing (patterns "errorhandling"/"retrylogic" match no stored JSON), so no semantic links are created — the memory lands as a graph island, invisible to spreading activation, and memory_health's isolated_memories count silently climbs.

**Verification note:** Confirmed at sqlite.rs:589/:633 — patterns are built with t.replace(['"','%','_'], ""), deleting LIKE metacharacters instead of escaping. Empirically verified in SQLite: stored tags JSON ["bug_fix"] is never matched by the stripped pattern %"bugfix"% (0 rows), while the escaped form used by find_by_tags (line 890, ESCAPE '\\') and Python's raw pattern both match. Callers cortex.rs:246/:273 are remember()'s auto-link pass; is_structural_tag only filters a fixed bookkeeping list so user underscore tags flow through, and the concept tokenizer (temporal.rs:87) deliberately keeps '_' in words, so snake_case concepts are actively produced and then never match. The cerebro autolink retrofit uses the same path. Not documented as deliberate: FINDINGS-1.md lists exactly two deviations (bookkeeping-tag exclusion, JSON-quoted match) — stripping is unremarked; CLAUDE.md Deferred/TBD is silent.

### R-26 [tests-docs] GraphStore::add_edge in-place update (parallel-edge double-count fix) has no regression test

`crates/cerebro/src/storage/graph.rs:69`

Of the four colony field fixes from wave 5, three have direct regression tests (insert_link ratchet: sqlite.rs:2760 link_reassertion_ratchets_instead_of_wiping; traversal stamping: integration_test.rs:602 recall_stamps_walked_links; thalamus dedup: integration_test.rs:641). The fourth — add_edge finding an existing (source, target, link_type) edge and updating it in place instead of letting petgraph store a parallel edge that double-counts spreading conductance — is guarded by nothing. No test anywhere adds the same edge triple twice to a GraphStore and asserts edge_count stays 1 or that the in-graph weight ratchets (verified by grepping all add_edge/edge_count call sites in tests; the spreading fixtures use unique edges, and no test calls associate twice on the same pair).

**Failure scenario:** A refactor reverts add_edge to plain self.graph.add_edge(src, tgt, link) (its pre-wave-5 body). All 250 tests stay green. Re-asserted links again create parallel petgraph edges, spreading activation double-counts their conductance, and recall rankings silently skew — the exact bug the colony's four nodes cross-checked and fixed.

**Verification note:** Confirmed. graph.rs:69-93 contains the in-place dedup exactly as claimed, and exhaustive inspection of every GraphStore::add_edge call site in test code refutes nothing: the spreading fixtures (all 7 cases in tests/fixtures/activation.json programmatically checked) use unique (source,target,link_type) triples; association.rs make_graph edge lists are all unique; the graph rebuild tests insert single links (and refresh_graph is a full rebuild fed by SQLite's upsert, so rebuilds can never surface duplicates); spreading.rs unit tests bypass GraphStore via raw petgraph; every associate call in tests hits each pair once. The three sibling wave-5 tests exist as cited (sqlite.rs:2760, integration_test.rs:602, :641) and none touches this path — the sqlite ratchet test never builds a graph.

### R-27 [tests-docs] ingest_csv adapter is untested end-to-end — only the delimiter sniffer has a test

`crates/cerebro/src/ingest.rs:487`

The CSV adapter's two documented behaviors — row-per-memory conversion (header:value pairs joined with ' | ', empty fields filtered) and the CSV_ROW_THRESHOLD=200 switchover to a single schema-summary memory with the 'schema' tag — have zero coverage. The ingest.rs unit tests (line 766) cover only sniff_delimiter, and neither the cerebro-mcp dispatch tests (which do cover markdown sections, JSON records, and honest errors at dispatch.rs:2446-2568) nor the integration suite ever writes a .csv file and ingests it. Contrast with the JSON adapter, whose per-record type/tags/salience honoring is asserted at dispatch.rs:2528-2538.

**Failure scenario:** A change to store_chunks or ingest_csv breaks the 200-row switchover (e.g., an off-by-one makes a 5000-row CSV store 5000 memories instead of one summary, flooding the store and the auto-link pass) or drops the header zip — no test fails, and the bug ships to the Pi where ingest_file is a user-facing tool.

**Verification note:** Verified: ingest_csv (ingest.rs:487-540) implements both documented behaviors exactly as claimed (row-per-memory 'header: value' join with ' | ' and empty-field filter; CSV_ROW_THRESHOLD=200 schema-summary switchover with 'schema' tag), and repo-wide grep confirms zero end-to-end coverage — only sniff_delimiter (ingest.rs:766) and the classify routing assertion (ingest.rs:694) touch CSV. Dispatch tests at 2446/2504/2541 cover markdown/JSON/errors but never a .csv; JSON's per-record assertions at 2528-2538 exist verbatim, confirming the contrast. The failure scenario holds: a threshold or header-zip regression passes the whole suite (thalamus dedup only blocks exact duplicates, so 5000 distinct rows would flood the store), and ingest_file is a live user-facing tool whose CSV behavior is advertised in its tool description (tools.rs:926).


---

## LOW severity

### R-28 [api-cli] API /procedures listing omits the undo_snapshot exclusion and min_salience floor

`crates/cerebro-api/src/main.rs:767`

MCP `list_procedures` (dispatch.rs:1033-1054) filters out `undo_snapshot`-tagged nodes — evolution rollback artifacts that get mis-typed Procedural and 'dominate by access count' — and supports `min_salience`. The API's list_procedures (main.rs:767-780) and the CLI's ProcedureCmd::List (cerebro-cli main.rs:772-790) apply neither filter, so the two surfaces disagree on what the procedure library contains. (Full-node vs wire_summary row shape is the documented MCP deviation and not counted here; the undo_snapshot exclusion is a correctness filter, not a wire-economy one.)

**Failure scenario:** On an ApexOS node where the evolution applier has stored soul.md undo snapshots, GET /procedures returns a list dominated by rollback records that the MCP tool correctly hides — a dashboard consumer sees phantom 'procedures' and different counts than the agent does.

**Verification note:** Verified in code: MCP list_procedures (dispatch.rs:1034/1045/1050) applies both min_salience and the undo_snapshot exclusion; the API route (cerebro-api main.rs:767-780) and CLI (cerebro-cli main.rs:772-790) apply neither — grep confirms both terms absent from those crates entirely. Not a documented deviation: BACKPORT-FROM-APEXOS.md:166-168 scopes the backport to the two MCP dispatch hunks and explicitly calls the filter read-time healing of existing polluted stores, with no decision recorded to exempt API/CLI; nothing in CLAUDE.md Deferred/TBD covers it. The failure scenario holds: the agentd-side episodic-typing fix is ApexOS-only and prospective, so existing mis-typed Procedural soul snapshots in a shared DB surface via GET /procedures while the MCP tool hides them. No tests cover it (cerebro-api/cerebro-cli have no test dirs).

### R-29 [dream] A phase that returns Err is reported as phase "unknown"

`crates/cerebro/src/engines/dream.rs:1852`

run_cycle maps a phase Err into `PhaseResult::failed(&e.to_string())` (dream.rs:277-280), and `failed()` constructs the result with `Self::new("unknown")` (1852-1857) — the phase name is lost, leaving only array position to infer which of the 8 phases failed. Python each phase catches its own exceptions inside the phase function and keeps its `phase` enum on the report. Also, on early Err the phase's partial counters (e.g. links strengthened before a storage error in sws_replay) are discarded entirely.

**Failure scenario:** A storage error in emotional_reprocessing surfaces in dream_status as `{"phase": "unknown", "success": false, "notes": "..."}`; the operator reading the journal cannot tell which phase failed without counting array positions against the source.

**Verification note:** Confirmed at dream.rs:277-280 and 1852-1857: run_cycle maps a phase Err into PhaseResult::failed(), which constructs Self::new("unknown") — the phase name is lost and partial counters are discarded. All 8 phases return Result<PhaseResult> and propagate storage errors with `?` (e.g. emotional_reprocessing lines 819/824), so the scenario is reachable; the report persists via save_dream_report and dream_status returns it verbatim, so the operator sees phase:"unknown". Python reference (dream.py, e.g. line 394) catches exceptions inside each phase and preserves the phase enum — this is a genuine unported behavior, not a documented deviation (nothing in CLAUDE.md Deferred/TBD or docs/), and no test covers the Err path.

### R-30 [dream] SWS replay only checks src→tgt for an existing link, duplicating reverse-direction links instead of strengthening

`crates/cerebro/src/engines/dream.rs:342`

For each consecutive episode pair, sws_replay looks up existing links via `list_links_from(&src)` and matches on `target_id == tgt` (dream.rs:339-346). A link that already exists in the opposite direction (tgt→src — e.g. created by REM recombination, whose random pair ordering is direction-blind, or by a manual associate) is not found, so a new src→tgt Temporal link at weight 0.1 is inserted, producing an antiparallel duplicate pair rather than strengthening the existing connection. REM itself uses the direction-blind `has_link_between` (line 1303-1305) for exactly this reason, and Python's phase 1 strengthens co-activated links undirected via `strengthen_co_activated`. Impact is mild link-table duplication and split weight between the two directions; the graph backport already prevents parallel same-direction edges from double-counting, but not antiparallel ones.

**Failure scenario:** REM created B→A (semantic, 0.6) last night; tonight's SWS replay of the episode containing A then B misses it, inserts A→B (temporal, 0.1), and future cycles strengthen the new duplicate while the stronger original decays untouched.

**Verification note:** Confirmed at dream.rs:339-346: sws_replay checks only list_links_from(src) matched on target_id==tgt, so a reverse (tgt→src) link — creatable by REM's randomly-ordered pairs or a manual associate — is missed and a new src→tgt Temporal 0.1 link is inserted. insert_link's ON CONFLICT(source_id,target_id,link_type) does not collide with the reverse row, GraphStore::add_edge dedupes only same-direction edges, and spreading.rs walks edges in both directions, so the antiparallel pair double-counts conductance. REM itself uses direction-blind has_link_between (sqlite.rs:2326), and Python's reference strengthen_co_activated (association.py:150-176) checks both directions — confirming direction-blindness is the intended convention. No documented deviation in CLAUDE.md/docs, and the only phase-1 test (integration_test.rs:1670) checks episode counts, not link direction.

### R-31 [mcp-wire] Argument-validation errors that break the error_code() 'required' invariant map to -32603

`crates/cerebro-mcp/src/dispatch.rs:1436`

error_code() (dispatch.rs:177-192) documents the invariant that every argument-validation error is phrased with the word 'required' so it maps to -32602. Three validation paths violate it: search_vision's missing-input message "provide `query` (text) or `path`/`b64` (an image)" (dispatch.rs:1436-1438), update_memory's "unknown visibility '...' " bail (dispatch.rs:410), and remember's thalamus rejection (cortex.rs:102-106) — all surface as -32603 internal error instead of -32602 invalid params.

**Failure scenario:** A client calls search_vision({}) — a plain missing-argument mistake — and receives -32603 "internal error" semantics; monitoring that alerts on internal-error rates, or a client that retries on -32603 (server fault) but not on -32602 (caller fault), misclassifies routine bad calls as server failures.

**Verification note:** Verified: error_code() (dispatch.rs:183-192) documents the 'required' phrasing invariant for -32602, and search_vision's missing-input message (dispatch.rs:1436-1438, no 'required') plus update_memory's visibility bail (dispatch.rs:410) are argument-validation errors inside route() that map to -32603. The failure scenario traces end-to-end: dispatch_tool converts route() Err via error_code with no upstream gate, so search_vision({}) returns -32603. The analogous describe_image path is deliberately phrased 'is required' and has a test asserting -32602 (dispatch.rs:2572-2585), proving intent; no test covers the violating paths. Not a documented deviation — ACTIONPLAN-REPORT.md marks C-RS-006 'Fixed: -32602 for arg-validation'.

### R-32 [mcp-wire] Stale tool-count comment: '66 tool names (62 functional + 4 deferred stubs)' above a 67-name, zero-stub registry

`crates/cerebro-mcp/src/tools.rs:949`

The doc comment over TOOL_NAMES (tools.rs:949-950) still says "All 66 tool names (62 functional + 4 deferred Tier-7 stubs)" while the array holds 67 entries, all routed (verified: every name has both a schema arm and a dispatch arm; the tools_list test at dispatch.rs:2103 asserts 67, and main.rs:14 says 67). The advertised wire count is honest — only the comment at the registry itself is wrong, which is exactly where a future maintainer will read the count.

**Failure scenario:** A contributor adding tool 68 trusts the header, miscounts the delta, and 'fixes' the tools_list assertion to the wrong number — or goes hunting for the four deferred stubs the comment claims exist, which were all shipped.

**Verification note:** Verified: tools.rs:949-950 doc comment says "All 66 tool names (62 functional + 4 deferred Tier-7 stubs)" but the TOOL_NAMES array below it holds exactly 67 entries, all functional (the "4 stubs" it references — cognitive_bootstrap, ingest_file, describe_image, search_vision — are all shipped; the fallthrough comment at tools.rs:938-940 in the same file says "nothing is deferred anymore"). main.rs:14, dispatch.rs:1453, the tools_list test asserting 67 (dispatch.rs:2103), and CLAUDE.md ("67/67 wired") all agree the comment is the sole stale count. Not a documented deviation, and no test can cover a comment. Comment-only defect with honest wire behavior, so severity stays low.

### R-33 [parity] recall drops Python's context_ids activation seeding and memory_types / min_salience filters

`crates/cerebro-mcp/src/dispatch.rs:321`

Python's recall accepts context_ids (seeded into spreading activation at weight 0.8, cortex.py:591-596), memory_types, min_salience, and offset (cortex.py:527-538; MCP surface mcp_server.py:1483-1495). Rust recall exposes only query/top_k/agent_id/visibility (tools.rs:40-53) and cortex::recall(query, k, scope) has no plumbing for any of them (cortex.rs:357-362). The schema is at least honest (params not advertised, so no accept-and-ignore), and C-RS-014 defers wire-SHAPE parity — but missing parameters are input-surface parity, not wire shape, and context-seeded spreading is a genuine pipeline step Python has that Rust lacks. Not in the documented-deviations list.

**Failure scenario:** An agentd caller ported from the Python server passes context_ids to bias recall toward the current working set, or memory_types:["procedural"] to filter: the Rust server ignores context (no seeding, different activation results) and returns all types unfiltered, forcing client-side filtering that changes which k results survive versus the Python reference.

**Verification note:** Verified against both codebases. Rust recall (dispatch.rs:321-340) reads only query/top_k/agent_id/visibility, the advertised schema (tools.rs:40-53) matches, and cortex::recall(query, k, scope) (cortex.rs:357-362) has no plumbing for anything else — the memory_search alias is equally bare. Python's cortex.recall accepts memory_types/min_salience/context_ids/conversation_thread/explain/offset, seeds context_ids into spreading activation at weight 0.8 (cortex.py ~590-596), and its MCP surface (mcp_server.py:141-163 schema, 1482-1499 handler) advertises and forwards memory_types, min_salience, context_ids, conversation_thread, and explain.

### R-34 [parity] Store-time access is a bare timestamp: access_count stays 0 and the first FSRS review is skipped (Python records a full access at encoding)

`crates/cerebro/src/models/memory.rs:46`

Python remember() runs record_access at store time (cortex.py:316-324): access_count becomes 1 and FSRS updates difficulty (5.0 → ~4.64 via update_difficulty_on_recall with R=1). Rust MemoryNode::new seeds access_times=[now] but leaves access_count=0 and never runs the FSRS first-review (memory.rs:45-47); the dedup re-encounter path does call record_access but skips record_recall_review (cortex.rs:120-135). Net: access_count is persistently one lower than Python for the same history and difficulty starts at 5.0 instead of ~4.64, slightly shifting FSRS stability growth on the first real recall. Mostly cosmetic today because the access-count-driven promotion machinery is unported (see the promotion finding), but it becomes an off-by-one against Python thresholds the day promotion lands.

**Failure scenario:** Same memory, same one-recall history: Python reports access_count=2, difficulty≈4.35; Rust reports access_count=1, difficulty≈4.71 — visible in wire_node output and any consumer comparing against Python-generated data, and a latent threshold off-by-one for LAYER_CONFIG promotion counts (2/5) if that sweep is ever ported.

**Verification note:** Verified against both codebases. Rust MemoryNode::new (memory.rs:45-47) seeds access_times=[now] but leaves access_count=0 and default FSRS strength (D=5.0, last_review=None), and Cortex::remember never records an access on a new node. Python remember() (src/cerebro/cortex.py:316-324) calls record_access at encoding, which in one function bumps access_count to 1 AND runs the FSRS updates — difficulty 5.0 → 4.64 exactly as claimed (R=1 at t=0; stability unchanged since the increment is 0 at R=1). Python's dedup path (storage/coordinator.py:91-99) also runs full record_access including FSRS, while Rust's dedup path (cortex.rs:120-135) bumps count/timestamps but skips record_recall_review.

### R-35 [storage] Python migration indexes soft-deleted rows into FTS5; later restore corrupts the FTS index, later purge orphans entries

`crates/cerebro/src/storage/sqlite.rs:292`

migrate_from_python copies Python trash rows with deleted_at intact (MIGRATION_SQL line 343) and then runs INSERT INTO memories_fts(memories_fts) VALUES('rebuild'), which indexes ALL content-table rows including soft-deleted ones (the memories_ai trigger at line 2542 also has no new.deleted_at guard). This breaks the CB-020 invariant that soft-deleted rows are absent from FTS, which memories_au/memories_ad rely on. Verified empirically: after rebuild, MATCH finds the trash row's content; after restore_memory (deleted_at -> NULL), memories_au inserts the same rowid a second time and the FTS 'integrity-check' command fails with 'database disk image is malformed'; conversely a purge of a migrated trash row skips the FTS 'delete' (memories_ad guard) leaving an orphaned entry whose freed rowid can be reused by a new memory, making queries on old trash tokens return the wrong memory.

**Failure scenario:** Andre points CEREBRO_DATA_DIR at the Python cerebro.db (the blessed onboarding flow). The Python DB has soft-deleted rows. After migration, restore_memory on one of them silently corrupts the FTS index (duplicate rowid entries), and purge_all_deleted on the rest leaves orphaned FTS entries; FTS keyword search (the fallback path and bm25 scoring) then returns corrupt-index errors or matches the wrong rows after rowid reuse.

**Verification note:** Partially real, headline mechanism refuted. CONFIRMED: MIGRATION_SQL (sqlite.rs:343) copies deleted_at intact, memories_ai (line 2542) has no deleted_at guard, and the external-content FTS5 'rebuild' (line 292) indexes the whole memories table — migrated Python trash rows land in the FTS index, breaking the CB-020 invariant the memories_ad/memories_au guards rely on; no test or doc covers this (step-12 migration fixtures contain only live rows). Also CONFIRMED empirically with the exact production SQL (vector.rs:302-305): purging a migrated trash row skips FTS eviction (memories_ad guard), leaving a permanent orphan; when the freed max rowid is reused by the next insert, keyword search returns the wrong memory for the old trash tokens.

### R-36 [storage] insert_memory's INSERT OR REPLACE orphans the old FTS entry (implicit DELETE fires no triggers) and fails on FK for linked/versioned rows

`crates/cerebro/src/storage/sqlite.rs:508`

OR REPLACE's implicit DELETE does not fire memories_ad because PRAGMA recursive_triggers defaults OFF, so overwriting an existing id leaves the old rowid's tokens in memories_fts while the row is reinserted under a new rowid. Verified empirically: after the replace, an FTS MATCH on the old content errors with 'database disk image is malformed (11)'. CB-005 explicitly anticipates this overwrite path and pre-deletes the stale memory_vectors row (line 500-506) but misses the FTS side entirely. Additionally, with foreign_keys=ON the implicit delete fails outright for a memory referenced by links or memory_versions. Latent today — the only production call site is cortex remember() with fresh UUIDs — but the method's own comments claim overwrite semantics it does not actually deliver.

**Failure scenario:** Any future caller (API import, federation sync, restore tooling) calls insert_memory with an existing id: if the row has links the insert errors; if not, the FTS index silently gains an orphaned entry and keyword searches touching the old content start failing with SQLITE_CORRUPT until an FTS rebuild.

**Verification note:** Core claim CONFIRMED empirically with the project's exact DDL: memories_fts is external-content FTS5 maintained only by triggers, PRAGMA recursive_triggers is never set (grep: 0 hits), so OR REPLACE's implicit DELETE skips memories_ad; reproduction showed rowid 1→2, orphaned FTS entry, and MATCH on old content failing with 'database disk image is malformed (11)'. CB-005 (sqlite.rs:494-506) pre-deletes only the stale memory_vectors row, proving the overwrite path is intended yet FTS is missed. Not documented in CLAUDE.md Deferred/TBD; no test covers same-id overwrite. HOWEVER the FK sub-claim is REFUTED: with foreign_keys=ON verified active (control DELETE fails with error 19), INSERT OR REPLACE of a linked row succeeds — SQLite defers FK checks for REPLACE-deleted rows to end-of-statement and the same-id reinsert satisfies them.

### R-37 [storage] Embedding writes are non-transactional across memories.embedding and the vec0 index, and there is no vec-index rebuild path to reconcile a desync

`crates/cerebro/src/storage/vector.rs:76`

embed_and_store / store_raw_embedding execute UPDATE memories SET embedding=... and the vec0 delete+insert (upsert_memory_vector, lines 371-387) as separate statements under the connection lock but not in a transaction; insert_memory's stale-vec DELETE + INSERT OR REPLACE pair (sqlite.rs:500-535) has the same shape. A crash/power-cut between the statements leaves embedding set but no memory_vectors row (or vice versa). Unlike the graph (rebuilt from SQLite at startup and on data_version staleness), memory_vectors is never rebuilt or reconciled — the backfill worklist list_missing_embeddings (sqlite.rs:721) only scans embedding IS NULL, so the desynced row is permanently invisible to vector KNN while looking fully embedded.

**Failure scenario:** Pi loses power mid-remember between the embedding UPDATE and the vec0 insert. That memory thereafter never appears in vector search results (and FTS fallback only engages when vec returns zero rows overall), with no error, no backfill candidate, and no repair short of manually nulling the embedding column.

**Verification note:** Confirmed at every cited site. embed_and_store/store_raw_embedding (vector.rs:76-101) run UPDATE memories SET embedding plus the vec0 DELETE+INSERT (upsert_memory_vector, vector.rs:371-387) as separate autocommit statements under the connection lock with no transaction — the code comment itself claims atomicity only "against other writers". insert_memory (sqlite.rs:500-535) has the same non-transactional DELETE + INSERT OR REPLACE shape, while purge/restore/traversal paths in the same file do use conn.transaction(), so this is a genuine omission. DB runs WAL + synchronous=NORMAL, so a power cut between statements durably persists the embedding UPDATE without the vec row.

### R-38 [storage] register_agent INSERT OR REPLACE clobbers registered_at on every re-registration

`crates/cerebro/src/storage/sqlite.rs:1317`

register_agent uses INSERT OR REPLACE with ?4 bound to now for both registered_at and last_seen, so re-registering an existing agent (a routine idempotent call at session start) rewrites its original registration timestamp and discards prior description/metadata distinctions. An ON CONFLICT(id) DO UPDATE SET last_seen=?, name=?, ... that preserves registered_at would keep the historical record intact.

**Failure scenario:** FORGE re-registers at every session start; list_agents always shows registered_at == the most recent session, erasing the agent's actual provenance date from the registry.

**Verification note:** Confirmed at crates/cerebro/src/storage/sqlite.rs:1316-1321: INSERT OR REPLACE binds ?4 (now) to both registered_at and last_seen, so re-registering an existing agent rewrites its original registration timestamp and wholesale-replaces description/metadata. register_agent is the only write path to the agents table and last_seen has no other update mechanism, making idempotent re-registration the natural "touch" pattern — and every touch clobbers provenance. Not a documented deviation (nothing in CLAUDE.md Deferred/TBD or docs/), and no test covers re-registration semantics.

### R-39 [storage] add_episode_step OR REPLACE re-appends memory_id to episodes.memory_ids on step replacement, accumulating duplicates in the stored JSON

`crates/cerebro/src/storage/sqlite.rs:1905`

Replacing an existing (episode_id, step_index) via INSERT OR REPLACE unconditionally runs json_insert(memory_ids, '$[#]', ?) again, so every re-submission of a step appends another copy of its memory_id to the episodes.memory_ids array. get_episode_memory_ids dedups on read (line 2044), but get_episode_raw (line 1959) returns the raw array, so the episode wire payload reports duplicate memory_ids and the JSON grows without bound under retries.

**Failure scenario:** An agent retries episode_add_step for step 3 after a timeout; the episode's memory_ids becomes ['mem_a','mem_b','mem_c','mem_c'], and get_episode consumers double-count the consolidated memory (e.g. dream SWS replay weighting an episode member twice).

**Verification note:** Confirmed at sqlite.rs:1898-1909: episode_steps has PRIMARY KEY (episode_id, step_index), so INSERT OR REPLACE replaces on retry while the json_insert(memory_ids,'$[#]',?) append runs unconditionally, duplicating the memory_id in episodes.memory_ids; dispatch.rs:887 passes caller step_index through (default 0, not required by the tool schema), so retries are easy to trigger, and get_episode/API/CLI return the raw duplicated array via get_episode_raw. However the finding's headline consequence is refuted: every dream-engine consumer (dream.rs:331/602/824, including SWS replay) reads via get_episode_memory_ids, which dedups on read (sqlite.rs:2044-2045), so no double-weighting of consolidated memories occurs.

### R-40 [tests-docs] ARCHITECTURE.md is two waves stale: 63/66 tool counts, '6-phase' dream engine, source tree missing ingest.rs and vision.rs

`ARCHITECTURE.md:18`

The design doc contradicts the code and itself: line 18 says 'MCP tool surface (66 tools)', lines 93 and 98 say '(63 tools)', lines 230/249/267 say 66 — actual is 67 (TOOL_NAMES, pinned by the dispatch test asserting 67). Lines 16, 92, 213, and 268 describe a '6-phase' dream engine ('3 algorithmic + 3 LLM') — actual is 8 phases since the exo-evolution port (asserted at integration_test.rs:1698 and dispatch.rs:2612). The source-tree diagram (lines 63-98) omits ingest.rs and vision.rs entirely, two full modules shipped in waves 4-5, and places tests/ at repo root instead of crates/cerebro/tests/.

**Failure scenario:** A contributor or agent onboarding from ARCHITECTURE.md plans against a 66-tool, 6-phase system with no ingestion/vision modules — e.g., adds a 'new' image-ingestion module that already exists, or writes a dream-phase assertion expecting 6 phases.

**Verification note:** Every cited discrepancy verifies against the code. ARCHITECTURE.md line 18 says 66 tools, lines 93/98 say 63, lines 230/249/267 say 66 — actual is 67 (TOOL_NAMES has 67 entries; dispatch.rs test tools_list_echoes_id_and_contains_67_tools asserts 67; CLAUDE.md says 67/67). Lines 16/92/213/268 describe a 6-phase dream engine — actual is 8 phases (dream.rs:277 runs [p1,p2,p3,pv,p4,pc,p5,p6]; asserted =8 in integration_test.rs and dispatch.rs:2612, both citing the exo-evolution port). The source-tree diagram omits crates/cerebro/src/ingest.rs and vision.rs (both exist) and shows tests/ at repo root (does not exist; tests are at crates/cerebro/tests/). Not a documented deviation — CLAUDE.md:207 explicitly requires ARCHITECTURE.md to travel with code, and git log confirms it was last touched before the exo-evolution/backport waves.

### R-41 [tests-docs] CLAUDE.md header still says 'Same 63 MCP tools' while its own build table says 67/67; CONTRIBUTING.md says 66

`CLAUDE.md:3`

CLAUDE.md line 3 ('Same 63 MCP tools'), line 17 ('MCP-over-stdio binary (63 tools)'), and line 100 ('Same 63 tools. Same MCP contract.') all predate the audit fix C-RS-012 that ACTIONPLAN-REPORT.md:32 claims reconciled '63 tools' drift — yet the file's own step-9 row (line 131) says '67/67 wired'. CONTRIBUTING.md line 22 warns against 'Changing the 66-tool interface'. Actual count is 67. CLAUDE.md is the auto-loaded agent guide, so this contradiction is served into every session's context.

**Failure scenario:** An agent following CLAUDE.md verifies the ApexOS drop-in by counting tools, gets 67 against the documented 63, and burns a session investigating a phantom discrepancy — or 'fixes' the count in the wrong direction.

**Verification note:** Verified verbatim: CLAUDE.md lines 3, 17, and 100 all say "63 tools" while line 131's step-9 row says "67/67 wired", and CONTRIBUTING.md:22 says "66-tool interface". The true count is 67, enforced by the test tools_list_echoes_id_and_contains_67_tools (dispatch.rs:2098, asserts tools.len() == 67) and stated in main.rs:14. Not a deliberate deviation: CLAUDE.md's own step-8 note says "Tool count corrected: 66 (not 63...)", and git pickaxe shows the "Same 63 MCP tools" header is unchanged since the initial commit (7bd1544). Audit fix C-RS-012 (ACTIONPLAN-REPORT.md:172) only scoped tools.rs/main.rs/the test name/CLAUDE.md's "62/66 wired" — the three header lines were never in its fix list, so they survived both subsequent count bumps (63→66→67). No test guards doc consistency.

### R-42 [tests-docs] README test counts stale: badge and suite table say 238, actual is 250

`README.md:13`

The shields.io badge (line 13, 'tests-238_passing'), the build instruction comment (line 444, '# 238 tests'), and the per-suite table (lines 447-453: cerebro unit 130, integration 64, cerebro-mcp 42, api 2 = 238) all lag one wave behind. cargo test today: 135 + 69 + 44 + 2 = 250 passing. The wave-5 backport (dedup gate, ratchet, stamping, summary listings) added ~12 tests without the README being bumped — violating the repo's own 'docs travel with code' discipline. Everything else on the README front page (67 tools, 8 phases) is current.

**Failure scenario:** A reader cross-checks the badge against a local cargo test run, sees 250 vs 238, and doubts either the repo's rigor or their own build; the badge is the repo's public quality claim.

**Verification note:** Confirmed on every axis. (1) README.md:13 badge reads 'tests-238_passing', line 444 comment reads '# 238 tests', and the suite table (lines 447-453) sums 130+64+42+2 = 238 — exactly as the finding claims. (2) A fresh `cargo test` run passes 135 (cerebro unit) + 69 (integration) + 44 (cerebro-mcp) + 2 (cerebro-api) = 250, matching the finding's per-suite numbers precisely. (3) This is not a documented deviation — the opposite: CLAUDE.md's own wave-5 note (line 163, dated 2026-07-28) states '250 tests green', and git history shows README.md was last touched at b8963cd, before the wave-5 merge commits (#19/#20) that added the ~12 tests, violating the repo's stated 'Docs travel with code' discipline. (4) Nothing prevents the failure scenario: the badge is a static shields.io URL, not generated from CI, so any reader cross-checking a local run sees 250 vs 238.

### R-43 [tests-docs] list_deleted's deliberate full-body (wire_node) exception has zero test coverage

`crates/cerebro-mcp/src/dispatch.rs:575`

CLAUDE.md documents that the four listing tools return wire_summary rows while 'list_deleted keeps wire_node (only pre-restore window)' — the one listing that must keep full bodies so content is inspectable before restore. The summary contract is thoroughly pinned for the four summary tools (listing_tools_return_summaries_not_bodies, dispatch.rs:2297-2412), but no test anywhere exercises list_deleted (grep across all crates finds only the route at dispatch.rs:575, the schema, and the api route — no test). The deliberate exception is exactly the kind of asymmetry a future cleanup sweep flattens.

**Failure scenario:** A follow-up token-efficiency pass converts list_deleted to wire_summary for consistency with its four siblings; no test fails; users can no longer read a soft-deleted memory's full content before deciding whether to restore or purge it.

**Verification note:** Verified: dispatch.rs:575 returns wire_nodes (full bodies) for list_deleted; the summary-contract test (dispatch.rs:2296-2413) pins only the four summary listings; grep across all crates finds zero tests of list_deleted at any layer (dispatch, storage, api). The exception is load-bearing — get_memory refuses soft-deleted memories (pinned at integration_test.rs:912), so list_deleted is the only pre-restore content window — yet converting it to wire_summary would pass all tests. CLAUDE.md documents the behavior but that doesn't guard it in CI. The Python reference even truncates to content[:100] in its list_deleted output, making a "consistency" sweep the plausible regression vector the finding describes.

### R-44 [tests-docs] retention_caps_env_parse mutates process-global env vars in a parallel test binary

`crates/cerebro/src/engines/dream.rs:1874`

The test calls std::env::set_var/remove_var on CEREBRO_RETAIN_* while the cerebro lib test binary runs 135 tests across parallel threads. Today it is safe only by accident: the sole non-test caller of RetentionCaps::from_env() is the dream cycle's pre-phase sweep (dream.rs:241), which no lib unit test currently invokes. Any future lib-crate test that runs a dream cycle (or reads these vars) will race with this test and flake intermittently. A panic between set_var and the trailing remove_var also leaks the values into subsequent tests. env::set_var is unsafe in Rust 2024, so this also blocks a future edition bump.

**Failure scenario:** Someone adds a unit test in the cerebro crate that exercises run_cycle; roughly one CI run in N, it reads CEREBRO_RETAIN_DREAM_REPORTS=0 mid-mutation and sweeps all dream reports, failing with an unreproducible assertion.

**Verification note:** Verified at dream.rs:1874-1893: the test mutates process-global CEREBRO_RETAIN_* env vars with no panic-safe cleanup, in a lib test binary of exactly 135 tests running on the default parallel libtest harness (no .cargo config, no serial_test, harness=false is bench-only). The sole non-test from_env() caller is run_cycle (dream.rs:241), invoked by no lib unit test today — the only test caller is tests/integration_test.rs:1691, a separate process, so the race is latent, exactly as the finding states. Crate is edition 2021 and set_var/remove_var are unsafe in Rust 2024, so the edition-bump blocker is also real. Not documented as a deliberate deviation anywhere (CLAUDE.md, docs/). Latent hazard + edition blocker, no current failure possible: low severity stands.
