//! cerebro-ui — Lucida's native mirror (U5). Same cerebro-api JSON surface as
//! ui-web, rendered by Slint: dashboard panels + the Atlas and Thought lenses.
//! House rules: Slint owns the main thread (never #[tokio::main]); every UI
//! write goes through Weak::upgrade_in_event_loop; all flattening happens in
//! Rust — the .slint only places and paints.

mod logic;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use slint::{Color, ComponentHandle, ModelRc, VecModel, Weak};

use logic::PreppedTrace;

slint::include_modules!();

const HOP_MS: u64 = 950; // the web lens's ripple cadence (settings knob there)

// ---------------------------------------------------------------------------
// config + API client
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Api {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
    agent: Option<String>,
}

impl Api {
    fn from_env() -> Result<Self> {
        let base = std::env::var("CEREBRO_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8765".into())
            .trim_end_matches('/')
            .to_string();
        // Same fallback order as cerebro-api itself.
        let token = std::env::var("CEREBRO_API_TOKEN")
            .or_else(|_| std::env::var("AGENTD_TOKEN"))
            .ok()
            .filter(|t| !t.is_empty());
        let agent = std::env::var("LUCIDA_AGENT").ok().filter(|a| !a.is_empty());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self { http, base, token, agent })
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut r = self.http.request(method, format!("{}{path}", self.base));
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        r
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self.req(reqwest::Method::GET, path).send().await?;
        anyhow::ensure!(resp.status().is_success(), "GET {path} → {}", resp.status());
        Ok(resp.json().await?)
    }

    /// GET with the agent scope appended, like the web's ?agent= deep link.
    async fn get_scoped(&self, path: &str) -> Result<Value> {
        match &self.agent {
            Some(a) => {
                let sep = if path.contains('?') { '&' } else { '?' };
                self.get(&format!("{path}{sep}agent_id={a}")).await
            }
            None => self.get(path).await,
        }
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let resp = self.req(reqwest::Method::POST, path).json(&body).send().await?;
        anyhow::ensure!(resp.status().is_success(), "POST {path} → {}", resp.status());
        Ok(resp.json().await?)
    }
}

// ---------------------------------------------------------------------------
// field state
// ---------------------------------------------------------------------------

/// One kept star, fully resolved (wire row + placement + hue).
struct NodeInfo {
    id: String,
    mtype: String,
    layer: String,
    salience: f32,
    tags: Vec<String>,
    head: String,
    created: String,
    access: i64,
    glow: f32,
    retr: f32,
    valence: String,
    intensity: f32,
    agent: String,
    visibility: String,
    fx: f32,
    fy: f32,
    rim: bool,
    /// U6 rim-label honesty: a rim star that IS embedded is merely awaiting
    /// a layout recompute — saying "no embedding" there would be a lie.
    embedded: bool,
    hue: u32,
}

/// The honest rim suffix for hover/card text ("" off the rim).
fn rim_note(rim: bool, embedded: bool) -> &'static str {
    match (rim, embedded) {
        (false, _) => "",
        (true, true) => " · rim (awaiting layout)",
        (true, false) => " · rim (no embedding)",
    }
}

struct Field {
    nodes: Vec<NodeInfo>,
    index: HashMap<String, usize>,
    /// Kept edges as star-index pairs, for focus-mode highlighting.
    edges: Vec<(usize, usize)>,
    edge_cmds: String,
    meta_line: String,
}

/// Ripple animation state: the prepared trace + how far the wavefront is.
struct TraceState {
    prep: PreppedTrace,
    hop: u8,
}

#[derive(Default)]
struct Shared {
    field: Option<Field>,
    selected: Option<usize>,
    trace: Option<TraceState>,
    /// Bumped on refresh / new trace — stale animation tasks see it and stop.
    gen: u64,
}

