# CerebroCortex-RS

Pure-Rust port of [CerebroCortex](https://github.com/buckster123/CerebroCortex) — a brain-analogous AI memory system with associative networks, ACT-R/FSRS activation, spreading activation, and a 6-phase dream engine.

**Status:** Scaffolded. Build-order step 1 (types + models) is the current target.

## Why

- Zero Python runtime on the Pi. Single binary, `scp` and done.
- ~10× smaller memory footprint vs CPython + venv + chromadb + igraph.
- ApexOS-RS readiness — prerequisite for a fully Rust-native stack.
- The activation math (ACT-R, FSRS, spreading activation) benefits from Rust's numerical performance for dream-engine churn.

## Architecture

```
crates/
  cerebro/        # library — all cognitive logic
  cerebro-mcp/    # MCP-over-stdio binary (63 tools, drop-in for ApexOS)
  cerebro-api/    # axum REST API + dashboard (optional)
  cerebro-cli/    # clap CLI
```

Full design: see [CEREBRO_RS_MASTERPLAN.md](CEREBRO_RS_MASTERPLAN.md).

## Dependency highlights

| Python | Rust |
|--------|------|
| `python-igraph` | `petgraph` |
| `chromadb` | `sqlite-vec` (ANN in SQLite) |
| `sentence-transformers` | `fastembed` (ONNX, no GPU) |
| `fastapi` + `uvicorn` | `axum` |
| MCP Python SDK | Hand-rolled JSON-RPC over stdio |

## Building

```bash
cargo build --release
```

## ApexOS drop-in

```toml
# plugins.toml
[[plugin]]
id   = "cerebro"
cmd  = "/usr/local/bin/cerebro-mcp"   # was: python -m cerebrocortex.mcp
restart = "always"
```

Same 63 MCP tools. Same wire format. agentd never knows.

## License

MIT
