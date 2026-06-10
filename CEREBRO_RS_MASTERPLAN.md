# CerebroCortex-RS — Port Masterplan

> A pure-Rust port of CerebroCortex: brain-analogous AI memory system with
> associative networks, ACT-R/FSRS activation, spreading activation, and a
> 6-phase dream engine. MCP-over-stdio compatible, Pi 5 native, no Python runtime.

**Status:** scaffold complete, `cargo check` clean. Build-order step 1 is the current target.
**Source:** `buckster123/CerebroCortex` (Python 3.11+, 467 tests, ~8k LOC)
**Target:** `buckster123/CerebroCortex-RS` (Rust, tokio, MCP-over-stdio, ApexOS-native)
**Relationship to ApexOS:** drop-in replacement for the `cerebro` plugin entry in
`plugins.toml`. Same MCP contract, no changes to agentd required.

---

## 1. Why port, and what the port is NOT

CerebroCortex already works. The port is not about correctness — it's about:

- **Zero Python runtime on the Pi.** Single binary, `scp` and done, same as agentd.
- **Memory ceiling.** On an 8GB Pi running agentd + multiple MCP plugins + the dream
  engine, every MB matters. Rust's footprint is an order of magnitude below CPython +
  venv + chromadb + igraph + sentence-transformers.
- **ApexOS-RS readiness.** The long-term vision is a fully Rust-native stack. A Rust
  Cerebro is the prerequisite for that.
- **Performance on Pi-class hardware.** The activation math (ACT-R, FSRS, spreading
  activation) is pure numerical computation — Rust will run it significantly faster,
  which matters when the dream engine churns through the full graph.

What the port is NOT: a redesign. The cognitive architecture, the 9 engine model, the
6-phase dream engine, the multi-agent visibility scoping, the MCP tool surface — all
preserved exactly. The Python version stays as the reference and daily driver until
the Rust port passes the full test suite.

---

## 2. Dependency mapping (Python → Rust)

| Python | Rust equivalent | Notes |
|--------|----------------|-------|
| `python-igraph` | `petgraph` | Pure Rust graph library. DFS/BFS/shortest path built in. Spreading activation maps directly. |
| `chromadb` | `sqlite-vec` | SQLite extension for ANN vector search. Zero extra process. Same DB file as the main store. |
| `sentence-transformers` (SBERT) | `fastembed` | Rust crate wrapping ONNX Runtime. Same 384-dim embeddings. ~50MB model, no GPU required. |
| FTS5 fallback | SQLite FTS5 (built-in) | Already used in Python as fallback. Lean on it more in Rust. |
| `fastapi` + `uvicorn` | `axum` | Same crate family as agentd. Reuse the pattern. |
| `mcp` Python SDK | Hand-rolled JSON-RPC over stdio | MCP-over-stdio is simple enough. Avoids SDK churn dependency. |
| `anthropic` (dream LLM) | `reqwest` + JSON | Direct HTTP to Anthropic API. Same as agentd's inference client. |
| `pydantic` | `serde` + `serde_json` | Derive macros replace field decorators. Direct mapping. |
| `click` (CLI) | `clap` | Near-identical ergonomics. |
| `watchdog` | `notify` | Cross-platform inotify wrapper, mature crate. |
| `pillow` / `pytesseract` / `pymupdf` | `image` + `lopdf` | Vision extras — defer to Phase 3. |
| `pytest` | `cargo test` | Built-in, parallel by default. |

**The key insight:** `sqlite-vec` + FTS5 means the entire storage layer (vector search,
full-text search, graph persistence) lives in ONE SQLite file with zero external
services. This is actually cleaner than Python's triple-backend (SQLite + igraph
in-memory + chromadb process).

---

## 3. Workspace layout