type SharedRef = Arc<Mutex<Shared>>;

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let api = Api::from_env()?;
    let ui = MainWindow::new().context("create window")?;
    let shared: SharedRef = Arc::new(Mutex::new(Shared::default()));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    ui.set_api_line(
        format!(
            "{}{}",
            api.base,
            api.agent.as_deref().map(|a| format!(" · scope {a}")).unwrap_or_default()
        )
        .into(),
    );

    // boot fetch (+ optional snapshot-mode boot query, web ?q= parity)
    let boot_query = std::env::var("LUCIDA_SNAPSHOT_QUERY").ok().filter(|q| !q.is_empty());
    if boot_query.is_some() {
        ui.set_lens(1);
    }
    rt.spawn({
        let weak = ui.as_weak();
        let api = api.clone();
        let shared = shared.clone();
        async move {
            refresh(&weak, &api, &shared).await;
            if let Some(q) = boot_query {
                run_trace(&weak, &api, &shared, q).await;
            }
        }
    });

    ui.on_refresh({
        let weak = ui.as_weak();
        let api = api.clone();
        let shared = shared.clone();
        let handle = rt.handle().clone();
        move || {
            let (weak, api, shared) = (weak.clone(), api.clone(), shared.clone());
            handle.spawn(async move { refresh(&weak, &api, &shared).await });
        }
    });

    ui.on_run_trace({
        let weak = ui.as_weak();
        let api = api.clone();
        let shared = shared.clone();
        let handle = rt.handle().clone();
        move |query| {
            let q = query.trim().to_string();
            if q.is_empty() {
                return;
            }
            let (weak, api, shared) = (weak.clone(), api.clone(), shared.clone());
            handle.spawn(async move { run_trace(&weak, &api, &shared, q).await });
        }
    });

    // Hover: ring + tooltip only — nothing opens (the U0 interaction rule).
    ui.on_hover_probe({
        let weak = ui.as_weak();
        let shared = shared.clone();
        move |fx, fy, tol| {
            let Some(ui) = weak.upgrade() else { return };
            let s = shared.lock().expect("shared");
            match s.field.as_ref().and_then(|f| hit_test(f, fx, fy, tol)) {
                Some(i) => {
                    let n = &s.field.as_ref().expect("field").nodes[i];
                    ui.set_hover_fx(n.fx);
                    ui.set_hover_fy(n.fy);
                    ui.set_hover_hue(color(n.hue, 0xff));
                    ui.set_hover_text(
                        format!(
                            "{} · {}{}",
                            trim_chars(&n.head, 64),
                            n.mtype,
                            rim_note(n.rim, n.embedded)
                        )
                        .into(),
                    );
                    ui.set_hover_visible(true);
                }
                None => ui.set_hover_visible(false),
            }
        }
    });

    // Click: select (pinned card) or dismiss on empty sky.
    ui.on_click_probe({
        let weak = ui.as_weak();
        let shared = shared.clone();
        let api = api.clone();
        let handle = rt.handle().clone();
        move |fx, fy, tol| {
            let hit = {
                let s = shared.lock().expect("shared");
                s.field.as_ref().and_then(|f| hit_test(f, fx, fy, tol))
            };
            match hit {
                Some(i) => select(&weak, &api, &shared, &handle, i),
                None => dismiss(&weak, &shared),
            }
        }
    });

    ui.on_result_clicked({
        let weak = ui.as_weak();
        let shared = shared.clone();
        let api = api.clone();
        let handle = rt.handle().clone();
        move |id| {
            let idx = {
                let s = shared.lock().expect("shared");
                s.field.as_ref().and_then(|f| f.index.get(id.as_str()).copied())
            };
            match idx {
                Some(i) => {
                    if let Some(ui) = weak.upgrade() {
                        // fly to the star
                        let s = shared.lock().expect("shared");
                        let n = &s.field.as_ref().expect("field").nodes[i];
                        ui.set_view_cx(n.fx);
                        ui.set_view_cy(n.fy);
                        if ui.get_zoom() < 3.0 {
                            ui.set_zoom(3.0);
                        }
                    }
                    select(&weak, &api, &shared, &handle, i);
                }
                None => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_status_line(
                            "that memory is outside the capped field — raise LUCIDA_STARS".into(),
                        );
                    }
                }
            }
        }
    });

    ui.on_dismiss({
        let weak = ui.as_weak();
        let shared = shared.clone();
        move || dismiss(&weak, &shared)
    });

    // LUCIDA_SNAPSHOT: write a PNG after a delay and exit — the headless
    // self-verification hook (the ApexOS take_snapshot technique).
    let _snap_timer = slint::Timer::default();
    if let Ok(path) = std::env::var("LUCIDA_SNAPSHOT") {
        let ms: u64 = std::env::var("LUCIDA_SNAPSHOT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2500);
        let weak = ui.as_weak();
        _snap_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(ms),
            move || {
                if let Some(ui) = weak.upgrade() {
                    match ui.window().take_snapshot() {
                        Ok(buf) => {
                            let (w, h) = (buf.width(), buf.height());
                            if let Err(e) = image::save_buffer(
                                &path,
                                buf.as_bytes(),
                                w,
                                h,
                                image::ExtendedColorType::Rgba8,
                            ) {
                                eprintln!("snapshot save failed: {e}");
                            } else {
                                eprintln!("snapshot written: {path}");
                            }
                        }
                        Err(e) => eprintln!("take_snapshot failed: {e}"),
                    }
                }
                let _ = slint::quit_event_loop();
            },
        );
    }

    ui.run()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// flows
