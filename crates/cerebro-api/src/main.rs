use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::{Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use cerebro::{
    models::AssociativeLink,
    storage::ListFilter,
    types::{AgentId, LinkType, MemoryId, MemoryType, VisibilityScope},
    CerebroCortex,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

/// Constant-time string equality for auth tokens. Guards on length first
/// (lengths are not secret), then compares bytes via `subtle::ConstantTimeEq`
/// so a mismatch does not leak the matching-prefix length through timing.
fn ct_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

// ---------------------------------------------------------------------------
// State alias
// ---------------------------------------------------------------------------
type Brain = Arc<CerebroCortex>;
type AppResult<T = Value> = Result<Json<T>, ApiError>;

// ---------------------------------------------------------------------------
// Error helper — any anyhow error → 500 JSON
// ---------------------------------------------------------------------------
struct ApiError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self { Self(e.into()) }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(json!({ "error": self.0.to_string() }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

fn not_found(id: &str) -> ApiError {
    ApiError(anyhow::anyhow!("not found: {id}"))
}

/// CB-023: responder for `CatchPanicLayer`. Turns a caught handler panic into a
/// clean 500 JSON body shaped like `ApiError` (instead of an aborted connection
/// with no response), mirroring the MCP sibling's per-call panic isolation.
fn panic_response(err: Box<dyn std::any::Any + Send + 'static>) -> axum::response::Response {
    let msg = err
        .downcast_ref::<&str>().map(|s| s.to_string())
        .or_else(|| err.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "handler panicked".to_string());
    tracing::error!("cerebro-api: caught handler panic: {msg}");
    let body = Json(json!({ "error": format!("internal panic: {msg}") }));
    (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
}

fn scope_from(agent_id: Option<&str>) -> VisibilityScope {
    match agent_id {
        Some(a) if !a.is_empty() => VisibilityScope::for_agent(AgentId(a.to_string())),
        _ => VisibilityScope::global(),
    }
}

/// CB-012: canonicalize a session priority to uppercase, matching the MCP
/// `normalize_priority` (dispatch.rs) so a `priority:<p>` tag written here is
/// findable by an MCP `session_recall` priority filter (which compares against
/// the uppercased value). Keep this in lockstep with the MCP twin.
fn normalize_priority(p: &str) -> String {
    p.to_uppercase()
}

/// Round to 3 decimals for the wire (mirrors the MCP sibling's round3).
fn round3(x: f32) -> f64 {
    (x as f64 * 1000.0).round() / 1000.0
}

fn parse_link_type(s: &str) -> LinkType {
    match s {
        "causal"       => LinkType::Causal,
        "temporal"     => LinkType::Temporal,
        "supports"     => LinkType::Supports,
        "contradicts"  => LinkType::Contradicts,
        "affective"    => LinkType::Affective,
        "contextual"   => LinkType::Contextual,
        "derived_from" => LinkType::DerivedFrom,
        "part_of"      => LinkType::PartOf,
        _              => LinkType::Semantic,
    }
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RememberReq {
    content:     String,
    memory_type: Option<String>,
    tags:        Option<Vec<String>>,
    salience:    Option<f64>,
    agent_id:    Option<String>,
    /// private|shared|thread; absent → Shared (Python parity, R-05 — the
    /// MCP twin's contract, mirrored).
    visibility:  Option<String>,
}

/// Parse an optional visibility string the way the MCP twin does: absent →
/// None (cortex defaults Shared), unknown → hard error, never silent.
fn parse_visibility_opt(v: Option<&str>) -> Result<Option<cerebro::types::Visibility>> {
    use cerebro::types::Visibility;
    match v {
        None => Ok(None),
        Some(s) => match s.to_lowercase().as_str() {
            "private" => Ok(Some(Visibility::Private)),
            "shared"  => Ok(Some(Visibility::Shared)),
            "thread"  => Ok(Some(Visibility::Thread)),
            other => anyhow::bail!("unknown visibility '{other}' (private|shared|thread)"),
        },
    }
}

/// Best-effort audit write for API-surface mutations (Lucida U1b): the Live
/// lens tails the audit log, and what you do in the observatory must show in
/// its own EEG. Mirrors the MCP dispatch discipline — an audit failure never
/// fails the call it records.
async fn audit(
    brain: &Brain,
    agent: Option<&str>,
    action: &str,
    memory_id: Option<&str>,
    details: Option<&str>,
) {
    if let Err(e) = brain.storage.read().await.sqlite
        .log_audit_event(agent, action, memory_id, details).await
    {
        tracing::warn!("api audit write failed for {action}: {e}");
    }
}

#[derive(Deserialize)]
struct RecallReq {
    query:    String,
    top_k:    Option<usize>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct AssociateReq {
    source_id: String,
    target_id: String,
    link_type: Option<String>,
    weight:    Option<f64>,
}

#[derive(Deserialize)]
struct UpdateMemoryReq {
    content:    Option<String>,
    tags:       Option<Vec<String>>,
    salience:   Option<f64>,
    visibility: Option<String>,
}

#[derive(Deserialize)]
struct EpisodeStartReq {
    title:    String,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct EpisodeStepReq {
    memory_id: String,
    role:      Option<String>,
}

#[derive(Deserialize)]
struct EpisodeEndReq {
    summary: Option<String>,
}

#[derive(Deserialize)]
struct SessionSaveReq {
    content:      String,
    priority:     Option<String>,
    session_type: Option<String>,
    agent_id:     Option<String>,
}

#[derive(Deserialize)]
struct RegisterAgentReq {
    agent_id:     String,
    display_name: String,
    symbol:       Option<String>,
    color:        Option<String>,
}

#[derive(Deserialize)]
struct IntentionReq {
    content:  String,
    tags:     Option<Vec<String>>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct CreateSchemaReq {
    content:    String,
    tags:       Option<Vec<String>>,
    source_ids: Option<Vec<String>>,
    agent_id:   Option<String>,
}

#[derive(Deserialize)]
struct StoreProcedureReq {
    content:  String,
    tags:     Option<Vec<String>>,
    agent_id: Option<String>,
}

#[derive(Deserialize)]
struct RenameTagReq {
    old_tag:  String,
    new_tag:  String,
}

#[derive(Deserialize)]
struct MergeTagsReq {
    source_tags: Vec<String>,
    target_tag:  String,
}

#[derive(Deserialize)]
struct BulkDeleteReq {
    ids: Vec<String>,
}

#[derive(Deserialize, Default)]
struct DreamRunQuery {
    agent_id:      Option<String>,
    #[serde(default = "default_max_llm_calls")]
    max_llm_calls: usize,
}
fn default_max_llm_calls() -> usize { 20 }

// ---------------------------------------------------------------------------
// Query param structs
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct AgentQuery { agent_id: Option<String> }

#[derive(Deserialize)]
struct LimitQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    agent_id: Option<String>,
}
fn default_limit() -> usize { 50 }

#[derive(Deserialize)]
struct RecallQuery {
    query:    String,
    #[serde(default = "default_top_k")]
    top_k:    usize,
    agent_id: Option<String>,
    // CB-026: honor the same priority/session_type filters the MCP session_recall
    // twin applies, so the HTTP surface returns the same result set.
    priority:     Option<String>,
    session_type: Option<String>,
}
fn default_top_k() -> usize { 10 }

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

// stats is a global endpoint — memory_stats aggregates the whole store
// (C-RS-009: dropped the unused agent_id query param).
async fn stats(
    State(brain): State<Brain>,
) -> AppResult {
    let v = brain.storage.read().await.sqlite.memory_stats().await?;
    Ok(Json(v))
}

// GET /q/:query
async fn quick_search(
    Path(query): Path<String>,
    Query(q): Query<LimitQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope   = scope_from(q.agent_id.as_deref());
    let results = brain.recall(&query, q.limit, scope).await?;
    let arr: Vec<Value> = results.into_iter()
        .map(|(n, s)| json!({ "score": s, "memory": n }))
        .collect();
    Ok(Json(Value::Array(arr)))
}

// POST /remember
async fn remember(
    State(brain): State<Brain>,
    Json(req): Json<RememberReq>,
) -> AppResult {
    let mt: Option<MemoryType> = req.memory_type
        .and_then(|s| serde_json::from_value(Value::String(s)).ok());
    let scope = scope_from(req.agent_id.as_deref());
    let vis   = parse_visibility_opt(req.visibility.as_deref())?;
    let node  = brain.remember_with_visibility(
        req.content, mt, req.tags, req.salience.map(|f| f as f32), scope, vis,
    ).await?;
    audit(&brain, req.agent_id.as_deref(), "remember", Some(&node.id.0), None).await;
    Ok(Json(serde_json::to_value(&node)?))
}

// POST /recall
async fn recall(
    State(brain): State<Brain>,
    Json(req): Json<RecallReq>,
) -> AppResult {
    let scope   = scope_from(req.agent_id.as_deref());
    let results = brain.recall(&req.query, req.top_k.unwrap_or(10), scope).await?;
    let arr: Vec<Value> = results.into_iter()
        .map(|(n, s)| json!({ "score": s, "memory": n }))
        .collect();
    Ok(Json(Value::Array(arr)))
}

// GET /memory/:id
async fn get_memory(
    Path(id): Path<String>,
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope = scope_from(q.agent_id.as_deref());
    let node  = brain.storage.read().await.sqlite
        .get_memory(&MemoryId(id.clone()), &scope).await?
        .ok_or_else(|| not_found(&id))?;
    Ok(Json(serde_json::to_value(&node)?))
}

// PUT /memory/:id
async fn update_memory(
    Path(id): Path<String>,
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
    Json(req): Json<UpdateMemoryReq>,
) -> AppResult {
    let scope   = scope_from(q.agent_id.as_deref());
    let storage = brain.storage.read().await;
    let mut node = storage.sqlite
        .get_memory(&MemoryId(id.clone()), &scope).await?
        .ok_or_else(|| not_found(&id))?;
    let content_changed = req.content.is_some();
    if let Some(c) = req.content  { node.content  = c; }
    if let Some(t) = req.tags     { node.tags      = t; }
    if let Some(s) = req.salience { node.salience  = s as f32; }
    // Visibility change mirrors the MCP orphan guard: private + no owner
    // would be visible to no one.
    if let Some(vis) = parse_visibility_opt(req.visibility.as_deref())? {
        if vis == cerebro::types::Visibility::Private && node.agent_id.is_none() {
            return Err(anyhow::anyhow!(
                "refusing to privatize an owner-less memory (it would be visible to no one)"
            ).into());
        }
        node.visibility = vis;
    }
    // Content edits snapshot the prior row store-side (R-04); the editor
    // identity is the caller's scope.
    storage.sqlite.update_memory_noted(&node, q.agent_id.as_deref(), None).await?;
    // CB-006: mirror the MCP update path — re-embed when content changed so the
    // vector index does not point at the pre-edit text (sqlite.update_memory only
    // refreshes the content column + FTS5 trigger, never the embedding/vec0 row).
    if content_changed {
        storage.vector.embed_and_store(&node.id, &node.content).await?;
    }
    drop(storage);
    audit(&brain, q.agent_id.as_deref(), "update_memory", Some(&id), None).await;
    Ok(Json(serde_json::to_value(&node)?))
}

// DELETE /memory/:id
async fn delete_memory(
    Path(id): Path<String>,
    State(brain): State<Brain>,
) -> AppResult {
    // R-08: go through the coordinator wrapper (write guard) so the graph
    // evicts the node too — the raw sqlite call left deleted memories
    // spreading activation until restart.
    let ok = brain.storage.write().await
        .delete_memory(&MemoryId(id.clone()), &VisibilityScope::global()).await?;
    if ok {
        audit(&brain, None, "delete_memory", Some(&id), None).await;
    }
    Ok(Json(json!({ "deleted": ok })))
}

// GET /memory/:id/versions
async fn get_memory_versions(
    Path(id): Path<String>,
    Query(q): Query<LimitQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let versions = brain.storage.read().await.sqlite
        .get_memory_versions_raw(&id, q.limit).await?;
    Ok(Json(Value::Array(versions)))
}

// POST /associate
async fn associate(
    State(brain): State<Brain>,
    Json(req): Json<AssociateReq>,
) -> AppResult {
    let src  = MemoryId(req.source_id.clone());
    let tgt  = MemoryId(req.target_id.clone());
    let link = AssociativeLink {
        source_id:       src.clone(),
        target_id:       tgt.clone(),
        link_type:       parse_link_type(req.link_type.as_deref().unwrap_or("semantic")),
        weight:          req.weight.unwrap_or(0.5) as f32,
        created_at:      Utc::now(),
        last_traversed:  None,
        traversal_count: 0,
    };
    brain.associate(src, tgt, link).await?;
    audit(&brain, None, "associate", Some(&req.source_id),
        Some(&format!("→ {}", req.target_id))).await;
    Ok(Json(json!({ "status": "ok" })))
}

// ---------------------------------------------------------------------------
// Episodes
// ---------------------------------------------------------------------------

async fn list_episodes(
    Query(q): Query<LimitQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let eps = brain.storage.read().await.sqlite
        .list_episodes(q.agent_id.as_deref(), q.limit).await?;
    Ok(Json(Value::Array(eps)))
}

async fn episode_start(
    State(brain): State<Brain>,
    Json(req): Json<EpisodeStartReq>,
) -> AppResult {
    let ep_id = format!("ep_{}", uuid::Uuid::new_v4().simple());
    brain.storage.read().await.sqlite
        .create_episode(&ep_id, Some(&req.title), req.agent_id.as_deref(), None).await?;
    Ok(Json(json!({ "episode_id": ep_id, "title": req.title })))
}

async fn episode_add_step(
    Path(episode_id): Path<String>,
    State(brain): State<Brain>,
    Json(req): Json<EpisodeStepReq>,
) -> AppResult {
    let step_index = {
        let ids = brain.storage.read().await.sqlite
            .get_episode_memory_ids(&episode_id).await?;
        ids.len() as i64
    };
    brain.storage.read().await.sqlite.add_episode_step(
        &episode_id,
        step_index,
        req.role.as_deref().unwrap_or("memory"),
        Some(&req.memory_id),
    ).await?;
    Ok(Json(json!({ "status": "ok" })))
}

async fn episode_end(
    Path(episode_id): Path<String>,
    State(brain): State<Brain>,
    Json(req): Json<EpisodeEndReq>,
) -> AppResult {
    let ok = brain.storage.read().await.sqlite
        .end_episode(&episode_id, req.summary.as_deref()).await?;
    Ok(Json(json!({ "ended": ok })))
}

async fn get_episode(
    Path(episode_id): Path<String>,
    State(brain): State<Brain>,
) -> AppResult {
    let ep = brain.storage.read().await.sqlite
        .get_episode_raw(&episode_id).await?
        .ok_or_else(|| not_found(&episode_id))?;
    Ok(Json(ep))
}

async fn get_episode_memories(
    Path(episode_id): Path<String>,
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope  = scope_from(q.agent_id.as_deref());
    let ids    = brain.storage.read().await.sqlite
        .get_episode_memory_ids(&episode_id).await?;
    let nodes  = brain.storage.read().await.sqlite
        .get_memories_by_ids(&ids, &scope).await?;
    let arr: Vec<Value> = nodes.into_iter()
        .map(|n| serde_json::to_value(&n).unwrap_or_default())
        .collect();
    Ok(Json(Value::Array(arr)))
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

async fn session_save(
    State(brain): State<Brain>,
    Json(req): Json<SessionSaveReq>,
) -> AppResult {
    let priority     = normalize_priority(req.priority.as_deref().unwrap_or("MEDIUM"));
    let session_type = req.session_type.as_deref().unwrap_or("general");
    let mut tags = vec![
        "session_note".to_string(),
        format!("priority:{priority}"),
        format!("session_type:{session_type}"),
    ];
    if let Some(ref aid) = req.agent_id {
        if !aid.is_empty() { tags.push(format!("agent:{aid}")); }
    }
    let scope = scope_from(req.agent_id.as_deref());
    let node  = brain.remember(
        req.content, Some(MemoryType::Episodic), Some(tags), Some(0.8), scope,
    ).await?;
    Ok(Json(serde_json::to_value(&node)?))
}

async fn session_recall(
    Query(q): Query<RecallQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope   = scope_from(q.agent_id.as_deref());
    let priority_filter = q.priority.as_deref();
    let type_filter     = q.session_type.as_deref();
    // Over-fetch so the tag filters don't deplete results (matches MCP twin).
    let results = brain.recall(&q.query, q.top_k * 5, scope).await?;
    let arr: Vec<Value> = results.into_iter()
        .filter(|(n, _)| n.tags.iter().any(|t| t == "session_note"))
        .filter(|(n, _)| priority_filter.is_none_or(|p| {
            let want = format!("priority:{}", normalize_priority(p));
            n.tags.iter().any(|t| t == &want)
        }))
        .filter(|(n, _)| type_filter.is_none_or(|st|
            n.tags.iter().any(|t| t == &format!("session_type:{st}"))))
        .take(q.top_k)
        .map(|(n, s)| json!({ "score": s, "memory": n }))
        .collect();
    Ok(Json(Value::Array(arr)))
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

async fn list_agents(State(brain): State<Brain>) -> AppResult {
    let agents = brain.storage.read().await.sqlite.list_agents().await?;
    Ok(Json(Value::Array(agents)))
}

async fn register_agent(
    State(brain): State<Brain>,
    Json(req): Json<RegisterAgentReq>,
) -> AppResult {
    let metadata = json!({
        "symbol": req.symbol,
        "color":  req.color,
    });
    brain.storage.read().await.sqlite.register_agent(
        &req.agent_id,
        &req.display_name,
        None,
        &metadata,
    ).await?;
    Ok(Json(json!({ "agent_id": req.agent_id, "status": "ok" })))
}

// ---------------------------------------------------------------------------
// Health / diagnostics
// ---------------------------------------------------------------------------

async fn memory_health(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let v = brain.storage.read().await.sqlite
        .memory_health(&scope_from(q.agent_id.as_deref())).await?;
    Ok(Json(v))
}

async fn emotional_summary(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let v = brain.storage.read().await.sqlite
        .emotional_summary(&scope_from(q.agent_id.as_deref())).await?;
    Ok(Json(v))
}

// ---------------------------------------------------------------------------
// Graph
// ---------------------------------------------------------------------------

async fn graph_stats(State(brain): State<Brain>) -> AppResult {
    let storage = brain.storage.read().await;
    let links   = storage.sqlite.list_all_links().await?;
    let ids     = storage.sqlite.list_all_memory_ids().await?;
    Ok(Json(json!({ "nodes": ids.len(), "edges": links.len() })))
}

async fn graph_neighbors(
    Path(memory_id): Path<String>,
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    // C-RS-009: honor agent scope — only return neighbors the caller can see,
    // consistent with the recall routes (was returning every neighbor id).
    let scope   = scope_from(q.agent_id.as_deref());
    let storage = brain.storage.read().await;
    let neighbor_ids: Vec<MemoryId> = storage.graph
        .neighbors(&MemoryId(memory_id))
        .into_iter().cloned().collect();
    let visible = storage.sqlite.get_memories_by_ids(&neighbor_ids, &scope).await?;
    let ids: Vec<Value> = visible.iter().map(|n| json!(n.id.0)).collect();
    Ok(Json(Value::Array(ids)))
}

async fn graph_path(
    Path((source_id, target_id)): Path<(String, String)>,
    State(brain): State<Brain>,
) -> AppResult {
    let storage = brain.storage.read().await;
    let path    = brain.association.find_path(
        &storage.graph,
        &MemoryId(source_id),
        &MemoryId(target_id),
    );
    let ids: Vec<Value> = path.as_ref()
        .map(|p| p.iter().map(|id| json!(id.0)).collect())
        .unwrap_or_default();
    Ok(Json(Value::Array(ids)))
}

async fn graph_common(
    Path((id_a, id_b)): Path<(String, String)>,
    State(brain): State<Brain>,
) -> AppResult {
    let storage = brain.storage.read().await;
    let common  = brain.association.get_common_neighbors(
        &storage.graph, &MemoryId(id_a), &MemoryId(id_b),
    );
    let ids: Vec<Value> = common.iter().map(|id| json!(id.0)).collect();
    Ok(Json(Value::Array(ids)))
}

// ---------------------------------------------------------------------------
// Lucida (U1): graph export + cached semantic layout + embedded observatory UI
// ---------------------------------------------------------------------------

/// Char-boundary-safe head of a string (the wire_summary doctrine: a graph
/// export is an index, not the texts — full bodies come from GET /memory/:id).
fn head_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[derive(Deserialize)]
struct GraphExportQuery {
    agent_id: Option<String>,
    cap:      Option<usize>,
    /// RFC3339 instant to project decay to (Lucida U6 time-lapse). Forward
    /// only — past instants clamp to now, because rewinding honestly would
    /// need access-history filtering, not just a different `t`.
    at:       Option<String>,
}

/// Resolve the projection instant: absent → now (live export); present →
/// parsed RFC3339 clamped forward. Second field says whether a projection
/// was requested (drives the `projected_at` echo).
fn resolve_projection(
    at: Option<&str>,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<(chrono::DateTime<Utc>, bool)> {
    match at {
        None => Ok((now, false)),
        Some(s) => {
            let t = chrono::DateTime::parse_from_rfc3339(s)
                .map_err(|e| anyhow::anyhow!("bad ?at= (want RFC3339): {e}"))?
                .with_timezone(&Utc);
            Ok((t.max(now), true))
        }
    }
}

/// Every visual channel the field needs, one round-trip. Nodes carry live
/// ACT-R/FSRS numbers (computed here, not stored); edges carry the decayed
/// effective weight and traversal stamps. Scope-filtered like any recall.
async fn graph_export(
    Query(q): Query<GraphExportQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    use std::collections::HashSet;
    let scope = scope_from(q.agent_id.as_deref());
    let cap   = q.cap.unwrap_or(4000).clamp(1, 20_000);
    let now   = Utc::now();
    // Every decay channel below (ACT-R, FSRS, link half-life) is computed at
    // `t` — with ?at= the whole sky dims together, same math, later clock.
    let (t, projected) = resolve_projection(q.at.as_deref(), now)?;

    let storage = brain.storage.read().await;
    let mut nodes = storage.sqlite.list_memories_scoped(
        &scope,
        &ListFilter { limit: cap + 1, ..Default::default() },
    ).await?;
    let truncated = nodes.len() > cap;
    nodes.truncate(cap);
    let embedded_ids = storage.sqlite.list_embedded_ids().await?;

    let node_wire: Vec<Value> = nodes.iter().map(|n| {
        let activation = cerebro::activation::base_level_activation(
            &n.access_times, t, cerebro::config::ACTR_DECAY_RATE,
        );
        // Sigmoid-mapped to [0,1] for the glow channel (raw B(t) is unbounded).
        let glow = cerebro::activation::recall_probability(
            activation, cerebro::config::ACTR_RETRIEVAL_THRESHOLD, cerebro::config::ACTR_NOISE,
        );
        // Elapsed from last review, falling back to creation — never pinned
        // at 1.0 for the never-recalled (the R-21 trap, not repeated here).
        let elapsed_days = (t - n.strength.last_review.unwrap_or(n.created_at))
            .num_seconds().max(0) as f32 / 86_400.0;
        let retr = cerebro::activation::retrievability(elapsed_days, n.strength.stability);
        json!({
            "id":            n.id.0,
            "memory_type":   n.memory_type,
            "layer":         n.layer,
            "salience":      round3(n.salience),
            "tags":          n.tags,
            "agent_id":      n.agent_id.as_ref().map(|a| a.0.clone()),
            "visibility":    n.visibility,
            "content_head":  head_chars(&n.content, 200),
            "content_chars": n.content.chars().count(),
            "created_at":    n.created_at.to_rfc3339(),
            "access_count":  n.access_count,
            "activation":    round3(glow),
            "retrievability": round3(retr),
            "valence":       n.emotional_valence,
            "intensity":     round3(n.emotional_intensity),
            // U6: rim-label honesty (embedded-but-unplaced ≠ no embedding)
            // and the at-risk gutter's filter (the engine's activation_at_risk
            // only considers reviewed rows).
            "embedded":      embedded_ids.contains(&n.id.0),
            "reviewed":      n.strength.last_review.is_some(),
        })
    }).collect();

    let ids: HashSet<&str> = nodes.iter().map(|n| n.id.0.as_str()).collect();
    let links = storage.sqlite.list_all_links().await?;
    let edge_wire: Vec<Value> = links.iter()
        .filter(|l| ids.contains(l.source_id.0.as_str()) && ids.contains(l.target_id.0.as_str()))
        .map(|l| json!({
            "source":           l.source_id.0,
            "target":           l.target_id.0,
            "link_type":        l.link_type,
            "weight":           round3(l.weight),
            "effective_weight": round3(l.effective_weight(t, cerebro::config::LINK_DECAY_HALFLIFE_DAYS)),
            "traversal_count":  l.traversal_count,
            "last_traversed":   l.last_traversed.map(|ts| ts.to_rfc3339()),
        }))
        .collect();

    Ok(Json(json!({
        "nodes":        node_wire,
        "edges":        edge_wire,
        "truncated":    truncated,
        "generated_at": now.to_rfc3339(),
        // Honest labeling for the time-lapse: null on a live export, the
        // clamped projection instant when ?at= was asked for.
        "projected_at": projected.then(|| t.to_rfc3339()),
    })))
}

#[derive(Deserialize)]
struct RecallTraceReq {
    query:    String,
    top_k:    Option<usize>,
    agent_id: Option<String>,
}

/// POST /recall/trace — a REAL recall (same pipeline, same reinforcement:
/// watching a thought is still thinking it) plus the trace the Thought lens
/// animates: seeds with similarities, every spread walk in firing order,
/// and the post-spread activation map. Results are summary rows — the field
/// fetches full bodies from /memory/:id on select.
async fn recall_trace(
    State(brain): State<Brain>,
    Json(req): Json<RecallTraceReq>,
) -> AppResult {
    let scope = scope_from(req.agent_id.as_deref());
    let (results, trace) = brain
        .recall_traced(&req.query, req.top_k.unwrap_or(10), scope)
        .await?;
    let rows: Vec<Value> = results.iter().map(|(n, s)| json!({
        "id":            n.id.0,
        "memory_type":   n.memory_type,
        "content_head":  head_chars(&n.content, 200),
        "content_chars": n.content.chars().count(),
        "tags":          n.tags,
        "salience":      round3(n.salience),
        "score":         round3(*s),
    })).collect();
    Ok(Json(json!({ "results": rows, "trace": trace })))
}

/// Top-2 PCA of the (mean-centered) embedding matrix via power iteration,
/// axes independently scaled to [-1, 1]. Deterministic (fixed start vector),
/// dependency-free, and honest about what it is: a stable *semantic* map, not
/// a force simulation — the sky must not reshuffle between visits.
fn pca_2d(embeddings: &[(MemoryId, Vec<f32>)]) -> Vec<(MemoryId, f32, f32)> {
    let n = embeddings.len();
    if n == 0 { return vec![]; }
    let dim = embeddings[0].1.len();
    if n == 1 || dim == 0 {
        return embeddings.iter().map(|(id, _)| (id.clone(), 0.0, 0.0)).collect();
    }

    // Mean-center.
    let mut mean = vec![0.0f32; dim];
    for (_, e) in embeddings {
        for (m, v) in mean.iter_mut().zip(e) { *m += v; }
    }
    for m in &mut mean { *m /= n as f32; }
    let centered: Vec<Vec<f32>> = embeddings.iter()
        .map(|(_, e)| e.iter().zip(&mean).map(|(v, m)| v - m).collect())
        .collect();

    // Power iteration for one principal axis of `rows`, seeded deterministically.
    let principal = |rows: &[Vec<f32>]| -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim).map(|i| 1.0 + (i as f32) * 1e-3).collect();
        for _ in 0..60 {
            // w = Σ_r (r·v) r   (covariance-matrix product without the matrix)
            let mut w = vec![0.0f32; dim];
            for r in rows {
                let dot: f32 = r.iter().zip(&v).map(|(a, b)| a * b).sum();
                for (wi, ri) in w.iter_mut().zip(r) { *wi += dot * ri; }
            }
            let norm = w.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < 1e-12 { break; }
            for x in &mut w { *x /= norm; }
            v = w;
        }
        v
    };

    let pc1 = principal(&centered);
    // Deflate: remove the PC1 component, then find PC2 in the residual.
    let deflated: Vec<Vec<f32>> = centered.iter().map(|r| {
        let dot: f32 = r.iter().zip(&pc1).map(|(a, b)| a * b).sum();
        r.iter().zip(&pc1).map(|(a, p)| a - dot * p).collect()
    }).collect();
    let pc2 = principal(&deflated);

    let mut coords: Vec<(MemoryId, f32, f32)> = embeddings.iter().zip(&centered)
        .map(|((id, _), r)| {
            let x: f32 = r.iter().zip(&pc1).map(|(a, b)| a * b).sum();
            let y: f32 = r.iter().zip(&pc2).map(|(a, b)| a * b).sum();
            (id.clone(), x, y)
        })
        .collect();

    // Scale each axis to [-1, 1] so the client maps to screen space directly.
    let max_x = coords.iter().map(|c| c.1.abs()).fold(1e-9f32, f32::max);
    let max_y = coords.iter().map(|c| c.2.abs()).fold(1e-9f32, f32::max);
    for c in &mut coords {
        c.1 /= max_x;
        c.2 /= max_y;
    }
    coords
}

async fn layout_inner(brain: Brain, force: bool) -> AppResult {
    let storage = brain.storage.read().await;
    let (mut rows, mut stamp) = storage.sqlite.get_layout().await?;
    let embedded = storage.sqlite.count_embedded().await?;
    // Recompute when forced, or when the cache covers under 80% of the
    // embedded set (new memories drift in; a dream can prune old ones).
    let stale = force || (embedded > 0 && rows.len() * 5 < embedded * 4);
    if stale {
        let embeddings = storage.sqlite.list_embeddings().await?;
        rows = pca_2d(&embeddings);
        storage.sqlite.replace_layout(&rows).await?;
        stamp = Some(Utc::now().to_rfc3339());
    }
    let coords: serde_json::Map<String, Value> = rows.into_iter()
        .map(|(id, x, y)| (id.0, json!([round3(x), round3(y)])))
        .collect();
    Ok(Json(json!({
        "coords":      coords,
        "count":       coords.len(),
        "embedded":    embedded,
        "computed_at": stamp,
    })))
}

async fn graph_layout(State(brain): State<Brain>) -> AppResult {
    layout_inner(brain, false).await
}

async fn graph_layout_recompute(State(brain): State<Brain>) -> AppResult {
    layout_inner(brain, true).await
}

/// The observatory's identity line (Lucida U1b): which skull are you inside?
/// Set once in main() from the resolved config; "unknown" in router tests.
static DB_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

async fn meta() -> AppResult {
    Ok(Json(json!({
        "db_path": DB_LABEL.get().map(String::as_str).unwrap_or("unknown"),
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

#[derive(Deserialize)]
struct EventsQuery {
    /// Replay from this audit rowid (exclusive). Default: only what happens
    /// after connect (MAX(id) at open).
    since:   Option<i64>,
    /// Poll cadence in ms, clamped 250..=5000. A U1b settings-drawer knob.
    poll_ms: Option<u64>,
}

/// GET /audit/since/{id} — the audit tail as plain JSON (rows strictly after
/// the given rowid, oldest first, capped at 200). The Live lens uses this to
/// replay history at boot before opening the SSE stream; also the REST
/// surface's first audit read.
async fn audit_since(
    Path(id): Path<i64>,
    State(brain): State<Brain>,
) -> AppResult {
    let rows = brain.storage.read().await.sqlite.list_audit_since(id, 200).await?;
    Ok(Json(json!({ "rows": rows })))
}

/// GET /events — the Live lens (Lucida U3): an SSE tail of the audit log.
/// Every mutating MCP tool call leaves an audit row (colony C3); this streams
/// them as they land, so the observatory shows the brain being used — no IPC,
/// the shared SQLite is already the bus. Each SSE message is a JSON array of
/// audit rows (one poll batch); silence is covered by keep-alive comments.
/// EventSource can't set headers, so auth rides the ?token= query param the
/// middleware already accepts.
async fn events(
    Query(q): Query<EventsQuery>,
    State(brain): State<Brain>,
) -> impl IntoResponse {
    use axum::response::sse::{Event, KeepAlive, Sse};
    let poll = std::time::Duration::from_millis(q.poll_ms.unwrap_or(1000).clamp(250, 5000));

    let stream = async_stream::stream! {
        let mut cursor: i64 = match q.since {
            Some(s) => s,
            None => brain.storage.read().await.sqlite.max_audit_id().await.unwrap_or(0),
        };
        let mut tick = tokio::time::interval(poll);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let batch = brain.storage.read().await.sqlite
                .list_audit_since(cursor, 200).await;
            match batch {
                Ok(rows) if !rows.is_empty() => {
                    if let Some(last) = rows.last().and_then(|r| r["id"].as_i64()) {
                        cursor = last;
                    }
                    let data = serde_json::to_string(&rows)
                        .unwrap_or_else(|_| "[]".to_string());
                    yield Ok::<_, std::convert::Infallible>(
                        Event::default().event("audit").data(data));
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("/events poll failed: {e}");
                }
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// The observatory itself — three embedded files, one binary (house rule).
async fn ui_index() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")],
     include_str!("../../../ui-web/index.html"))
}
async fn ui_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")],
     include_str!("../../../ui-web/style.css"))
}
async fn ui_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
     include_str!("../../../ui-web/app.js"))
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

async fn list_tags(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let tags = brain.storage.read().await.sqlite
        .list_tags(&scope_from(q.agent_id.as_deref())).await?;
    Ok(Json(Value::Array(tags)))
}

async fn rename_tag(
    State(brain): State<Brain>,
    Json(req): Json<RenameTagReq>,
) -> AppResult {
    let count = brain.storage.read().await.sqlite
        .rename_tag_everywhere(&req.old_tag, &req.new_tag, &VisibilityScope::global()).await?;
    Ok(Json(json!({ "updated": count })))
}

async fn merge_tags(
    State(brain): State<Brain>,
    Json(req): Json<MergeTagsReq>,
) -> AppResult {
    let mut total = 0usize;
    let storage = brain.storage.read().await;
    for src in &req.source_tags {
        total += storage.sqlite.rename_tag_everywhere(src, &req.target_tag, &VisibilityScope::global()).await?;
    }
    Ok(Json(json!({ "updated": total })))
}

async fn delete_tag(
    Path(tag): Path<String>,
    State(brain): State<Brain>,
) -> AppResult {
    let count = brain.storage.read().await.sqlite
        .delete_tag_everywhere(&tag, &VisibilityScope::global()).await?;
    Ok(Json(json!({ "removed_from": count })))
}

// ---------------------------------------------------------------------------
// Intentions
// ---------------------------------------------------------------------------

async fn store_intention(
    State(brain): State<Brain>,
    Json(req): Json<IntentionReq>,
) -> AppResult {
    let scope = scope_from(req.agent_id.as_deref());
    let mut tags = vec!["intention".to_string()];
    if let Some(t) = req.tags { tags.extend(t); }
    let node = brain.remember(
        req.content, Some(MemoryType::Prospective), Some(tags), Some(0.7), scope,
    ).await?;
    Ok(Json(serde_json::to_value(&node)?))
}

async fn list_intentions(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let nodes = brain.storage.read().await.sqlite
        .list_memories_scoped(
            &scope_from(q.agent_id.as_deref()),
            &ListFilter { memory_type: Some(MemoryType::Prospective), limit: 100, ..Default::default() },
        ).await?;
    let active: Vec<Value> = nodes.into_iter()
        .filter(|n| !n.tags.iter().any(|t| t == "status:resolved"))
        .map(|n| serde_json::to_value(&n).unwrap_or_default())
        .collect();
    Ok(Json(Value::Array(active)))
}

async fn resolve_intention(
    Path(memory_id): Path<String>,
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let mid  = MemoryId(memory_id.clone());
    let scope = scope_from(q.agent_id.as_deref());
    let storage = brain.storage.read().await;
    let mut node = storage.sqlite.get_memory(&mid, &scope).await?
        .ok_or_else(|| not_found(&memory_id))?;
    node.tags.retain(|t| !t.starts_with("status:"));
    node.tags.push("status:resolved".into());
    node.salience = 0.1;
    storage.sqlite.update_memory(&node).await?;
    Ok(Json(json!({ "status": "resolved", "id": memory_id })))
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

async fn create_schema(
    State(brain): State<Brain>,
    Json(req): Json<CreateSchemaReq>,
) -> AppResult {
    let scope = scope_from(req.agent_id.as_deref());
    let mut tags = vec!["schema".to_string(), "support_count:0".to_string()];
    if let Some(t) = req.tags { tags.extend(t); }
    let node = brain.remember(
        req.content, Some(MemoryType::Schematic), Some(tags), Some(0.7), scope,
    ).await?;
    if let Some(sources) = req.source_ids {
        if !sources.is_empty() {
            let mut n = node.clone();
            if let serde_json::Value::Object(ref mut map) = n.metadata {
                map.insert("derived_from".into(), json!(sources));
            } else {
                n.metadata = json!({ "derived_from": sources });
            }
            brain.storage.read().await.sqlite.update_memory(&n).await?;
        }
    }
    Ok(Json(serde_json::to_value(&node)?))
}

async fn list_schemas(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let nodes = brain.storage.read().await.sqlite
        .list_memories_scoped(
            &scope_from(q.agent_id.as_deref()),
            &ListFilter { memory_type: Some(MemoryType::Schematic), limit: 100, ..Default::default() },
        ).await?;
    let arr: Vec<Value> = nodes.into_iter()
        .map(|n| serde_json::to_value(&n).unwrap_or_default())
        .collect();
    Ok(Json(Value::Array(arr)))
}

// ---------------------------------------------------------------------------
// Procedures
// ---------------------------------------------------------------------------

async fn store_procedure(
    State(brain): State<Brain>,
    Json(req): Json<StoreProcedureReq>,
) -> AppResult {
    let scope = scope_from(req.agent_id.as_deref());
    let mut tags = vec!["procedure".to_string()];
    if let Some(t) = req.tags { tags.extend(t); }
    let node = brain.remember(
        req.content, Some(MemoryType::Procedural), Some(tags), Some(0.8), scope,
    ).await?;
    Ok(Json(serde_json::to_value(&node)?))
}

async fn list_procedures(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let nodes = brain.storage.read().await.sqlite
        .list_memories_scoped(
            &scope_from(q.agent_id.as_deref()),
            &ListFilter { memory_type: Some(MemoryType::Procedural), limit: 100, ..Default::default() },
        ).await?;
    let arr: Vec<Value> = nodes.into_iter()
        .map(|n| serde_json::to_value(&n).unwrap_or_default())
        .collect();
    Ok(Json(Value::Array(arr)))
}

// ---------------------------------------------------------------------------
// Trash
// ---------------------------------------------------------------------------

async fn list_trash(
    Query(q): Query<LimitQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope   = scope_from(q.agent_id.as_deref());
    let deleted = brain.storage.read().await.sqlite
        .list_deleted_memories(&scope, q.limit).await?;
    let arr: Vec<Value> = deleted.into_iter()
        .map(|n| serde_json::to_value(&n).unwrap_or_default())
        .collect();
    Ok(Json(Value::Array(arr)))
}

async fn restore_trash(
    Path(memory_id): Path<String>,
    State(brain): State<Brain>,
) -> AppResult {
    // R-08: coordinator wrappers (write guard) keep the graph in step —
    // restore rebuilds the node AND its links; the raw sqlite calls left the
    // graph stale until restart.
    let ok = brain.storage.write().await
        .restore_memory(&MemoryId(memory_id.clone()), &VisibilityScope::global()).await?;
    if ok {
        audit(&brain, None, "restore_memory", Some(&memory_id), None).await;
    }
    Ok(Json(json!({ "restored": ok })))
}

async fn purge_trash(
    Path(memory_id): Path<String>,
    State(brain): State<Brain>,
) -> AppResult {
    let ok = brain.storage.write().await
        .purge_memory(&MemoryId(memory_id.clone()), &VisibilityScope::global()).await?;
    if ok {
        audit(&brain, None, "purge_memory", Some(&memory_id), None).await;
    }
    Ok(Json(json!({ "purged": ok })))
}

async fn purge_all_trash(
    State(brain): State<Brain>,
) -> AppResult {
    let count = brain.storage.read().await.sqlite
        .purge_all_deleted(&VisibilityScope::global()).await?;
    if count > 0 {
        audit(&brain, None, "purge_all_deleted", None,
            Some(&format!("{count} purged"))).await;
    }
    Ok(Json(json!({ "purged": count })))
}

async fn bulk_delete(
    State(brain): State<Brain>,
    Json(req): Json<BulkDeleteReq>,
) -> AppResult {
    // R-08: wrapper form — evicts each actually-deleted id from the graph.
    let ids: Vec<MemoryId> = req.ids.into_iter().map(MemoryId).collect();
    let count = brain.storage.write().await
        .bulk_delete(&ids, &VisibilityScope::global()).await?;
    if count > 0 {
        audit(&brain, None, "bulk_delete", None, Some(&format!("{count} deleted"))).await;
    }
    Ok(Json(json!({ "deleted": count })))
}

// ---------------------------------------------------------------------------
// Threads
// ---------------------------------------------------------------------------

async fn list_threads(
    Query(q): Query<AgentQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let threads = brain.storage.read().await.sqlite
        .list_threads(&scope_from(q.agent_id.as_deref())).await?;
    Ok(Json(Value::Array(threads.into_iter().map(|s| json!(s)).collect())))
}

async fn get_thread_memories(
    Path(thread_id): Path<String>,
    Query(q): Query<LimitQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope = scope_from(q.agent_id.as_deref());
    let mems  = brain.storage.read().await.sqlite
        .get_thread_memories(&thread_id, &scope).await?;
    let arr: Vec<Value> = mems.into_iter()
        .map(|n| serde_json::to_value(&n).unwrap_or_default())
        .collect();
    Ok(Json(Value::Array(arr)))
}

async fn prune_thread(
    Path(thread_id): Path<String>,
    State(brain): State<Brain>,
) -> AppResult {
    let count = brain.storage.read().await.sqlite.prune_thread(&thread_id, &VisibilityScope::global()).await?;
    Ok(Json(json!({ "deleted": count })))
}

// ---------------------------------------------------------------------------
// Dream
// ---------------------------------------------------------------------------

async fn dream_run(
    Query(q): Query<DreamRunQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let scope     = scope_from(q.agent_id.as_deref());
    let brain_arc = Arc::clone(&brain);
    let report    = brain.dream.run_cycle(scope, brain_arc, q.max_llm_calls).await?;
    audit(&brain, q.agent_id.as_deref(), "dream_run", None,
        Some(&format!("{} phases, success={}", report.phases.len(), report.success))).await;
    Ok(Json(serde_json::to_value(&report)?))
}

/// GET /dream/reports — the observatory timeline (Lucida U4): every recorded
/// cycle, newest first, full per-phase counters.
async fn dream_reports(
    Query(q): Query<LimitQuery>,
    State(brain): State<Brain>,
) -> AppResult {
    let rows = brain.storage.read().await.sqlite
        .list_dream_reports(q.limit.min(100)).await?;
    Ok(Json(json!({ "reports": rows })))
}

// dream_status is a global endpoint — the last dream report is not agent-scoped
// (C-RS-009: dropped the unused agent_id query param rather than pretending to
// honor it).
async fn dream_status(
    State(brain): State<Brain>,
) -> AppResult {
    let v = brain.storage.read().await.sqlite
        .get_last_dream_report().await?
        .unwrap_or(json!({ "status": "no_cycles_run" }));
    Ok(Json(v))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let config = cerebro::config::Config::from_env()?;
    let _ = DB_LABEL.set(config.db_path.display().to_string());
    let brain: Brain = Arc::new(CerebroCortex::new(config).await?);

    let app = build_router(brain);
    run_server(app).await
}

/// The full route table, testable without a listener. Path params use the
/// axum 0.8 brace syntax — under axum 0.7 these were silently LITERAL
/// segments and every parameterized route 404'd (found 2026-08-08; the
/// `parameterized_routes_resolve` test pins the semantics).
fn build_router(brain: Brain) -> Router {
    Router::new()
        // Lucida observatory (embedded UI + its data feeds)
        .route("/",                get(ui_index))
        .route("/style.css",       get(ui_css))
        .route("/app.js",          get(ui_js))
        .route("/graph/export",    get(graph_export))
        .route("/graph/layout",    get(graph_layout).post(graph_layout_recompute))
        .route("/recall/trace",    post(recall_trace))
        .route("/events",          get(events))
        .route("/audit/since/{id}", get(audit_since))
        .route("/meta",            get(meta))
        // Core
        .route("/health",          get(health))
        .route("/stats",           get(stats))
        .route("/q/{query}",       get(quick_search))
        .route("/remember",        post(remember))
        .route("/recall",          post(recall))
        // Memory CRUD
        .route("/memory/{id}",              get(get_memory).put(update_memory).delete(delete_memory))
        .route("/memory/{id}/versions",     get(get_memory_versions))
        // Associate
        .route("/associate",       post(associate))
        // Episodes
        .route("/episodes",        get(list_episodes).post(episode_start))
        .route("/episodes/{episode_id}/step",      post(episode_add_step))
        .route("/episodes/{episode_id}/end",       post(episode_end))
        .route("/episodes/{episode_id}",           get(get_episode))
        .route("/episodes/{episode_id}/memories",  get(get_episode_memories))
        // Sessions
        .route("/sessions/save",   post(session_save))
        .route("/sessions",        get(session_recall))
        // Agents
        .route("/agents",          get(list_agents).post(register_agent))
        // Diagnostics
        .route("/memory/health",   get(memory_health))
        .route("/emotions",        get(emotional_summary))
        // Graph
        .route("/graph/stats",                         get(graph_stats))
        .route("/graph/neighbors/{memory_id}",         get(graph_neighbors))
        .route("/graph/path/{source_id}/{target_id}",  get(graph_path))
        .route("/graph/common/{id_a}/{id_b}",          get(graph_common))
        // Tags
        .route("/tags",            get(list_tags))
        .route("/tags/rename",     post(rename_tag))
        .route("/tags/merge",      post(merge_tags))
        .route("/tags/{tag}",      delete(delete_tag))
        // Intentions
        .route("/intentions",                      get(list_intentions).post(store_intention))
        .route("/intentions/{memory_id}/resolve",  post(resolve_intention))
        // Schemas
        .route("/schemas",         get(list_schemas).post(create_schema))
        // Procedures
        .route("/procedures",      get(list_procedures).post(store_procedure))
        // Trash / lifecycle
        .route("/trash",           get(list_trash))
        .route("/trash/{id}/restore", post(restore_trash))
        .route("/trash/{id}",      delete(purge_trash))
        .route("/trash/purge-all", post(purge_all_trash))
        .route("/bulk/delete",     post(bulk_delete))
        // Threads
        .route("/threads",                         get(list_threads))
        .route("/threads/{thread_id}/memories",    get(get_thread_memories))
        .route("/threads/{thread_id}",             delete(prune_thread))
        // Dream
        .route("/dream/run",       post(dream_run))
        .route("/dream/status",    get(dream_status))
        .route("/dream/reports",   get(dream_reports))
        .with_state(brain)
}

async fn run_server(app: Router) -> Result<()> {
    // Token auth — CEREBRO_API_TOKEN, falling back to AGENTD_TOKEN (the shared
    // secret on an ApexOS node, so those deployments work unchanged).
    // Binds 127.0.0.1 by default; use CEREBRO_API_ADDR=0.0.0.0:8765 for LAN
    // exposure — which then REQUIRES a token (refused below without one).
    let api_token = Arc::new(
        std::env::var("CEREBRO_API_TOKEN")
            .or_else(|_| std::env::var("AGENTD_TOKEN"))
            .unwrap_or_default(),
    );
    if api_token.is_empty() {
        info!("cerebro-api: CEREBRO_API_TOKEN/AGENTD_TOKEN not set — auth disabled (127.0.0.1 only)");
    }
    let token_mw = api_token.clone();
    let app = app.layer(axum::middleware::from_fn(
        move |req: Request, next: Next| {
            let tok = token_mw.clone();
            async move {
                if tok.is_empty() { return next.run(req).await; }
                let from_header = req.headers()
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.strip_prefix("Bearer "))
                    .unwrap_or("");
                if ct_eq(from_header, tok.as_str()) { return next.run(req).await; }
                let from_query = req.uri().query().unwrap_or("")
                    .split('&')
                    .find_map(|p| p.strip_prefix("token="))
                    .unwrap_or("");
                if ct_eq(from_query, tok.as_str()) { return next.run(req).await; }
                (StatusCode::UNAUTHORIZED, "invalid or missing token").into_response()
            }
        }
    ));

    // CB-023: outermost layer so a panic anywhere in a handler (or the auth
    // middleware) becomes a 500 JSON body rather than a dropped connection.
    let app = app.layer(
        tower_http::catch_panic::CatchPanicLayer::custom(panic_response),
    );

    let addr = std::env::var("CEREBRO_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8765".into());
    // F036: an env typo must not silently re-open the unauthenticated-LAN hole.
    if api_token.is_empty() {
        if let Ok(sa) = addr.parse::<std::net::SocketAddr>() {
            if !sa.ip().is_loopback() {
                anyhow::bail!("refusing to bind {addr} without CEREBRO_API_TOKEN/AGENTD_TOKEN");
            }
        }
    }
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("cerebro-api listening on {addr}");
    if !api_token.is_empty() {
        info!("cerebro-api dashboard: http://{addr}/?token=<token>  (bearer token required)");
    }
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // U6 time-lapse: absent → live now; future → honored; past → clamps to
    // now (rewinding honestly would need access-history filtering); garbage
    // → honest error, not a silent live export.
    #[test]
    fn projection_resolves_forward_only() {
        let now = Utc::now();
        let (t, p) = resolve_projection(None, now).unwrap();
        assert!((t, p) == (now, false));

        let future = now + chrono::Duration::days(90);
        let (t, p) = resolve_projection(Some(&future.to_rfc3339()), now).unwrap();
        assert!(p);
        assert_eq!(t, future);

        let past = now - chrono::Duration::days(30);
        let (t, p) = resolve_projection(Some(&past.to_rfc3339()), now).unwrap();
        assert!(p);
        assert_eq!(t, now);

        assert!(resolve_projection(Some("not-a-time"), now).is_err());
    }

    // The projection's whole claim in one assertion: the same FSRS math at a
    // later clock reads dimmer. (ACT-R and link half-life decay likewise —
    // the export computes all three at `t`.)
    #[test]
    fn projection_dims_the_sky() {
        let r_now = cerebro::activation::retrievability(0.0, 1.0);
        let r_half_year = cerebro::activation::retrievability(180.0, 1.0);
        assert!(r_half_year < r_now);
    }

    // CB-012: the HTTP priority normalization must match the MCP canonical
    // (uppercase) so a `priority:<p>` tag written here is matched by an MCP
    // session_recall priority filter that uppercases its argument.
    #[test]
    fn normalize_priority_uppercases() {
        assert_eq!(normalize_priority("medium"), "MEDIUM");
        assert_eq!(normalize_priority("High"), "HIGH");
        assert_eq!(normalize_priority("LOW"), "LOW");
    }

    // The session_save default ("MEDIUM") and an HTTP-supplied lowercase value
    // ("medium") must produce the identical canonical tag.
    #[test]
    fn normalize_priority_default_matches_lowercase_input() {
        assert_eq!(normalize_priority("MEDIUM"), normalize_priority("medium"));
    }

    // U1: the projection must actually separate semantic clusters — two blobs
    // far apart in embedding space land far apart on the PC1 axis.
    #[test]
    fn pca_2d_separates_two_clusters() {
        let dim = 32;
        let mut embeddings = Vec::new();
        // Cluster A near +e0, cluster B near -e0, with small deterministic
        // per-point offsets on other axes so the data isn't degenerate.
        for i in 0..12 {
            let mut a = vec![0.0f32; dim];
            a[0] = 1.0;
            a[1 + (i % 8)] = 0.05 * (i as f32 + 1.0);
            embeddings.push((MemoryId(format!("a{i}")), a));
            let mut b = vec![0.0f32; dim];
            b[0] = -1.0;
            b[2 + (i % 8)] = -0.04 * (i as f32 + 1.0);
            embeddings.push((MemoryId(format!("b{i}")), b));
        }
        let coords = pca_2d(&embeddings);
        assert_eq!(coords.len(), 24);
        let xs_a: Vec<f32> = coords.iter().filter(|c| c.0.0.starts_with('a')).map(|c| c.1).collect();
        let xs_b: Vec<f32> = coords.iter().filter(|c| c.0.0.starts_with('b')).map(|c| c.1).collect();
        let mean_a = xs_a.iter().sum::<f32>() / xs_a.len() as f32;
        let mean_b = xs_b.iter().sum::<f32>() / xs_b.len() as f32;
        assert!((mean_a - mean_b).abs() > 1.0,
            "clusters must separate on PC1: a={mean_a} b={mean_b}");
        // Axes are normalized to [-1, 1].
        assert!(coords.iter().all(|c| c.1.abs() <= 1.001 && c.2.abs() <= 1.001));
    }

    // The axum-0.8 brace-syntax pin: under 0.7 every `{param}` route was a
    // silent LITERAL segment — the whole parameterized REST surface 404'd
    // (found 2026-08-08 via the Live lens's /audit/since route). This test
    // fails on any regression to colon-syntax semantics.
    #[tokio::test]
    async fn parameterized_routes_resolve() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let dir = tempfile::TempDir::new().unwrap();
        let config = cerebro::config::Config {
            db_path:       dir.path().join("test.db"),
            anthropic_key: None,
            embed_model:   "".into(),
        };
        let brain: Brain = Arc::new(CerebroCortex::new(config).await.unwrap());
        let app = build_router(brain);

        // A parameterized route must MATCH (200 with an empty rows array).
        let resp = app.clone()
            .oneshot(Request::get("/audit/since/0").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "/audit/since/{{id}} must route");

        // A missing memory is the HANDLER's error (500 ApiError), never the
        // router's 404 — a 404 here means the param syntax broke again.
        let resp = app
            .oneshot(Request::get("/memory/nonexistent").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_ne!(resp.status(), StatusCode::NOT_FOUND,
            "/memory/{{id}} must route to the handler");
    }

    #[test]
    fn pca_2d_degenerate_inputs_are_safe() {
        assert!(pca_2d(&[]).is_empty());
        let one = pca_2d(&[(MemoryId("solo".into()), vec![0.5; 8])]);
        assert_eq!(one, vec![(MemoryId("solo".into()), 0.0, 0.0)]);
        // Identical points: no panic, finite coords.
        let same = pca_2d(&[
            (MemoryId("x".into()), vec![0.3; 8]),
            (MemoryId("y".into()), vec![0.3; 8]),
        ]);
        assert!(same.iter().all(|c| c.1.is_finite() && c.2.is_finite()));
    }
}
