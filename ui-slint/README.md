# cerebro-ui — Lucida's native mirror (Slint)

The reading-surface twin of `ui-web/` (Lucida U5): dashboard panels plus the
Atlas and Thought lenses, rendered natively over the same `cerebro-api` JSON
surface. Simplified field — the few hundred brightest stars, batched
`Path`-drawn links, a timer-driven recall ripple. Dream observatory, Live
(EEG) and Health stay web-only until wanted; anything this app can do, the
web app can do (charter: no native-only writes).

## Run

```bash
# against the API (default http://127.0.0.1:8765)
cargo run -p ui-slint --release

CEREBRO_API_URL=http://127.0.0.1:8765 \
CEREBRO_API_TOKEN=…      # or AGENTD_TOKEN; empty = auth disabled (loopback)
LUCIDA_AGENT=FORGE       # optional visibility scope, like ?agent= on the web
cargo run -p ui-slint
```

| Env | Default | Purpose |
|-----|---------|---------|
| `CEREBRO_API_URL` | `http://127.0.0.1:8765` | cerebro-api base URL |
| `CEREBRO_API_TOKEN` / `AGENTD_TOKEN` | — | bearer token (same fallback order as the API) |
| `LUCIDA_AGENT` | — | agent scope for export + recall |
| `LUCIDA_STARS` | `400` | field cap: brightest N stars |
| `LUCIDA_EDGES` | `900` | field cap: strongest N links |
| `LUCIDA_SNAPSHOT` | — | write a PNG of the window after `LUCIDA_SNAPSHOT_MS` and exit (headless self-verification). Run with `SLINT_BACKEND=winit-software`: on femtovg, `take_snapshot` returns the last *presented* frame, and an occluded window stops presenting — the software renderer re-renders synchronously |
| `LUCIDA_SNAPSHOT_MS` | `2500` | snapshot delay |
| `LUCIDA_SNAPSHOT_QUERY` | — | boot-run a traced recall (Thought lens) before the snapshot |

## License note

Linking Slint places **this one binary** under Slint's license terms
(GPLv3 or the Slint Royalty-Free/commercial licenses). The rest of the
workspace is MIT and does not link Slint.