// ---------------------------------------------------------------------------

async fn refresh(weak: &Weak<MainWindow>, api: &Api, shared: &SharedRef) {
    set_status(weak, "fetching…", None);
    let fetched = tokio::try_join!(
        api.get("/meta"),
        api.get("/stats"),
        api.get_scoped("/graph/export?cap=8000"),
        api.get("/graph/layout"),
    );
    let (meta, stats, export, layout) = match fetched {
        Ok(v) => v,
        Err(e) => {
            set_status(weak, &format!("cerebro-api unreachable: {e}"), Some(false));
            return;
        }
    };
    let field = build_field(&export, &layout, api.agent.as_deref());

    // dashboard lines
    let db_line = format!(
        "{} · v{}",
        meta["db_path"].as_str().unwrap_or("unknown"),
        meta["version"].as_str().unwrap_or("?")
    );
    let totals_line = format!(
        "{} memories · {} links · {} in trash",
        stats["total_memories"].as_i64().unwrap_or(0),
        stats["total_links"].as_i64().unwrap_or(0),
        stats["deleted_memories"].as_i64().unwrap_or(0),
    );
    let mut by_type: Vec<(String, i64)> = stats["by_type"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect())
        .unwrap_or_default();
    by_type.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    let type_stats: Vec<TypeStat> = by_type
        .iter()
        .map(|(t, c)| TypeStat {
            name: t.clone().into(),
            count: c.to_string().into(),
            hue: color(logic::type_color(t), 0xff),
        })
        .collect();

    {
        let mut s = shared.lock().expect("shared");
        s.gen += 1;
        s.field = Some(field);
        s.selected = None;
        s.trace = None;
    }

    let stars = star_items(shared);
    let (edge_cmds, meta_line) = {
        let s = shared.lock().expect("shared");
        let f = s.field.as_ref().expect("field");
        (f.edge_cmds.clone(), f.meta_line.clone())
    };
    let stamp = chrono::Local::now().format("%H:%M:%S");
    let status = format!("refreshed {stamp}");
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_stars(ModelRc::from(std::rc::Rc::new(VecModel::from(stars))));
        ui.set_edge_cmds(edge_cmds.into());
        ui.set_focus_cmds("".into());
        ui.set_trace_done_cmds("".into());
        ui.set_trace_active_cmds("".into());
        ui.set_meta_line(meta_line.into());
        ui.set_db_line(db_line.into());
        ui.set_totals_line(totals_line.into());
        ui.set_type_stats(ModelRc::from(std::rc::Rc::new(VecModel::from(type_stats))));
        ui.set_results(ModelRc::from(std::rc::Rc::new(VecModel::from(Vec::<ResultItem>::new()))));
        ui.set_trace_meta("".into());
        ui.set_sel_visible(false);
        ui.set_hover_visible(false);
        ui.set_connected(true);
        ui.set_status_line(status.into());
    });
}