```
CerebroCortex-RS/
├── Cargo.toml                    # workspace
├── crates/
│   ├── cerebro/                  # the library — all cognitive logic
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs          # MemoryType, LinkType, Layer, Visibility, IDs
│   │       ├── models/           # MemoryNode, AssociativeLink, Episode, Agent
│   │       ├── storage/
│   │       │   ├── sqlite.rs     # SQLite schema + CRUD (source of truth)
│   │       │   ├── graph.rs      # petgraph in-memory (rebuilt on init)
│   │       │   └── vector.rs     # sqlite-vec ANN + FTS5 fallback
│   │       ├── activation/
│   │       │   ├── actr.rs       # B(t) = ln(Σ t_k^{-d})
│   │       │   ├── fsrs.rs       # R(t,S) = (1 + t/9S)^{-1}
│   │       │   └── spreading.rs  # Collins & Loftus, 2-hop, 50-node cap
│   │       ├── engines/
│   │       │   ├── thalamus.rs   # GatingEngine
│   │       │   ├── amygdala.rs   # AffectEngine
│   │       │   ├── temporal.rs   # SemanticEngine
│   │       │   ├── hippocampus.rs# EpisodicEngine
│   │       │   ├── association.rs# LinkEngine (Hebbian)
│   │       │   ├── cerebellum.rs # ProceduralEngine
│   │       │   ├── prefrontal.rs # ExecutiveEngine
│   │       │   ├── neocortex.rs  # SchemaEngine
│   │       │   └── dream.rs      # DreamEngine (6-phase)
│   │       ├── cortex.rs         # CerebroCortex coordinator
│   │       └── config.rs         # All tuneable parameters
│   ├── cerebro-mcp/              # MCP-over-stdio binary (63 tools)
│   ├── cerebro-api/              # axum REST API + dashboard (optional)
│   └── cerebro-cli/              # clap CLI
├── tests/                        # integration tests (mirrors Python suite)
├── benches/                      # criterion benchmarks
└── deploy/
    └── cerebro.service           # systemd unit (replaces Python venv)
```

---

## 4. The load-bearing types

Direct port of `types.py` and `models/`.

```rust
// crates/cerebro/src/types.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Episodic, Semantic, Procedural, Affective, Prospective, Schematic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Temporal, Causal, Semantic, Affective, Contextual,
    Contradicts, Supports, DerivedFrom, PartOf,
}

impl LinkType {
    /// Spreading activation conductance weight per link type.
    /// Mirrors Python LINK_TYPE_WEIGHTS exactly.
    pub fn activation_weight(self) -> f32 {
        match self {
            Self::Causal      => 0.9,
            Self::Semantic    => 0.8,
            Self::Supports    => 0.8,
            Self::PartOf      => 0.8,
            Self::Contextual  => 0.7,
            Self::DerivedFrom => 0.7,
            Self::Temporal    => 0.6,
            Self::Affective   => 0.5,
            Self::Contradicts => 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayer { Sensory, Working, LongTerm, Cortex }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility { Shared, Private, Thread }
```

```rust
// crates/cerebro/src/models/memory.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    pub id:           MemoryId,
    pub content:      String,
    pub memory_type:  MemoryType,
    pub layer:        MemoryLayer,
    pub salience:     f32,
    pub tags:         Vec<String>,
    pub agent_id:     Option<AgentId>,
    pub visibility:   Visibility,
    pub thread_id:    Option<String>,
    pub created_at:   DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
    pub access_count: u32,
    pub access_times: Vec<DateTime<Utc>>,  // ACT-R timestamps, capped at 50
    pub strength:     StrengthState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrengthState {
    pub stability:   f32,    // FSRS S parameter
    pub difficulty:  f32,    // FSRS D parameter  
    pub last_review: Option<DateTime<Utc>>,
}
```

---

## 5. The activation system (pure math — ports clean)

All three subsystems are pure numerical computation. No external dependencies.
This is also where the Rust port will show its most obvious performance gain.

```rust
// crates/cerebro/src/activation/actr.rs
// B(t) = ln( Σ t_k^{-d} )

pub fn base_level_activation(
    access_times: &[DateTime<Utc>],
    now: DateTime<Utc>,
    decay: f32,    // default 0.5
    noise: f32,    // default 0.4
) -> f32 {
    if access_times.is_empty() { return f32::NEG_INFINITY; }
    let sum: f32 = access_times.iter()
        .map(|t| (now - *t).num_seconds().max(1) as f32)
        .map(|secs| secs.powf(-decay))
        .sum();
    sum.ln() + thread_rng().gen::<f32>() * noise * 2.0 - noise
}
```

