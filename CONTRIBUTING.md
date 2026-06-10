# Contributing to CerebroCortex-RS

Thanks for the interest. The project is a clean Rust port of a working Python system, so the bar is behavioural parity, not invention.

Read [ARCHITECTURE.md](ARCHITECTURE.md) before opening a PR — it explains the cognitive model, build order, and what the gates are. Read [CLAUDE.md](CLAUDE.md) for the development workflow including Pi deploy.

---

## Scope

CerebroCortex-RS is a **port, not a redesign.** Good PRs:

- Implement a build-order step or a piece of one
- Fix a deviation from the Python reference behaviour
- Add a missing test (especially activation math fixtures)
- Improve a doc that was wrong or incomplete

Out of scope for now:
- New cognitive features (add them to the Python original first, then port)
- New MCP tools not in the Python version
- Changing the 63-tool interface (it's a contract with agentd)
- Vision extras / CCBS bootstrap modules — deferred to Phase 3

---

## Getting started

```bash
git clone https://github.com/buckster123/CerebroCortex-RS
cd CerebroCortex-RS
cargo build
cargo test
```

You do not need the Python version to work on most of the Rust code. You need it only to regenerate activation fixtures (step 2):

```bash
# Requires ../CerebroCortex checked out alongside this repo
PYTHONPATH=../CerebroCortex/src python scripts/gen_activation_fixtures.py
```

---

## Branch and PR conventions

- Branch off `main`. Name: `step-3-sqlite-crud`, `fix-spreading-decay`, `add-episode-tests`
- One logical change per PR. Don't bundle unrelated cleanup with feature work.
- All existing tests must pass: `cargo test`
- New behaviour needs a new test. Activation math changes need fixture regeneration.
- No `#[allow(unused)]` or `todo!()` left in code that is supposed to be complete — stubs are fine in files clearly marked as build-order future steps.

---

## Correctness standard

The Python implementation at `../CerebroCortex` is the ground truth. When in doubt:

1. Read the Python source for the module you're porting
2. Check the constants in `cerebro/config.py` — they're mirrored exactly in `crates/cerebro/src/config.rs`
3. For activation math: the fixture test (`cargo test activation_fixtures`) is the gate — values must match within `1e-4`
4. For SQL schema: the column names and types must match the Python SQLite schema so that a Python-generated `cerebro.db` can be opened by Rust (step 12 gate)

---

## Code style

- No comments explaining *what* code does — good names do that. Comments only for non-obvious *why* (a hidden constraint, a Python gotcha, a numeric invariant).
- No docstrings / multi-line comment blocks.
- `f32` for all weights, salience, activation scores. `f64` only if you have a specific precision reason.
- `tracing::info!` / `tracing::warn!` for runtime logging. All log output goes to **stderr**. stdout is sacred MCP JSON-RPC.
- `anyhow::Result` for fallible public functions. `thiserror` for typed errors in the storage layer.

---

## Commit format

```
implement sqlite crud and scope filtering (step 3)
fix fsrs stability update on failed retrieval
add spreading activation tests for 2-hop decay
```

Imperative, lowercase, under 72 chars. No PR numbers. No "WIP". Include the step number if the commit completes or advances a build-order step.

---

## Reporting issues

If you find a behavioural difference from the Python version, open an issue with:
1. The Python tool call and result
2. The Rust equivalent call and result
3. Which config constants were in play (decay rate, weights, etc.)

---

## First release milestone

The first release (`v0.1.0`) is gated on:
- Build-order steps 1–9 complete (full MCP tool surface, no dream engine yet)
- Step 12: Rust reads a Python-generated `cerebro.db` without error
- `cerebro-mcp` running on Pi under systemd, wired into agentd as the `cerebro` plugin
- The Python version is retired from `plugins.toml`

Dream engine (step 10) is `v0.2.0`.