/// The wire → field flattening: placement (web-parity math), brightest-N
/// ranking, strongest-N edges, the honest meta line.
fn build_field(export: &Value, layout: &Value, agent: Option<&str>) -> Field {
    let star_cap = env_cap("LUCIDA_STARS", 400, 50, 2000);
    let edge_cap = env_cap("LUCIDA_EDGES", 900, 100, 5000);
    let coords = layout["coords"].as_object();
    let empty = vec![];
    let wire_nodes = export["nodes"].as_array().unwrap_or(&empty);

    let channels: Vec<(f32, f32)> = wire_nodes
        .iter()
        .map(|n| (f32_of(&n["activation"]), f32_of(&n["salience"])))
        .collect();
    let kept = logic::pick_stars(&channels, star_cap);

    let mut nodes = Vec::with_capacity(kept.len());
    let mut index = HashMap::with_capacity(kept.len());
    let mut rim_count = 0usize;
    for &wi in &kept {
        let n = &wire_nodes[wi];
        let id = n["id"].as_str().unwrap_or_default().to_string();
        let coord = coords
            .and_then(|c| c.get(&id))
            .and_then(|v| v.as_array())
            .and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
        let (fx, fy, rim) = logic::field_pos(coord, &id);
        rim_count += rim as usize;
        let mtype = n["memory_type"].as_str().unwrap_or("?").to_string();
        index.insert(id.clone(), nodes.len());
        nodes.push(NodeInfo {
            hue: logic::type_color(&mtype),
            mtype,
            layer: n["layer"].as_str().unwrap_or("?").to_string(),
            salience: f32_of(&n["salience"]),
            tags: n["tags"]
                .as_array()
                .map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            head: n["content_head"].as_str().unwrap_or_default().to_string(),
            created: n["created_at"].as_str().unwrap_or_default().to_string(),
            access: n["access_count"].as_i64().unwrap_or(0),
            glow: f32_of(&n["activation"]),
            retr: f32_of(&n["retrievability"]),
            valence: n["valence"].as_str().unwrap_or("neutral").to_string(),
            intensity: f32_of(&n["intensity"]),
            agent: n["agent_id"].as_str().unwrap_or("—").to_string(),
            visibility: n["visibility"].as_str().unwrap_or("?").to_string(),
            fx,
            fy,
            rim,
            embedded: n["embedded"].as_bool().unwrap_or(false),
            id,
        });
    }

    let wire_edges = export["edges"].as_array().unwrap_or(&empty);
    let candidate: Vec<(usize, usize, f32)> = wire_edges
        .iter()
        .filter_map(|e| {
            let a = *index.get(e["source"].as_str()?)?;
            let b = *index.get(e["target"].as_str()?)?;
            Some((a, b, f32_of(&e["effective_weight"])))
        })
        .collect();
    let eff: Vec<f32> = candidate.iter().map(|c| c.2).collect();
    let edges: Vec<(usize, usize)> = logic::pick_edges(&eff, edge_cap)
        .into_iter()
        .map(|i| (candidate[i].0, candidate[i].1))
        .collect();
    let edge_cmds = logic::path_commands(edges.iter().map(|&(a, b)| {
        let (ax, ay) = (nodes[a].fx, nodes[a].fy);
        let (bx, by) = (nodes[b].fx, nodes[b].fy);
        (ax, ay, bx, by)
    }));

    let meta_line = format!(
        "{} of {} stars · {} on rim · {} of {} links{}{}",
        nodes.len(),
        wire_nodes.len(),
        rim_count,
        edges.len(),
        wire_edges.len(),
        agent.map(|a| format!(" · scope {a}")).unwrap_or_default(),
        if export["truncated"].as_bool().unwrap_or(false) { " · export truncated" } else { "" },
    );

    Field { nodes, index, edges, edge_cmds, meta_line }
}