```rust
// crates/cerebro/src/activation/fsrs.rs
// R(t, S) = (1 + t / (9 × S))^{-1}

pub fn retrievability(stability: f32, last_review: DateTime<Utc>, now: DateTime<Utc>) -> f32 {
    let t_days = (now - last_review).num_seconds() as f32 / 86400.0;
    (1.0 + t_days / (9.0 * stability)).powi(-1)
}

pub fn update_stability(stability: f32, difficulty: f32, retrieved: bool) -> f32 {
    let s = if retrieved {
        stability * (1.0 + 0.1 * (11.0 - difficulty))
    } else {
        stability * 0.5
    };
    s.clamp(FSRS_MIN_STABILITY, FSRS_MAX_STABILITY)
}
```

```rust
// crates/cerebro/src/activation/spreading.rs
// Collins & Loftus: max 2 hops, 0.6 decay per hop, 50-node cap.

pub fn spread(
    graph: &Graph<MemoryId, AssociativeLink>,
    seeds: &[NodeIndex],
    scope: &VisibilityScope,
    max_nodes: usize,   // 50
    hop_decay: f32,     // 0.6
) -> HashMap<NodeIndex, f32> {
    let mut activated: HashMap<NodeIndex, f32> = HashMap::new();
    let mut frontier: Vec<(NodeIndex, f32, u8)> =
        seeds.iter().map(|&n| (n, 1.0, 0)).collect();

    while let Some((node, activation, depth)) = frontier.pop() {
        if activated.len() >= max_nodes || activation < 0.05 { continue; }
        activated.entry(node)
            .and_modify(|a| *a = a.max(activation))
            .or_insert(activation);
        if depth >= 2 { continue; }
        for edge in graph.edges(node) {
            if !scope.can_access_idx(edge.target()) { continue; }
            let w = edge.weight().link_type.activation_weight() * edge.weight().weight;
            frontier.push((edge.target(), activation * hop_decay * w, depth + 1));
        }
    }
    activated
}
```

