# CerebroCortex-RS — backport plan from ApexOS-RS cerebro

> Generated 2026-07-22 by an 8-agent classification sweep over the 45 cerebro-touching
> commits ApexOS-RS made since the 2026-06-11 fork (each commit's cerebro hunks read in
> full; presence grep-verified against this repo's `feat/exo-evolution-frontier` tree).
> Verdict: **27 backports + 2 partials**; 12 commits confirmed already-present via the
> June parity/mirror work; 4 skips (cosmetic).
>
> Severity legend: high = corruption/security/wrong results · medium = degraded behavior · low = polish.
> Difficulty: clean = near-cherry-pick · adapt = re-work against this tree · entangled = needs listed deps.

## Wave 1 — data integrity & panic fixes (do these before pointing -RS at a real DB)

The store/recall paths that corrupt data or crash. All 'clean' — the standalone's files barely diverged here.

### `c653e07` fix ws reconnect, utf-8 streaming, broadcast lagged, fts5 escaping, enum unwrap (waves 3+4)
*high / clean*

cerebro hunks only (the WS-reconnect/utf8-carry/broadcast-Lagged fixes are ui-slint/agentd files, not cerebro/): (1) F020 vector.rs fts5_search — replace naive query.replace('"'," ") with per-token quoted-phrase escaping (split_whitespace, each token wrapped as "tok" with internal quotes doubled), neutralizing FTS5 operators while preserving multi-keyword implicit AND, plus an early return Ok(vec![]) on an empty query (old code sends an empty MATCH string = FTS5 syntax error); (2) F022 — remove the dead dyn_params vec build+drop left in fts5_search; (3) F021 sqlite.rs — emotional_valence enum_to_str(v).unwrap() -> .transpose()? in both insert_memory and update_memory (panic -> Err).

**Evidence:** All absent from standalone: crates/cerebro/src/storage/vector.rs:197 still has `let safe_query = query.replace('"', " ");` and vector.rs:208-212 still carries the dead `dyn_params` build + `drop(dyn_params)`; vector.rs:203 is the only text-MATCH site in the standalone (grep MATCH across storage/ + cortex.rs), so every FTS5/keyword recall goes through the unescaped path — on the FTS5-only (Nano-equivalent) tier an operator-bearing or empty query errors the whole recall. crates/cerebro/src/storage/sqlite.rs:475 and :540 still read `node.emotional_valence.as_ref().map(|v| enum_to_str(v).unwrap())`. Surrounding context in both files is byte-identical to the pre-fix ApexOS tree, so the hunks apply cleanly.

### `1494e0b` cerebro storage: busy_timeout, vec0/FTS5/link integrity, FSRS+bm25 ranking (CB-002/005/013/014/020/022)
*high / clean*

Six storage-integrity fixes in cerebro/src/storage/{sqlite,vector}.rs + 162 lines of integration tests: (CB-002) busy_timeout(5s) + synchronous=NORMAL so the two daemons sharing one WAL DB wait instead of dropping writes on SQLITE_BUSY; (CB-005) DELETE stale memory_vectors rows before INSERT OR REPLACE and inside purge, preventing orphaned vec0 vectors that mis-rank recall after rowid reuse; (CB-013) activation_at_risk uses the canonical FSRS power-law retrievability instead of a divergent exp(-t/S); (CB-014) FTS5 fallback surfaces bm25() mapped through a logistic instead of a flat 0.5 score; (CB-020) FTS triggers guard on deleted_at so soft-deleted rows leave the index and a second 'delete' of an absent row can't corrupt it; (CB-022) purge_memory/purge_all_deleted delete dependent links rows in-transaction so hard delete doesn't fail under foreign_keys=ON.

**Evidence:** All six absent in standalone: crates/cerebro/src/storage/sqlite.rs:422 has only 'PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;' (no busy_timeout/synchronous — repo-wide grep for busy_timeout empty); insert_memory (sqlite.rs:456) has no vec cleanup and grep 'DELETE FROM memory_vectors' / 'DELETE FROM links' hits nothing; purge_memory (sqlite.rs:662) is a bare single DELETE, purge_all_deleted (:669) likewise; activation_at_risk (sqlite.rs:1114) still computes (-days / stability.max(0.001)).exp(); triggers memories_ad/memories_au (sqlite.rs:1767-1777) have no 'WHERE old.deleted_at IS NULL' guards; vector.rs:223-224 still pushes the 0.5_f32 placeholder with no bm25 column. The fix's crate::activation::retrievability path resolves in the standalone too (re-exported at crates/cerebro/src/activation/mod.rs:6).

### `f79d39c` fix(cerebro): vec0 upsert must delete-then-insert, not INSERT OR REPLACE (#141)
*high / clean*

sqlite-vec's vec0 virtual table does not honor INSERT OR REPLACE — it raises 'UNIQUE constraint failed on memory_vectors primary key' on an existing rowid. embed_and_store and store_raw_embedding both used INSERT OR REPLACE, so any re-embed of an existing memory (update_memory with a content change) hard-failed. Fix routes both write paths through a shared upsert_memory_vector helper (DELETE stale rowid, then INSERT, under the shared connection lock) plus regression test reembedding_an_existing_memory_succeeds (uses store_raw_embedding, no ONNX model needed).

**Evidence:** Bug CONFIRMED live in standalone: crates/cerebro/src/storage/vector.rs:84 and :103 both contain the exact buggy 'INSERT OR REPLACE INTO memory_vectors(rowid, embedding) SELECT rowid, ?1 FROM memories WHERE id = ?2'; no upsert_memory_vector, and NO 'DELETE FROM memory_vectors' anywhere in crates/cerebro/src (grep exit 1) — the standalone also lacks ApexOS's CB-005 pre-delete on insert/purge (present in ApexOS sqlite.rs:490/:793/:824), so it is MORE exposed: stale vec rows survive memory deletion and rowid reuse can silently attach an old vector to a new memory. Test-support symbols all present: VectorStore::new(&sqlite, model) vector.rs:25, SqliteStore::open sqlite.rs:415, MemoryNode::new models/memory.rs:29, tempfile dep crates/cerebro/Cargo.toml:28, vec_available field vector.rs:21. Pre-image around both write paths is nearly byte-identical to ApexOS's (same comment + SQL, verified vector.rs:70-110).

### `8053b3d` cerebro: fix dream_run UTF-8 panic, constant-time token compare, priority casing drift **[PARTIAL]**
*medium / clean*

Three independent fixes in the cerebro hunks: (1) char-boundary-safe truncate_chars in engines/dream.rs replacing byte-slicing that panicked mid-emoji/CJK during consolidation, with a unit test; (2) cerebro-api bearer-token comparison switched to subtle::ConstantTimeEq (ct_eq helper) in the AGENTD_TOKEN middleware; (3) cerebro-mcp normalize_priority: session_save canonicalizes the priority tag to uppercase and session_recall canonicalizes the filter, so 'medium'/'MEDIUM'/'Medium' all match, plus a case-insensitivity note in the tool schema and two tests (normalize_priority_is_case_insensitive, session_save_recall_priority_casing_matches).

Parts:
- BACKPORT: normalize_priority helper + its application in session_save (canonicalize before writing the priority: tag) and session_recall (canonicalize the filter), the tools.rs schema description note, and the two tests — the standalone has the exact bug (schema-uppercase enum vs lowercase default/raw compare → silent empty recall results)
- PRESENT: dream.rs truncate_chars + test (already upstream, more widely used there)
- APEXOS-ONLY: cerebro-api ct_eq/subtle constant-time compare — attaches to the AGENTD_TOKEN middleware the standalone does not have; only relevant upstream if/when cerebro-api grows an auth layer (bring ct_eq with it then)

**Evidence:** truncate_chars PRESENT upstream: CerebroCortex-RS crates/cerebro/src/engines/dream.rs:1300 (fn truncate_chars) with the identical mid-emoji test at dream.rs:1754-1759, used at 363/387/481/etc. normalize_priority ABSENT: crates/cerebro-mcp/src/dispatch.rs:306 still `unwrap_or("medium")` writing `priority:{priority}` raw (:311), recall filter compares the raw caller string at :338-339, no `normalize_priority`/`to_uppercase` hit anywhere in crates/cerebro-mcp/src/, while crates/cerebro-mcp/src/tools.rs:205 advertises the UPPERCASE enum ["LOW","MEDIUM","HIGH","CRITICAL"] with 'default: medium' — the save/recall case-mismatch bug is live upstream. ct_eq N/A: the standalone cerebro-api (crates/cerebro-api/src/main.rs, 917 lines) has ZERO grep hits for token/TOKEN/auth/Bearer — no auth middleware exists to harden; the ApexOS gate reads AGENTD_TOKEN (ApexOS-RS cerebro/crates/cerebro-api/src/main.rs:968-991), an agentd integration.

### `39a5fb1` cerebro: dream error accounting + enforce timestamp cap (CB-024/030) **[PARTIAL]**
*low / clean*

Two fixes: (CB-024) dream phases log+skip-count on write failure instead of swallowing errors while counting success — four sites: the dream-report persist, Phase 3 schema persist, Phase 4 emotional persist, Phase 5 prune count; (CB-030) MemoryNode::record_access() enforces the previously-dead MAX_STORED_TIMESTAMPS cap on access_times (with 2 unit tests). Most of this already lives in the standalone via its parallel exo-evolution commits; only two CB-024 sites remain unported.

Parts:
- save_dream_report failure warn (dream.rs report persist: replace 'let _ =' with if-let-Err + tracing::warn)
- Phase 4 emotional-reprocess persist accounting (match on update_memory result, count only Ok)
- optional: the two CB-030 record_access unit tests (logic already present, tests never flowed)

**Evidence:** PRESENT: record_access with the CB-030 cap is byte-identical at crates/cerebro/src/models/memory.rs:52-67 (called from cortex.rs:184); CB-024 Phase-3 accounting at crates/cerebro/src/engines/dream.rs:528-539 and Phase-5 prune accounting at dream.rs:1069-1081 — both carrying the literal 'CB-024' comments, landed via standalone commit 875aec1 (git log -S "CB-024" hits it). ABSENT: the save_dream_report persist is still 'let _ =' at dream.rs:246-252 (failure silently dropped), and the Phase-4 emotional persist is still 'let _ = ...update_memory' + unconditional 'memories_processed += 1' at dream.rs:704-706; the two CB-030 unit tests (record_access_caps_timestamps_and_keeps_most_recent, record_access_below_cap_appends) are also absent from the standalone's memory.rs.

## Wave 2 — security & scope enforcement

The API is currently LAN-exposed unauthenticated by default; scope enforcement lives only in the dispatch layer. Order matters: a4a2c51 before 3b2a1cd; 74e0a57 before 56b6a05; 0adce38+56b6a05 before 8ade4c6.

### `a4a2c51` add bearer token auth and default localhost bind (wave 1)
*high / adapt*

cerebro hunk (cerebro-api/src/main.rs only): adds an axum middleware gating every route on a shared secret read from AGENTD_TOKEN (accepts 'Authorization: Bearer <tok>' or '?token=' query param; auth disabled with a log note when the var is unset), and flips the default CEREBRO_API_ADDR bind from 0.0.0.0:8765 to 127.0.0.1:8765. The agentd/install.sh parts of the commit are outside cerebro/.

**Evidence:** Absent from standalone: /home/andre/Projects/CerebroCortex-RS/crates/cerebro-api/src/main.rs:911-912 still binds '0.0.0.0:8765' by default; grep for AGENTD_TOKEN|Bearer|middleware|is_loopback in that file yields zero hits — no auth layer of any kind. The full memory API (remember/recall/update/delete/trash/dream routes, main.rs route table ~line 880-905) is LAN-exposed unauthenticated by default.

### `3b2a1cd` fix ui bearer token, workspace traversal, non-loopback tokenless bind (wave 7)
*medium / entangled — depends on a4a2c51*

cerebro hunk (cerebro-api/src/main.rs only): F036 — bail! at startup if the token is empty AND the parsed bind address is non-loopback (prevents an env typo from silently re-opening the unauthenticated-LAN hole); F037 — log the '?token=' dashboard URL hint at startup when a token is set. The F034 (ui-slint bearer header) and F035 (workspace_decision traversal guard) parts are ui-slint/agentd files, not cerebro/.

**Evidence:** Absent from standalone: grep for is_loopback|bail|127.0.0.1 in crates/cerebro-api/src/main.rs yields zero hits (only main.rs:912 '0.0.0.0:8765'); the tail of main() (main.rs:934-940) binds and serves with no guard and no hint. Both hunks reference the `api_token` variable introduced by a4a2c51's middleware, so this only makes sense stacked on that backport.

### `74e0a57` fix(cerebro): CB-007/009/019 — lock-free embedding + graph-before-vector on the store/recall hot path (#250)
*medium / clean*

Three fixes, one seam. CB-007: remember() no longer holds the storage write guard across fastembed inference — new CerebroCortex::embed_lockfree clones the embedder Arc under a brief read guard, runs spawn_blocking with no lock. CB-009: graph add_node now runs BEFORE the vector persist and embed failure is non-fatal (warn + vector-less store; FTS5 still finds it) — previously an embed error ?-returned before add_node, orphaning the memory out of spreading activation until restart. CB-019: recall() pre-embeds the query lock-free and passes it to new VectorStore::search_seeded (never embeds; None → FTS5); vec_search_blob shared KNN body; embedder_handle() accessor. search()/embed_and_store() signatures unchanged. 2 integration tests.

**Evidence:** Standalone has all three bugs verbatim: remember() takes the write guard then `storage.vector.embed_and_store(&node.id, &node.content).await?` before add_node (crates/cerebro/src/cortex.rs:89-92 — lock across inference + orphan-on-error + vector-before-graph), and recall() takes the read guard then storage.vector.search (cortex.rs:108-113). search_seeded/embed_lockfree/embedder_handle/vec_search_blob = zero grep hits. Prereqs present: store_raw_embedding (crates/cerebro/src/storage/vector.rs:94), embedder: Option<Arc<TextEmbedding>> (vector.rs:20), search+is_vec_available (vector.rs:116,135). Only friction: the cortex.rs hunk context includes 718c295's thalamus error-message line (trivial).

### `0adce38` fix(cerebro): CB-018 — destructive ops are scope-enforced at the store layer (#252)
*high / entangled*

delete_memory/purge_memory/restore_memory/bulk_delete/prune_thread/purge_all_deleted and the tag rewrites (delete_tag_everywhere/rename_tag_everywhere) all gain a VisibilityScope param with scope_sql IN the WHERE (atomic under the connection lock, no check-then-act; purge gates a visibility SELECT under the same lock). share_memory gains a true ownership rule: owner or admin/global only; shared/ownerless commons memory refused with a loud Err. bulk_delete returns its RETURNING id set and the coordinator evicts graph nodes ONLY for rows actually deleted (denied delete no longer hides a live memory from spreading). Call sites: MCP dispatch passes agent_scope(args); cerebro-api + CLI pass explicit global() (admin unchanged); dream prune passes cycle scope. 2 integration tests.

Parts:
- Scope-in-WHERE on the nine sqlite bodies + share_memory ownership gate + dispatch/api/cli call-site plumbing — adaptable directly against the standalone's simpler bodies
- Coordinator graph-eviction gating (bulk_delete RETURNING set, gated remove_node) — requires the earlier ApexOS coordinator-wrapper/purge-cleanup backports (CB-005/CB-022/CB-024 class, outside this assigned set); without them the standalone has no coordinator layer to gate

**Evidence:** Standalone is fully unscoped: delete_memory sqlite.rs:512, purge_memory:662 (naive single-statement DELETE — no CB-005/022 dependent-row cleanup), purge_all_deleted:669, restore_memory:676, bulk_delete:828 (returns usize), share_memory:893 (sets visibility+agent_id on ANY id), prune_thread:955, delete_tag_everywhere:1021, rename_tag_everywhere:1033 — none take a scope. Dispatch calls .sqlite directly (crates/cerebro-mcp/src/dispatch.rs:195,372,389,433,497). VisibilityScope::sql_filter/for_agent/global present (crates/cerebro/src/types.rs:132-144). Entanglement: the diff's storage/mod.rs half modifies coordinator wrappers that don't exist upstream — standalone StorageCoordinator is 23 lines with only new() (no delete/purge/bulk/restore wrappers, no graph eviction on delete at all), and the purge bodies differ (no vector/links cleanup transaction).

### `56b6a05` fix(cerebro): CB-008 — recall visibility bounded to the spread frontier + chunked meta fetch (#253)
*medium / adapt — depends on 74e0a57*

recall() computed a store-wide visibility map (ALL graph ids, one placeholder per id) on every scoped recall — O(live-store) on the hottest path, hard-failing past SQLite's ~32k bind-parameter limit as the store grows. New activation::reachable_frontier (undirected, SPREADING_MAX_HOPS-bounded, strict superset of what spread visits, unit-tested) bounds the visibility fetch to the seeds' reachable neighbourhood; global scope builds its all-true map over the frontier too. Fail-closed by construction (spread's unwrap_or(false) means under-collection weakens, never leaks). get_visibility_meta chunks its IN-clause at 500 ids as belt-and-braces for every caller. Tests: frontier units, cross-agent denial preserved end-to-end, 1200-id chunk test.

**Evidence:** Standalone has the exact bomb: recall builds `all_ids: Vec<MemoryId> = storage.graph.index.keys().cloned().collect()` and calls un-chunked get_visibility_meta (crates/cerebro/src/cortex.rs:130-135; get_visibility_meta with one placeholder per id at crates/cerebro/src/storage/sqlite.rs:754-763). reachable_frontier/CHUNK = zero grep hits. Safety prereq holds upstream: spread is already fail-closed via unwrap_or(false) (crates/cerebro/src/activation/spreading.rs:104); SPREADING_MAX_HOPS at config.rs:54. Adaptation: `scope.shared_only` (the ApexOS federation scope) doesn't exist upstream — drop that condition from the branch; the cortex.rs hunk also stacks textually on 74e0a57's query_vec lines in the same function.

### `8ade4c6` fix(cerebro): CB-003 — cross-process graph freshness via PRAGMA data_version (#254)
*medium / adapt — depends on 0adce38, 56b6a05*

cerebro-mcp and cerebro-api each hold their own in-memory petgraph over one SQLite file, rebuilt once at startup — a memory/link committed by one process never appeared in the other's graph until restart (silently missing association hits in recall; associate falsely bailing 'memory does not exist'). Fix uses SQLite's own signal: PRAGMA data_version changes only on ANOTHER connection's commit. SqliteStore::data_version(); StorageCoordinator records graph_data_version at each (re)build, graph_is_stale() = one pragma row, refresh_graph() re-checks under the write lock. Wired at every graph consumer: recall (two-phase cheap-check-then-upgrade), associate (under its existing write guard), and the four graph-analytics dispatch arms. Tests: two cortexes over one db file, both directions, plus own-writes-never-flag-stale.

**Evidence:** data_version/graph_is_stale/refresh_graph = zero grep hits in standalone. The standalone has the identical dual-process shape and the bug: GraphStore::rebuild_from_db called exactly once, in StorageCoordinator::new (crates/cerebro/src/storage/mod.rs:19; graph.rs:28) — the commit message's 'deliberate divergence from the Python standalone' refers to the Python original, not CerebroCortex-RS, which ships both front-ends over one file. All wiring anchor points exist upstream: get_associations/find_path/get_common_neighbors/memory_graph_stats arms (crates/cerebro-mcp/src/dispatch.rs:250-291), associate (cortex.rs:201). Adaptation: the mod.rs hunks contextually assume the CB-018-signature coordinator wrappers (esp. the restore_memory re-baseline) which don't exist in the 23-line upstream coordinator — the mechanism itself (field + three methods + call sites) ports directly.

## Wave 3 — behavior fixes & lifecycle hygiene

Wrong-result and slow-rot classes: recall filters, FSRS decay, graph pruned on delete, write-dead audit log (52ed3dd before 85912af), dream rediscovery dedup, the memory_store alias arg-drop.

### `f50ba92` cerebro-mcp: per-frame parse isolation + string-or-array coercion + derived_from (CB-010/011/025)
*medium / clean*

Three cerebro-mcp fixes: (CB-010) transport.rs gains a Frame enum (Value/Eof/ParseError) so a malformed JSON-RPC line answers -32700 and the daemon keeps serving instead of the process exiting (main.rs loop rewritten around it, dispatch.rs adds parse_error()); (CB-011) coerce_str_list() honors the advertised anyOf[array,string] schema for tags/concepts/source_ids across remember, update_memory, store_intention, store_procedure, find_relevant_procedures, create_schema, find_matching_schemas — a bare string was previously silently dropped; (CB-025) store_procedure reads derived_from (and sibling source_ids) into node.metadata mirroring create_schema instead of discarding provenance. Plus 4 tests covering all three.

**Evidence:** All absent in standalone: crates/cerebro-mcp/src/transport.rs:23 still 'anyhow::bail!("EOF on stdin")' and read() propagates serde parse errors as Err (fatal in main.rs loop); repo grep for coerce_str_list/parse_error/Frame:: is empty; every list arg still parses via .as_array() only (dispatch.rs:143,211,715,766,793,890,896,927); store_procedure (dispatch.rs:759-772) never touches derived_from — the only derived_from writes (:904-906) are create_schema's. Hunk pre-images match the standalone's current handler bodies verbatim (store_procedure, create_schema, find_relevant_procedures — the latter's exo champion-ranking sits below the two parse lines being replaced), so the patch applies with minimal fuzz despite the file's exo-frontier additions.

### `700c739` cerebro-api: re-embed on PUT, priority casing, panic layer, recall filters (CB-006/012/023/026)
*medium / clean*

Four cerebro-api fixes in src/main.rs (+ tower-http dep): (CB-006) PUT /memory re-runs vector.embed_and_store when content changed, so the vec0 index no longer points at pre-edit text; (CB-012) session_save normalizes priority to uppercase via normalize_priority(), matching the MCP canonical so priority:<P> tags written over HTTP are findable by MCP session_recall filters; (CB-023) tower_http CatchPanicLayer::custom(panic_response) as the outermost layer turns a handler panic into a 500 JSON body instead of a dropped connection; (CB-026) HTTP session_recall gains priority/session_type filters on RecallQuery, matching the MCP twin's result set. Plus 2 unit tests for the casing.

**Evidence:** All absent in standalone crates/cerebro-api/src/main.rs: update_memory calls only sqlite.update_memory with no embed_and_store/content_changed (fn body verified, matches ApexOS pre-image byte-for-byte); session_save still 'req.priority.as_deref().unwrap_or("medium")' (:424) with no normalize_priority anywhere; RecallQuery (:199-204) has only query/top_k/agent_id; session_recall filters only on the session_note tag; grep CatchPanicLayer/catch_panic empty and the workspace Cargo.toml has tower 0.4 (:39) but no tower-http. Backport must also add tower-http (catch-panic feature) to the standalone's workspace Cargo.toml, mirroring what this commit did to ApexOS's root Cargo.toml.

### `a685dab` fix(cerebro): graceful embed-model fallback + bge-small on pro tier (#35)
*medium / clean*

Cerebro hunk (the only upstream-relevant part): init_fastembed (storage/vector.rs) no longer anyhow::bail!s on an unrecognized CEREBRO_EMBED_MODEL — it falls back to BGESmallENV15 with a loud tracing::warn, so embeddings stay enabled instead of the bail propagating into VectorStore::new's catch-arm which silently degrades cerebro to FTS5-only. The install.sh tier-table half of the commit is ApexOS-side and irrelevant upstream.

**Evidence:** ABSENT upstream: CerebroCortex-RS crates/cerebro/src/storage/vector.rs:249-250 still reads `"BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15, other => anyhow::bail!("unsupported embed model: {other}")`; VectorStore::new at vector.rs:41-43 catches init errors with `tracing::warn!("fastembed init failed … — embedding disabled, FTS5 only"); None` — i.e. the standalone has the exact silent-degradation failure mode this commit fixes. init_fastembed is byte-identical to the pre-fix ApexOS version, so the patch applies as-is.

### `815d380` feat(cerebro): run the FSRS half of recall-reinforcement (decay + last_review) (#94)
*medium / clean*

Adds MemoryNode::record_recall_review(now) (rating-free successful FSRS review: recompute stability via update_stability_on_recall, difficulty via update_difficulty_on_recall from current retrievability, stamp last_review=now), calls it in cortex::recall beside record_access, and widens SqliteStore::record_accesses from a 3-tuple to a 6-tuple so the batched recall UPDATE also persists fsrs_stability/fsrs_difficulty/fsrs_last_review (RFC3339, NULL when unset). Fixes: fsrs_last_review was never written, so activation_at_risk (WHERE fsrs_last_review IS NOT NULL) was always empty and the FSRS forgetting curve never advanced. Includes unit test record_recall_review_sets_last_review_and_grows_stability.

**Evidence:** Cluster question answered: the standalone's 9fe92cd recall reinforcement is the ACT-R half ONLY — its FSRS decay half does NOT match 815d380. CerebroCortex-RS crates/cerebro/src/cortex.rs:181-193 calls only node.record_access(now) and builds Vec<(MemoryId,u32,String)>; sqlite.rs:561-574 record_accesses still takes &[(MemoryId,u32,String)] and UPDATEs only access_count/access_times; models/memory.rs has record_access at :58 but NO record_recall_review; activation_at_risk at sqlite.rs:1093-1095 filters `fsrs_last_review IS NOT NULL` → provably always empty. All prerequisites are present upstream: fsrs.rs:28 update_stability_on_recall / :59 update_difficulty_on_recall (+ retrievability), StrengthState{stability,difficulty,last_review} memory.rs:71-75, fsrs_* columns in schema sqlite.rs:1656 and full read/write paths (:463,:528). The three touched regions match the pre-commit ApexOS code near-verbatim → cherry-pickable.

### `bc226ee` fix(cerebro): prune the in-memory graph on delete/purge/bulk_delete (#95)
*medium / adapt*

delete_memory was a SQLite soft-delete that never touched the in-memory petgraph (GraphStore had no remove_node; the graph only shrank on restart rebuild), so a deleted/purged node kept participating in spreading-activation until restart — the recall-final get_memories_by_ids scope/deleted filter was the only safety net (content never leaked, but a dead node still shaped the spread and boosted its neighbors). Adds GraphStore::remove_node (idempotent, repairs the id→NodeIndex map for the node petgraph swap-moves into the freed slot — plain Graph renumbers on removal) and coordinator-level delete/purge/bulk_delete/restore that prune the graph beside the SQLite write (restore rebuilds the graph to recover links); all callers moved to write-lock coordinator calls.

Parts:
- GraphStore::remove_node with swap-remove index-map repair + test remove_node_repairs_swapped_index_and_keeps_edges (graph.rs) — drops in verbatim, standalone GraphStore struct is identical (plain Graph + index HashMap, graph.rs:9-12)
- StorageCoordinator::{delete_memory, purge_memory, bulk_delete, restore_memory} wrappers (storage/mod.rs) — drop in verbatim
- Rewire cerebro-mcp dispatch delete/restore/purge/bulk_delete from read().sqlite direct to write().coordinator (dispatch.rs:191-195, 360-364, 368-372, 382-389)
- Rewire dream prune loop through the coordinator write lock (standalone dream.rs:1069-1071 — same code shape, different file offset in the exo-frontier version)
- STANDALONE-EXTRA (not in the ApexOS diff): cerebro-api delete/restore/purge/bulk_delete routes (main.rs:303, 749, 758, 774) and cerebro-cli purge/delete (main.rs:391, 394) also call sqlite direct and need the same coordinator routing for the fix to be complete upstream

**Evidence:** ABSENT upstream: grep for remove_node across CerebroCortex-RS/crates hits only rebuild_from_db call-sites (no definition); graph.rs ends at neighbors() (~:80) with no remove_node; storage/mod.rs:16-23 StorageCoordinator has only new(); dispatch.rs:191-389 and dream.rs:1069-1071 (`match cortex.storage.read().await.sqlite.delete_memory(&node.id).await` — same pre-fix shape incl. the CB-024 comment) still bypass the graph. Rated adapt (not clean) because the standalone has MORE delete call sites than the ApexOS diff covers: cerebro-api/main.rs:303/749/758/774 + cerebro-cli/main.rs:391/394 — a straight cherry-pick would leave those paths still graph-stale.

### `583ccb3` fix(cerebro): exclude soul.md undo-snapshots from procedure recall (#166)
*medium / clean*

cerebro hunks: two tag-filter insertions in cerebro-mcp dispatch.rs — list_procedures and find_relevant_procedures now skip memories tagged 'undo_snapshot'. Rationale: evolution rollback snapshots embed the full soul text (packed with 'how to'/'workflow'/'step'), get mis-typed Procedural by classify_type, and dominated skill recall by access count (APEX's top-3 'procedures' were 3 soul snapshots). Non-destructive — no DB mutation, heals existing polluted stores at read time. The commit's second hunk (agentd main.rs typing future snapshots as episodic at store time) is agentd-side, outside cerebro/, and inherently ApexOS-only.

**Evidence:** ABSENT from standalone: grep for 'undo_snapshot' across CerebroCortex-RS/crates returns nothing (exit 1). The target routes exist with closely matching shape but no filter: list_procedures at crates/cerebro-mcp/src/dispatch.rs:775-790 (filters only on salience), find_relevant_procedures at :792-831 (tag/concept gate + champion-aware retrieval_rank ordering, no snapshot exclusion). The tag's producer is ApexOS's evolution applier, but CerebroCortex-RS is the drop-in candidate on the same ApexOS DBs (its stated deployment path), where unfiltered snapshot pollution directly degrades champion retrieval — the exo-evolution frontier feature this branch is named for. Two defensive filter lines, generic to any host storing rollback artifacts.

### `a307e01` fix(cerebro): C6 — find_relevant_procedures widened + honest empty result (#242)
*medium / adapt*

Widens find_relevant_procedures: norm_tag() case/separator-insensitive tag matching (lookup-side only), concepts scan content as well as metadata (case-insensitive), a stage-2 semantic widening through brain.recall (new `query` arg, procedural-filtered, fires only when exact matching leaves room), champion retrieval_rank ordering preserved across both stages, and an honest structured response ({procedures, matched:{exact,semantic}, procedures_in_scope, note}) so an empty result over a non-empty store says 'matcher may have missed', not 'nothing exists'. Tool schema updated; 4 new tests.

**Evidence:** Standalone has the OLD matcher: exact tag equality `nt == t` and metadata-only substring concept scan at crates/cerebro-mcp/src/dispatch.rs:812-817, returns bare `json!(filtered)` array (dispatch.rs:829). grep for norm_tag/procedures_in_scope across crates/ = zero hits. retrieval_rank champion ordering IS already present (dispatch.rs:826-827, from ab94573) so only the widening + honesty layer is missing. brain.recall + MemoryType available upstream.

### `52ed3dd` fix(cerebro): C3 — the audit log was write-dead; wire self-history writes at the dispatch chokepoint (#243)
*medium / clean*

The audit table + both read tools shipped but log_audit_event had ZERO call sites — query_audit was empty forever. Fix: dispatch_tool wraps route() and writes one attributed audit row per successful mutating call via a pure audit_action whitelist (stores/updates/deletes/episodes/procedures/tags/dream_run; reads excluded; describe_image only when remember:true), audit_memory_id (args-then-result), audit_details (120-char capped preview). Best-effort — audit failure never fails the recorded call. query_audit gains action + since filters (dynamic WHERE, RFC3339 string compare) and an honest description.

**Evidence:** Standalone is write-dead identically: `log_audit_event` defined at crates/cerebro/src/storage/sqlite.rs:1389 with no call site anywhere in crates/ (grep: only the definition matches). query_audit at sqlite.rs:1405 is the old 2-arg version, byte-identical to the ApexOS pre-change body. The dispatch_tool spawn wrapper (dispatch.rs:74-78) matches the ApexOS pre-change shape exactly, so the chokepoint hunk drops in. All whitelist tools exist upstream except describe_image (that arm is dead-but-harmless, or drop it).

### `85912af` fix(cerebro): CB-021 — retention sweep bounds the three forever-growing lifecycle tables (#249)
*medium / clean — depends on 52ed3dd*

Adds SqliteStore::retention_sweep (keep newest N memory_versions PER memory via ROW_NUMBER window, newest N dream_reports, newest N audit_log rows; cap 0 = keep forever) run as a dream pre-phase beside close_stale_episodes, fail-soft. RetentionCaps::from_env (CEREBRO_RETAIN_VERSIONS/_DREAM_REPORTS/_AUDIT_ROWS, defaults 10/90/20000, unit-tested). The sweep writes one self-audit 'retention_sweep' row naming counts+caps when it prunes (honesty rule). Integration test: newest-N survival per table, self-audit marker, idempotence.

**Evidence:** retention_sweep/RetentionCaps/CEREBRO_RETAIN absent from standalone (grep = zero hits). All prerequisites present: memory_versions + dream_reports tables (crates/cerebro/src/storage/sqlite.rs:1729-1751), log_memory_version:1461 / get_memory_versions_raw:1486 / save_dream_report:1578 / get_last_dream_report:1601 / audit_log + log_audit_event:1389, and the dream pre-phase hook (close_stale_episodes at crates/cerebro/src/engines/dream.rs:192-196) where the sweep slots in.

### `718c295` fix(cerebro): CB-029 — bounded input on the store path and the MCP transport (#251)
*medium / entangled*

Two independent bounds. (1) Thalamus: MAX_CONTENT_LENGTH = 64 KiB upper gate in evaluate_input, reject-not-truncate, with an honest cortex rejection message naming the cap; boundary test. (2) Transport: read_frame() extracted from StdioTransport, caps a newline-delimited frame at 32 MiB via take(), drains the oversized tail buffer-sized to the next newline so the following frame parses; Frame::Oversized answered as a JSON-RPC parse error in main.rs (init + loop), daemon stays alive; 4 unit tests.

Parts:
- Thalamus 64 KiB MAX_CONTENT_LENGTH + honest cortex remember() rejection message — clean, standalone-independent
- Transport read_frame cap+drain + Frame::Oversized handling in main.rs — requires the CB-010 Frame-enum backport first (standalone still bails the whole read on a malformed/EOF frame; not in this assigned set), else rewrite read() from scratch with the cap

**Evidence:** Part 1 ports clean: standalone thalamus has only MIN_CONTENT_LENGTH (crates/cerebro/src/engines/thalamus.rs:6, gate at :42), no MAX; evaluate_input shape identical. Part 2 is blocked on missing groundwork: standalone transport.rs has NO Frame enum at all — read() returns Result<Value> and bails on EOF/parse error (crates/cerebro-mcp/src/transport.rs:19-26), i.e. CB-010's per-frame isolation never flowed upstream either. MAX_FRAME_BYTES/Oversized/read_frame = zero grep hits.

### `054dc86` feat(dream): C2 — semantic rediscovery reinforcement + novel/rediscovery diff in report + journal (#237)
*medium / clean*

Cerebro hunk (dream.rs only): dream pattern-extraction's dedup was a 40-char prefix match, so the LLM re-minting the same lesson in new words each night sailed past it (apex2: five near-identical procedures over five nights, never merged). Adds reinforce_if_rediscovery — a semantic gate against the WHOLE store: a candidate ≥0.86 cosine (REDISCOVERY_SIMILARITY) to an existing PROCEDURAL memory REINFORCES it (capped salience bump `(s+0.05).min(0.95)` + rediscovered_count/last_rediscovered metadata ledger) instead of storing a fragment; embeddings-only (FTS5/Nano keeps prefix dedup — BM25 isn't a similarity), fail-open on any probe error. PhaseResult gains serde-default procedures_rediscovered + 'N novel / K re-discoveries' notes + back-compat deserialization test. The compose_dream_journal fixes in this commit are agentd-side, not cerebro/.

**Evidence:** Absent from standalone: grep 'REDISCOVERY|rediscover|reinforce_if_rediscovery' over crates/cerebro/src/engines/dream.rs = zero; the 40-char prefix dedup is live at dream.rs:386-387. Port surface matches almost exactly: pattern_extraction has the identical signature (&self, scope, cortex, calls_used, budget, overall_budget) at dream.rs:320-327; vector.search(query,k,scope_sql,scope_params) + is_vec_available/is_embedder_loaded at crates/cerebro/src/storage/vector.rs:116-136; get_memories_by_ids at crates/cerebro/src/storage/sqlite.rs:715; PhaseResult uses the same serde-default pattern at dream.rs:1635-1660 (procedures_mutated/merged already there from ab94573); MemoryNode.metadata is serde_json::Value (models/memory.rs:25).

### `f24e447` fix(evolution): C1 residue — private/attributed/low-salience undo snapshots + H4 rewrite gate (#235)
*medium / adapt — depends on f50ba92*

Cerebro hunks fix the C1 root cause and add the healing knobs: (1) memory_store becomes a TRUE alias of remember — it previously dropped its documented args (remember(content, None, None, None, scope)), so callers' memory_type/tags/salience silently never landed; (2) update_memory gains visibility + set_agent_id params with the orphan guard (refuses to privatize an owner-less memory — private + no owner = visible to no one — demanding set_agent_id first) and unknown-visibility bail; (3) matching schema additions. The applier attribution, fossil_heal_args migration, and H4 snapshot gate parts of this commit are agentd-side, not in the cerebro hunks.

**Evidence:** Both fixes absent from standalone, and the alias bug is LIVE there in identical form: crates/cerebro-mcp/src/dispatch.rs:224-229 memory_store arm calls `brain.remember(content, None, None, None, scope)` while its own schema documents tags (crates/cerebro-mcp/src/tools.rs:117-128, tags at line 125) — documented args silently discarded; grep 'set_agent_id|refusing to privatize' over cerebro-mcp = zero hits; update_memory arm at dispatch.rs:198-220 handles only content/salience/tags. Adapt notes: alias fix uses coerce_str_list (absent upstream — f50ba92, or inline); standalone memory_store schema lacks memory_type/salience properties so the schema hunk needs a small merge; add Visibility to the dispatch.rs:8 import (otherwise imports match — no VisionQuery upstream).

## Wave 4 — features & generic additions

Vision (describe_image → search_vision, vision.rs is new file), VisibilityScope::shared_only, find_by_tags + procedural-import semantics. 03bc1a1 before 1b01533.

### `03bc1a1` feat(cerebro): implement describe_image with a tiered VLM backend (#138)
*medium / adapt*

Wires the advertised-but-stubbed describe_image tool to a real caption implementation: new cerebro::vision module (392 lines — VisionBackend auto|ollama|anthropic|off parsed from CEREBRO_VISION_BACKEND, Ollama transport at CEREBRO_VISION_URL/_MODEL default localhost:11434/moondream, Anthropic claude-haiku-4-5 fallback, magic-byte media-type sniffing, prepare_from_path/prepare_from_b64), a dispatch arm (path|b64 + prompt + remember:true folds the caption into memory tagged 'vision' via brain.remember), a real tool schema in tools.rs, base64 workspace dep, and a dispatch test (missing image → -32602). Entirely generic cerebro capability — no agentd coupling.

**Evidence:** ABSENT from standalone: describe_image is still a deferred stub — CerebroCortex-RS/crates/cerebro-mcp/src/tools.rs:846 ('Deferred Tier-7 tools (ingest_file, describe_image, search_vision)') + tools.rs:916 (TOOL_NAMES entry), dispatch.rs:1034 (deferred not-implemented arm); `find crates -name 'vision*'` and grep for VisionBackend/CEREBRO_VISION return nothing in crates/cerebro/src. Port prerequisites verified present: agent_scope (dispatch.rs:1207), remember(content, Option<MemoryType>, Option<tags>, Option<salience>, scope) (cortex.rs:64), reqwest dep (crates/cerebro/Cargo.toml:16). Missing: base64 in the standalone workspace Cargo.toml (grep exit 1) and the coerce_str_list helper (grep exit 1).

### `1b01533` feat(cerebro): implement search_vision — CLIP visual recall (#186)
*medium / entangled — depends on 03bc1a1*

The read half of the vision loop: cerebro::vision gains lazy-loaded CLIP image+text towers (fastembed ClipVitB32, shared 512-dim space: clip_embed_image/clip_embed_text, PreparedImage::decoded); new plain vision_embeddings table (memory_id PK → 512-dim blob + image_path, schema in sqlite.rs); VectorStore::store_vision_embedding + vision_search (brute-force cosine, no vec0); Cortex::index_image + Cortex::search_vision (VisionQuery::Text|Image, scope-filtered via get_memories_by_ids, caption/FTS fallback over vision-tagged memories when CLIP off or no images indexed); tier gate vision_embed_enabled (follows CEREBRO_EMBED_MODEL, CEREBRO_VISION_EMBED override); describe_image{remember} now also indexes the image; real search_vision tool spec + dispatch arm. Unit tests with fake vectors.

**Evidence:** ABSENT from standalone: search_vision is still a deferred stub (crates/cerebro-mcp/src/tools.rs:846/:917, dispatch.rs:1034); zero grep hits for vision_embeddings, ClipVitB32, clip_embed, index_image, vision_search, or CEREBRO_VISION_EMBED across crates/. Port prerequisites verified present: get_memories_by_ids (dispatch.rs:252/:282/:684), fastembed 4 (workspace Cargo.toml:34 — ClipVitB32 available), Config.embed_model + CEREBRO_EMBED_MODEL (config.rs:83/:91), CerebroCortex::recall (cortex.rs:103), matching CerebroCortex struct shape (cortex.rs:24-27). Hunks are additive (new table, new methods, new arm) but extend the vision.rs module and describe_image arm that 03bc1a1 creates.

### `4b08312` feat(mesh): federated recall — mesh_recall over shared-only scope (slice 2) (#217)
*low / clean*

Cerebro hunks add the federation scope primitive: VisibilityScope gains a shared_only flag + shared_only() constructor, enforced at all three recall touch points — sql_filter() returns "visibility='shared'", can_access() matches only Visibility::Shared, and cortex.rs's spreading-activation all-visible short-circuit is bypassed (`scope.agent_id.is_none() && !scope.shared_only`) so private nodes don't even influence the spread. The MCP recall tool gains a visibility:"shared" param (only narrows). Two tests (scope semantics + end-to-end private-hidden recall). The mesh_recall wiring itself is agentd-side and NOT in these hunks.

**Evidence:** Absent from standalone: crates/cerebro/src/types.rs:127-149 VisibilityScope has only agent_id (no shared_only field, no shared_only() ctor, sql_filter has no shared branch); crates/cerebro/src/cortex.rs:131 still `if scope.agent_id.is_none()` unguarded; recall schema in crates/cerebro-mcp/src/tools.rs has no visibility param (the tools.rs:31 'visibility' hit is the remember schema); grep 'shared_only|mesh_recall|federation' over crates/ = zero hits.

### `6f6af1e` feat(mesh): procedure replication — skills travel, trust is re-earned (slice 4) (#219)
*low / adapt — depends on f50ba92*

Cerebro hunks add the generic find_by_tags tool: SqliteStore::find_by_tags (exact-tag AND lookup over the tags JSON via `tags LIKE ? ESCAPE '\\'` with %/_/\\ escaped so a tag can't wildcard, scope-filtered, newest-first, limit-clamped), the MCP schema + dispatch arm (200-char content snippets), tool count 66→67, and an integration test (all-tags AND, per-tag sweep, exactness-not-substring, empty-tags→empty). Precise where recall is fuzzy — the provenance/cleanup query. The mesh_procedure_send wrapper, track_record_note, and receiver salience-drop are agentd-side, not in these hunks.

**Evidence:** Absent from standalone: grep find_by_tags over crates/ = zero hits; count test crates/cerebro-mcp/src/dispatch.rs:1466-1471 still asserts 66; no 'LIKE ? ESCAPE' in crates/cerebro/src/storage/sqlite.rs. Port prerequisites present: SELECT_COLS/row_to_raw/into_memory_node at sqlite.rs:102-207, agent_scope at dispatch.rs:1207, TOOL_NAMES at tools.rs:859. Gap: dispatch arm uses coerce_str_list, absent upstream (grep coerce = empty; helper is ApexOS f50ba92).

### `d568569` fix: assorted low-sev cleanups (key persist, apexos.conf, dream span) (#97)
*low / clean*

Only the cerebro hunk is upstream-relevant: save_dream_report bound both started_at and ended_at to the same `?3` (= now), so every stored dream report had ended_at == started_at and the cycle span was unrecoverable. Fix: ended_at = now (save happens at cycle end), started_at = now − report.total_duration_secs, bound as separate params (?3/?4). The gateway set_key error-surfacing and install.sh apexos.conf hunks are ApexOS-side, out of scope.

**Evidence:** ABSENT upstream — exact bug present: CerebroCortex-RS crates/cerebro/src/storage/sqlite.rs:1578-1595 save_dream_report has `let now = chrono::Utc::now().to_rfc3339();` and `VALUES (?1, ?2, ?3, ?3, ?4, ?5)` with params![id, agent_id, now, phases_json, metadata_json]. DreamReport.total_duration_secs is already in the metadata_json there, so the reconstruction patch applies as-is.

### `8319f57` chore: backlog sweep 2026-07 — re-grade 47 items, ship 8 quick fixes, harvest 13 forgotten (#221)
*low / clean*

Only two cerebro hunks in this large mixed commit. (1) sqlite.rs: one-time reap of Python-migration leftovers — in migrate_from_python's already-migrated branch, DROP TABLE IF EXISTS memory_nodes/associative_links/_py_agents/_py_episodes/_py_episode_steps/_py_audit_log (non-fatal on error; dropping memory_nodes also makes every future open skip the has_py probe). This is the substantive piece and is directly relevant upstream — the standalone is exactly the drop-in-on-a-Python-DB deployment. (2) tools.rs: header-comment refresh (stale '63 tools' → defer-to-count-test) — cosmetic, and the ApexOS text says 'currently 67' while the standalone's count is 66, so rewrite rather than copy.

Parts:
- sqlite.rs orphan-table reap in the already-migrated branch — port this
- tools.rs header comment — skip-grade (ApexOS-specific count 67; standalone is 66; reword if touched at all)

**Evidence:** Reap absent from standalone: crates/cerebro/src/storage/sqlite.rs:236-245 already-migrated branch is bare `if done { return Ok(()); }` — no DROP statements, no 'orphan' string anywhere in the file (the _py_ hits at 329-407 are the migration's RENAMEs, not the reap). Same stale header live at crates/cerebro-mcp/src/tools.rs:3-6 ('63 tools mirroring the Python MCP server'). Insertion site identical between trees (same migrate_from_python structure, migration marker version=100).

## Confirmed PRESENT (no action — the June mirror held)

- `c1f69d0` cerebro: port spreading-activation + scope-privacy + integrity fixes (C-RS-001/003/004/005/008/010)
- `36b0831` cerebro-mcp: panic isolation + JSON-RPC error codes + honest stubs (C-RS-002/006/007/012)
- `d8d6843` cerebro-api: scope-filter graph_neighbors, drop dead scope params (C-RS-009)
- `fc83ecf` clippy: make cerebro lint-clean (C-RS-013 parity with standalone) (#12)
- `9c59b26` cerebro: implement cognitive_bootstrap (CCBS) + wire recall reinforcement (CB-001)
- `1efa59b` distil skills from successful procedure clusters in dream phase 3 (#9)
- `58e54aa` surface distilled skills section in cognitive_bootstrap (#10)
- `aedf563` make procedure outcomes real selection pressure (failure demotes) (#11)
- `af7df0f` feat(cerebro): niche competition — procedures compete within a task niche (E1) (#109)
- `108fe21` feat(cerebro): variation — refine struggling procedures into fresh variants (E2) (#111)
- `b300efa` feat(cerebro): merge operator — recombine two strong same-niche procedures (E2b) (#112)
- `ae12184` feat(cerebro): champion-aware retrieval — surface the crowned procedure first (E1 follow-up) (#113)

## Skipped (cosmetic / superseded)

- `f69b656` docs, cleanup, and correctness — wave 6 complete (all 30 audit findings resolved) — cerebro hunks are pure compiler-warning silencing (F006): cerebro-api Query(q)->Query(_q) in [...]
- `77e0271` chore(clippy): sweep cosmetic style lints (29 → 7 warnings) (#96) — Cerebro hunks are pure clippy cosmetics: map_or(true,…)→is_none_or on the session_recall priority/type filters [...]
- `59006f9` chore(clippy): zero warnings workspace-wide (#107) — The sole cerebro hunk is a one-line clippy fix (`g.index.get(&a).is_none()` → `!g.index.contains_key(&a)`) inside [...]
- `de019ee` docs: add a one-paragraph README to every workspace crate (#157) — cerebro hunks are 4 new README.md files (cerebro, cerebro-mcp, cerebro-api, cerebro-cli — 51 lines total), each a [...]

---

*Post-backport check: run the full test suites of both repos plus a migration dry-run against a copy of the python-cerebro DB before swapping the dev MCP to -RS (the Wave-1 vec0/FTS5 fixes are exactly the write-path the real DB will hit).*