async fn run_trace(weak: &Weak<MainWindow>, api: &Api, shared: &SharedRef, query: String) {
    set_busy(weak, true);
    set_status(weak, "recalling (reinforces — watching a thought is thinking it)…", None);
    let mut body = json!({ "query": query, "top_k": 12 });
    if let Some(a) = &api.agent {
        body["agent_id"] = json!(a);
    }
    let resp = match api.post("/recall/trace", body).await {
        Ok(v) => v,
        Err(e) => {
            set_busy(weak, false);
            set_status(weak, &format!("recall failed: {e}"), None);
            return;
        }
    };
    set_busy(weak, false);

    // wire → prepared trace, against the capped field
    let (my_gen, prep, results) = {
        let mut s = shared.lock().expect("shared");
        s.gen += 1;
        let my_gen = s.gen;
        let Some(f) = s.field.as_ref() else { return };
        let seeds = pairs_of(&resp["trace"]["seeds"]);
        let activated = pairs_of(&resp["trace"]["activated"]);
        let events: Vec<(u8, String, String, f32)> = resp["trace"]["events"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| {
                        (
                            e["hop"].as_u64().unwrap_or(0) as u8,
                            e["source"].as_str().unwrap_or_default().to_string(),
                            e["target"].as_str().unwrap_or_default().to_string(),
                            f32_of(&e["amount"]),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let pos: Vec<(f32, f32)> = f.nodes.iter().map(|n| (n.fx, n.fy)).collect();
        let prep = logic::prep_trace(&seeds, &events, &activated, &f.index, &pos);

        let results: Vec<(String, String, String, u32)> = resp["results"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            r["id"].as_str().unwrap_or_default().to_string(),
                            format!("{}.", i + 1),
                            format!(
                                "{:.2}|{}",
                                f32_of(&r["score"]),
                                trim_chars(r["content_head"].as_str().unwrap_or_default(), 90)
                            ),
                            logic::type_color(r["memory_type"].as_str().unwrap_or("?")),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        s.selected = None;
        s.trace = Some(TraceState { prep: prep.clone(), hop: 0 });
        (my_gen, prep, results)
    };

    // hop 0: seeds ignite
    push_trace_frame(weak, shared, &results, false);
    let max_hop = prep.max_hop;
    for hop in 1..=max_hop {
        tokio::time::sleep(Duration::from_millis(HOP_MS)).await;
        {
            let mut s = shared.lock().expect("shared");
            if s.gen != my_gen {
                return; // superseded by a newer trace / refresh
            }
            if let Some(t) = s.trace.as_mut() {
                t.hop = hop;
            }
        }
        push_trace_frame(weak, shared, &results, hop == max_hop);
    }
    if max_hop == 0 {
        // no walks survived — crystallize immediately, honestly
        push_trace_frame(weak, shared, &results, true);
    }
    set_status(weak, "trace complete — click a result to fly to it", None);
}

/// Ship one animation frame: star boosts up to the current hop, done/active
/// edge layers, ranked results (lit when the constellation crystallizes).
fn push_trace_frame(
    weak: &Weak<MainWindow>,
    shared: &SharedRef,
    results: &[(String, String, String, u32)],
    done: bool,
) {
    let stars = star_items(shared);
    let (done_cmds, active_cmds, meta) = {
        let s = shared.lock().expect("shared");
        let Some(t) = s.trace.as_ref() else { return };
        let hop = t.hop as usize;
        // done layer = fully fired hops; the current hop rides the bright
        // active layer until the animation crystallizes.
        let upto = if done { hop } else { hop.saturating_sub(1) };
        let done_cmds = t.prep.hop_cmds[..upto.min(t.prep.hop_cmds.len())].concat();
        let active_cmds = if !done && hop >= 1 {
            t.prep.hop_cmds.get(hop - 1).cloned().unwrap_or_default()
        } else {
            String::new()
        };
        let meta = if done {
            format!(
                "{} walks ({} shown) · {} activated · reinforced",
                t.prep.walks_total, t.prep.walks_shown, t.prep.activated_total
            )
        } else {
            format!("hop {} / {} · {} walks", t.hop, t.prep.max_hop, t.prep.walks_total)
        };
        (done_cmds, active_cmds, meta)
    };
    let rows: Vec<ResultItem> = results
        .iter()
        .map(|(id, rank, packed, hue)| {
            let (score, head) = packed.split_once('|').unwrap_or(("", packed));
            ResultItem {
                id: id.clone().into(),
                rank: rank.clone().into(),
                score: score.to_string().into(),
                head: head.to_string().into(),
                hue: color(*hue, 0xff),
                lit: done,
            }
        })
        .collect();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_stars(ModelRc::from(std::rc::Rc::new(VecModel::from(stars))));
        ui.set_trace_done_cmds(done_cmds.into());
        ui.set_trace_active_cmds(active_cmds.into());
        ui.set_trace_meta(meta.into());
        ui.set_results(ModelRc::from(std::rc::Rc::new(VecModel::from(rows))));
    });
}

fn select(
    weak: &Weak<MainWindow>,
    api: &Api,
    shared: &SharedRef,
    handle: &tokio::runtime::Handle,
    i: usize,
) {
    let (focus_cmds, id, card) = {
        let mut s = shared.lock().expect("shared");
        s.selected = Some(i);
        let f = s.field.as_ref().expect("field");
        let n = &f.nodes[i];
        let focus = logic::path_commands(f.edges.iter().filter(|&&(a, b)| a == i || b == i).map(
            |&(a, b)| (f.nodes[a].fx, f.nodes[a].fy, f.nodes[b].fx, f.nodes[b].fy),
        ));
        (focus, n.id.clone(), card_props(n))
    };
    let stars = star_items(shared);
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_stars(ModelRc::from(std::rc::Rc::new(VecModel::from(stars))));
        ui.set_focus_cmds(focus_cmds.into());
        apply_card(&ui, card);
    });

    // full body fetch — the export row is an index, not the text (wire_summary
    // doctrine); upgrade the card when the whole memory lands.
    let (weak, api, shared) = (weak.clone(), api.clone(), shared.clone());
    handle.spawn(async move {
        let Ok(v) = api.get(&format!("/memory/{id}")).await else { return };
        let still = {
            let s = shared.lock().expect("shared");
            s.selected
                .and_then(|i| s.field.as_ref().map(|f| f.nodes[i].id == id))
                .unwrap_or(false)
        };
        if still {
            if let Some(content) = v["content"].as_str() {
                let content = content.to_string();
                let _ = weak.upgrade_in_event_loop(move |ui| ui.set_sel_content(content.into()));
            }
        }
    });
}