**Combined recall score** (mirrors Python's weighted blend exactly):
```rust
pub fn recall_score(vector_sim: f32, actr: f32, fsrs: f32, salience: f32) -> f32 {
    0.35 * vector_sim + 0.30 * actr + 0.20 * fsrs + 0.15 * salience
}
```

---

## 6. Multi-agent visibility scoping

Direct port. Every SQL query and graph traversal receives a `VisibilityScope`.

```rust
pub struct VisibilityScope { pub agent_id: Option<AgentId> }

impl VisibilityScope {
    pub fn can_access(&self, node: &MemoryNode) -> bool {
        match node.visibility {
            Visibility::Shared  => true,
            Visibility::Private => self.agent_id == node.agent_id,
            Visibility::Thread  => true, // thread_id checked separately
        }
    }

    /// SQL fragment — mirrors Python's _scope_sql()
    pub fn sql_filter(&self) -> (&'static str, Vec<String>) {
        match &self.agent_id {
            None      => ("1=1", vec![]),
            Some(id)  => (
                "(visibility='shared' OR (visibility='private' AND agent_id=?))",
                vec![id.0.clone()]
            ),
        }
    }
}
```

---

## 7. Dream engine (6-phase consolidation)

Three phases are algorithmic (1, 4, 5). Three call the LLM (2, 3, 6).
LLM calls use the same `reqwest`-based Anthropic client as agentd.

```rust
pub struct DreamEngine {
    api:    Arc<AnthropicClient>,
    config: DreamConfig,
}

impl DreamEngine {
    pub async fn run_cycle(
        &self,
        scope: AgentScope,
        store: &StorageCoordinator,
    ) -> Result<DreamReport> {
        // Phase 1: SWS replay — algorithmic, strengthen temporal links in episodes
        let p1 = self.sws_replay(&scope, store).await?;
        // Phase 2: Pattern extraction — cluster similar memories, LLM summarizes
        let p2 = self.pattern_extraction(&scope, store).await?;
        // Phase 3: Schema formation — LLM abstracts episodes → schemas
        let p3 = self.schema_formation(&scope, store).await?;
        // Phase 4: Emotional reprocessing — algorithmic salience adjustment
        let p4 = self.emotional_reprocessing(&scope, store).await?;
        // Phase 5: Pruning — remove stale low-salience sensory-layer orphans
        let p5 = self.pruning(&scope, store).await?;
        // Phase 6: REM recombination — LLM finds non-obvious connections
        let p6 = self.rem_recombination(&scope, store).await?;
        Ok(DreamReport { phases: vec![p1, p2, p3, p4, p5, p6] })
    }
}
```

LLM call budget: 20 per cycle per agent (mirrors Python `DREAM_MAX_LLM_CALLS`).
Phases 2, 3, 6 share this budget; the engine tracks consumed calls and stops early
if the budget is exhausted.

---

## 8. MCP server (the ApexOS integration point)

```rust
// crates/cerebro-mcp/src/main.rs

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let brain = Arc::new(CerebroCortex::new(Config::from_env()?).await?);
    let mut transport = StdioTransport::new(stdin(), stdout());

    // MCP initialize handshake
    let init_req = transport.read().await?;
    transport.write(handle_initialize(&init_req)).await?;

    // Main dispatch loop
    loop {
        match transport.read().await? {
            msg if msg.method == "tools/list" =>
                transport.write(tools_list_response()).await?,
            msg if msg.method == "tools/call" =>
                transport.write(dispatch_tool(msg, Arc::clone(&brain)).await).await?,
            msg => transport.write(method_not_found(&msg)).await?,
        }
    }
}
```

All 63 tools dispatch to `brain` methods. Tool descriptions are verbatim from the
Python MCP server — they're already written for agent consumption.

**Drop-in for ApexOS:** replace `plugins.toml` entry:
```toml
[[plugin]]
id   = "cerebro"
cmd  = "/usr/local/bin/cerebro-mcp"   # was: python -m cerebrocortex.mcp
restart = "always"
```
One line change. Same 63 tools. Same MCP contract. agentd never knows.

---

## 9. Build order for Claude Code

Each step is independently testable.

| Step | Module | Gate |
|------|--------|------|
| 1 | `types.rs` + `models/` | All types compile, serde round-trips |
| 2 | `activation/` | Values match Python fixture within 1e-4 |
| 3 | `storage/sqlite.rs` | Schema init, CRUD, scope filtering |
| 4 | `storage/vector.rs` | sqlite-vec loads, cosine search, FTS5 fallback |
| 5 | `storage/graph.rs` | petgraph rebuild + neighbor traversal |
| 6 | `engines/` (thalamus → neocortex, skip dream) | All 8 deterministic engines pass |
| 7 | `cortex.rs` | `remember()` + `recall()` end-to-end |
| 8 | `cerebro-mcp/` (core tools) | MCP handshake + remember/recall vs agentd |
| 9 | Remaining 61 MCP tools | Full tool surface |
| 10 | `engines/dream.rs` | All 6 phases, live LLM calls |
| 11 | `cerebro-cli/` + `cerebro-api/` | CLI and REST parity |
| 12 | DB compatibility test | Rust reads a Python-generated cerebro.db |

**Step 2 is the correctness gate.** Generate Python fixture values before writing
any Rust activation code. See Appendix B.

---

## 10. Dual CC session strategy

Two Claude Code sessions sharing git access, the live Pi Cerebro, and the ability to
message each other via `send_to_agent`.

**Session A — CerebroCortex-RS:** owns the port. Reference: Python Cerebro repo.
Writes procedural memories for Rust porting patterns discovered.

**Session B — ApexOS:** continues on Python Cerebro until step 8 clears.
Flips `plugins.toml` to the Rust binary when Session A hits step 8.

Neither blocks the other. André orchestrates. The flip to Rust is one line and a
`hot_reload_subsystem plugins` call.

---

## 11. Deferred to the keyboard

- `sqlite-vec` `.so` path on RaspiOS — confirm before writing storage layer
- `fastembed` first-run model download path and size on Pi
- Python ↔ Rust DB compatibility — confirm same schema during step 3
- CCBS (12 Markdown bootstrap modules) — defer until core is solid
- Vision extras (image/PDF/CLIP) — Phase 3 of the roadmap

---

## 12. First CC session checklist

- [x] Create `buckster123/CerebroCortex-RS` repo
- [x] Python `CerebroCortex` is beside us at `../CerebroCortex` — do NOT touch it
- [x] Workspace scaffolded per §3 — `cargo check` clean (warnings only)
- [x] All types, models, activation math, storage stubs, 9 engine stubs, cortex coordinator, MCP transport + dispatch skeleton, CLI, API stub, bench harness, deploy/cerebro.service
- [ ] Run `apt-cache show sqlite-vec` on the Pi; confirm `.so` path
- [ ] Run `fastembed` hello-world on the Pi; confirm model cache path
- [ ] Generate Python activation fixtures: `PYTHONPATH=../CerebroCortex/src python scripts/gen_activation_fixtures.py`
- [ ] Start at build-order step 1 (types serde round-trips already pass in integration_test.rs)

## 13. Notes from Python source inspection

- `EmotionalValence` enum exists in types.py — added to Rust types (was missing from original plan)
- `MediaType` enum exists — added (needed for vision/ingestion, defer Phase 3)
- `DreamPhase` enum exists — added
- Python `activation/strength.py` contains `compute_actr_activation(access_times, now, decay, noise=0.0)` — fixture script uses `noise=0.0` for deterministic ground truth
- Python `activation/spreading.py` is separate from `strength.py` — matched in Rust layout
- Python storage has three backends (SQLite + chromadb + igraph in-memory). Rust collapses to SQLite + sqlite-vec + petgraph — same single-file outcome, cleaner
- `LINK_DECAY_HALFLIFE_DAYS = 30` in Python config — ported to `config.rs`, used in `AssociativeLink::effective_weight()`
- MCP server exposes exactly 63 tools — full list captured in `cerebro-mcp/src/tools.rs`
- Python `interfaces/mcp_server.py` is the tool-description source of truth for step 9

---

## Appendix A — Python→Rust quick reference

| Python | Rust |
|--------|------|
| `@dataclass` / Pydantic model | `#[derive(Debug, Clone, Serialize, Deserialize)] struct` |
| `Enum` | `enum` + `#[serde(rename_all = "snake_case")]` |
| `Optional[T]` | `Option<T>` |
| `list[T]` | `Vec<T>` |
| `dict[K, V]` | `HashMap<K, V>` |
| `async def` / `await` | `async fn` / `.await` |
| `datetime.utcnow()` | `Utc::now()` (chrono) |
| `uuid4()` | `Uuid::new_v4()` |
| `float` (weights, salience) | `f32` (sufficient; half the memory of f64) |
| `logging.getLogger` | `tracing::info!` / `tracing::error!` |
| `pytest` fixture | `#[tokio::test]` + shared `TestDb` |

## Appendix B — Activation math fixture (step 2 gate)

Run this against the Python version to produce ground-truth values before writing
any Rust. The Rust step-2 tests load this JSON and assert values within `1e-4`.

```python
# scripts/gen_activation_fixtures.py
import json
from datetime import datetime, timedelta, timezone
from cerebro.activation.strength import compute_actr_activation, compute_fsrs_retrievability

now = datetime(2025, 1, 1, 12, 0, 0, tzinfo=timezone.utc)

cases = [
    ([60, 3600, 86400], 0.5, 5.0, 1.0),
    ([10, 10, 10],      0.5, 1.0, 0.5),
    ([86400 * 30],      0.5, 10.0, 30.0),
    ([3600, 7200],      0.5, 2.0, 7.0),
]

fixtures = []
for times, decay, stability, days in cases:
    access_times = [now - timedelta(seconds=s) for s in times]
    fixtures.append({
        "access_times_ago_secs": times,
        "decay": decay,
        "stability": stability,
        "days_since_review": days,
        "actr": compute_actr_activation(access_times, now, decay),
        "fsrs": compute_fsrs_retrievability(
            stability, now - timedelta(days=days), now
        ),
    })

with open("tests/fixtures/activation.json", "w") as f:
    json.dump(fixtures, f, indent=2)
print(f"wrote {len(fixtures)} fixtures")
```