fn dismiss(weak: &Weak<MainWindow>, shared: &SharedRef) {
    {
        let mut s = shared.lock().expect("shared");
        if s.selected.is_none() {
            return;
        }
        s.selected = None;
    }
    let stars = star_items(shared);
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_stars(ModelRc::from(std::rc::Rc::new(VecModel::from(stars))));
        ui.set_focus_cmds("".into());
        ui.set_sel_visible(false);
    });
}

// ---------------------------------------------------------------------------
// flattening helpers
// ---------------------------------------------------------------------------

/// Build the star model from field + selection + trace state. Trace dimming
/// (non-participants recede) takes precedence over focus dimming.
fn star_items(shared: &SharedRef) -> Vec<StarItem> {
    let s = shared.lock().expect("shared");
    let Some(f) = s.field.as_ref() else { return vec![] };
    let neighbors: Option<std::collections::HashSet<usize>> = s.selected.map(|sel| {
        f.edges
            .iter()
            .filter(|&&(a, b)| a == sel || b == sel)
            .flat_map(|&(a, b)| [a, b])
            .chain([sel])
            .collect()
    });
    f.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (boost, seed, trace_dim) = match s.trace.as_ref() {
                Some(t) => {
                    let lit = t.prep.ignite.get(&i).map(|&h| h <= t.hop).unwrap_or(false);
                    (
                        if lit { t.prep.boost.get(&i).copied().unwrap_or(0.55) } else { 0.0 },
                        lit && t.prep.ignite.get(&i) == Some(&0),
                        !lit,
                    )
                }
                None => (0.0, false, false),
            };
            let dimmed = if s.trace.is_some() {
                trace_dim
            } else {
                neighbors.as_ref().map(|nb| !nb.contains(&i)).unwrap_or(false)
            };
            let halo_alpha = ((0.22 + 0.62 * n.glow + 0.4 * boost).min(1.0) * 255.0) as u8;
            StarItem {
                fx: n.fx,
                fy: n.fy,
                size: 3.0 + n.salience * 4.0,
                core: color(n.hue, 0xff),
                halo: color(n.hue, halo_alpha),
                boost,
                seed,
                dimmed,
            }
        })
        .collect()
}

/// Card fields, flattened (full content arrives async from /memory/:id).
struct CardProps {
    fx: f32,
    fy: f32,
    hue: u32,
    mtype: String,
    id: String,
    content: String,
    tags: String,
    meta1: String,
    meta2: String,
    meta3: String,
}

fn card_props(n: &NodeInfo) -> CardProps {
    CardProps {
        fx: n.fx,
        fy: n.fy,
        hue: n.hue,
        mtype: n.mtype.clone(),
        id: n.id.clone(),
        content: n.head.clone(),
        tags: n.tags.join(" · "),
        meta1: format!(
            "salience {:.2} · activation {:.2} · retrievability {:.2}",
            n.salience, n.glow, n.retr
        ),
        meta2: format!("{} · intensity {:.2} · {}", n.valence, n.intensity, n.layer),
        meta3: format!(
            "created {} · access {} · {} · {}{}",
            n.created.get(..10).unwrap_or(&n.created),
            n.access,
            n.agent,
            n.visibility,
            rim_note(n.rim, n.embedded),
        ),
    }
}

fn apply_card(ui: &MainWindow, c: CardProps) {
    ui.set_sel_fx(c.fx);
    ui.set_sel_fy(c.fy);
    ui.set_sel_hue(color(c.hue, 0xff));
    ui.set_sel_type(c.mtype.into());
    ui.set_sel_id(c.id.into());
    ui.set_sel_content(c.content.into());
    ui.set_sel_tags(c.tags.into());
    ui.set_sel_meta1(c.meta1.into());
    ui.set_sel_meta2(c.meta2.into());
    ui.set_sel_meta3(c.meta3.into());
    ui.set_sel_visible(true);
    ui.set_hover_visible(false);
}

fn hit_test(f: &Field, fx: f32, fy: f32, tol: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, n) in f.nodes.iter().enumerate() {
        let d2 = (n.fx - fx).powi(2) + (n.fy - fy).powi(2);
        if d2 <= tol * tol && best.map(|(_, bd)| d2 < bd).unwrap_or(true) {
            best = Some((i, d2));
        }
    }
    best.map(|(i, _)| i)
}

fn set_status(weak: &Weak<MainWindow>, msg: &str, connected: Option<bool>) {
    let msg = msg.to_string();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_status_line(msg.into());
        if let Some(c) = connected {
            ui.set_connected(c);
        }
    });
}

fn set_busy(weak: &Weak<MainWindow>, busy: bool) {
    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_busy(busy));
}

fn color(rgb: u32, alpha: u8) -> Color {
    Color::from_argb_u8(alpha, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn f32_of(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

/// [[id, value], …] wire pairs (RecallTrace seeds/activated).
fn pairs_of(v: &Value) -> Vec<(String, f32)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let arr = p.as_array()?;
                    Some((arr.first()?.as_str()?.to_string(), arr.get(1)?.as_f64()? as f32))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn trim_chars(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out.replace('\n', " ")
}

fn env_cap(var: &str, default: usize, lo: usize, hi: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
        .clamp(lo, hi)
}
