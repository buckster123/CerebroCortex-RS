'use strict';
/* ============================================================
   Lucida (U1) — the Atlas lens over a real cerebro.db.
   Data: /graph/export (visual channels) + /graph/layout (cached
   PCA of the embeddings) + /stats + /memory/health.
   Interaction rules (charter, field feedback 2026-08-07):
   hover = highlight + tooltip ONLY; click = pinned select.
   ============================================================ */

const REDUCED = matchMedia('(prefers-reduced-motion: reduce)').matches;

/* ---------- auth: ?token=… once, then sessionStorage ---------- */
const urlParams = new URLSearchParams(location.search);
if (urlParams.get('token')) {
  sessionStorage.setItem('lucida-token', urlParams.get('token'));
  urlParams.delete('token');
  const qs = urlParams.toString();
  history.replaceState(null, '', location.pathname + (qs ? '?' + qs : ''));
}
const TOKEN = sessionStorage.getItem('lucida-token') || '';
const AGENT = urlParams.get('agent') || '';

async function api(path, opts = {}) {
  const headers = Object.assign(
    { 'Content-Type': 'application/json' },
    TOKEN ? { 'Authorization': 'Bearer ' + TOKEN } : {},
    opts.headers || {},
  );
  const resp = await fetch(path, Object.assign({}, opts, { headers }));
  if (resp.status === 401) {
    notice('Unauthorized. Reload with <code>?token=&lt;your token&gt;</code> in the URL.');
    throw new Error('unauthorized');
  }
  if (!resp.ok) throw new Error(path + ' → ' + resp.status);
  return resp.json();
}

function notice(html) {
  const el = document.getElementById('notice');
  el.innerHTML = html;
  el.hidden = false;
}

/* ---------- palette ---------- */
const TYPE_COLOR = {
  episodic:    '#c07f28',
  affective:   '#2fa8a0',
  semantic:    '#3a7de0',
  prospective: '#3aa76c',
  procedural:  '#8f6ee0',
  schematic:   '#cf5d96',
};
const RISK = '#e05252';
const FALLBACK_COLOR = '#8fa0b8';

/* ---------- state ---------- */
let nodes = [];               // {id,type,layer,salience,tags,head,chars,glow,retr,x,y,rim,…}
let edges = [];               // {a,b (indices), weight,eff,cold}
let edgeRank = [];            // edge indices sorted by effective weight, desc
let byId = new Map();
let adj = [];                 // node index → [edge index]
let health = null;

const view = { x: 0, y: 0, k: 0.55 };
let lens = 'atlas';
let hoverIdx = -1, selectedIdx = -1;
let searchSet = null;         // Set of node indices matched by the last recall
let dirty = true;

/* deterministic small hash for rim placement + jitter */
function idHash(s) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
  return (h >>> 0) / 4294967296;
}

/* ---------- boot ---------- */
async function boot() {
  const scopeQ = AGENT ? '?agent_id=' + encodeURIComponent(AGENT) : '';
  document.getElementById('agent-chip').textContent = AGENT ? '⚒ ' + AGENT : '◎ ALL';

  let exp, layout, stats, healthResp;
  try {
    [exp, layout, stats, healthResp] = await Promise.all([
      api('/graph/export' + scopeQ),
      api('/graph/layout'),
      api('/stats'),
      api('/memory/health' + scopeQ),
    ]);
  } catch (e) {
    if (e.message !== 'unauthorized')
      notice('Could not reach cerebro-api: <code>' + e.message + '</code>');
    return;
  }
  health = healthResp;

  const WORLD = 850;
  nodes = exp.nodes.map((n, i) => {
    const c = layout.coords[n.id];
    const h = idHash(n.id);
    let x, y, rim = false;
    if (c) {
      // Cached PCA position + a tiny deterministic jitter so exact-duplicate
      // embeddings don't stack into one star.
      x = c[0] * WORLD + (h - 0.5) * 30;
      y = c[1] * WORLD + (idHash(n.id + '·') - 0.5) * 30;
    } else {
      // No embedding (FTS5-only store, or pre-backfill row): the outer rim,
      // honestly outside the semantic map rather than faked into it.
      rim = true;
      const ang = h * Math.PI * 2;
      const r = WORLD * 1.35 + idHash(n.id + 'r') * 120;
      x = Math.cos(ang) * r;
      y = Math.sin(ang) * r * 0.72;
    }
    byId.set(n.id, i);
    return {
      id: n.id, type: n.memory_type, layer: n.layer, salience: n.salience,
      tags: n.tags || [], head: n.content_head, chars: n.content_chars,
      glow: n.activation, retr: n.retrievability, access: n.access_count,
      created: n.created_at, agent: n.agent_id,
      x, y, rim, twinkle: h * Math.PI * 2, degree: 0,
    };
  });

  edges = [];
  adj = nodes.map(() => []);
  for (const e of exp.edges) {
    const a = byId.get(e.source), b = byId.get(e.target);
    if (a === undefined || b === undefined) continue;
    const idx = edges.length;
    edges.push({ a, b, weight: e.weight, eff: e.effective_weight, cold: !e.last_traversed });
    adj[a].push(idx); adj[b].push(idx);
    nodes[a].degree++; nodes[b].degree++;
  }
  // Density LOD: a dense brain (auto-link + Hebbian encoding) can carry 30+
  // links per node — at overview zoom only the strongest strands may draw,
  // or the field becomes the hairball the charter forbids. Rank once.
  edgeRank = edges.map((_, i) => i).sort((i, j) => edges[j].eff - edges[i].eff);

  /* telemetry + health panel */
  const g = (health && health.graph) || {};
  setText('t-mem',   stats.total_memories);
  setText('t-links', stats.total_links);
  setText('t-cold',  fmtPct(g.never_traversed_links_pct));
  setText('t-comp',  g.components ?? '—');
  setText('h-comp',  g.components ?? '—');
  setText('h-iso',   (g.isolated_memories ?? '—') + ' (' + fmtPct(g.isolated_pct) + ')');
  setText('h-cold',  fmtPct(g.never_traversed_links_pct));
  setText('h-largest', g.largest_component ?? '—');

  if (exp.truncated)
    notice('Field truncated to ' + nodes.length + ' memories — raise <code>?cap=</code> on /graph/export when the LOD work lands.');

  dirty = true;
}
function setText(id, v) { document.getElementById(id).textContent = String(v); }
function fmtPct(v) { return v == null ? '—' : v + '%'; }

/* ---------- canvas ---------- */
const canvas = document.getElementById('field');
const ctx = canvas.getContext('2d');
let W = 0, H = 0, DPR = 1;
function resize() {
  DPR = Math.min(devicePixelRatio || 1, 2);
  W = innerWidth; H = innerHeight;
  canvas.width = W * DPR; canvas.height = H * DPR;
  canvas.style.width = W + 'px'; canvas.style.height = H + 'px';
  dirty = true;
}
addEventListener('resize', resize);
resize();

/* additive glow sprites, one per hue */
const SPRITE = 64;
const sprites = {};
function makeSprite(color) {
  const c = document.createElement('canvas');
  c.width = c.height = SPRITE;
  const g = c.getContext('2d');
  const grad = g.createRadialGradient(SPRITE/2, SPRITE/2, 0, SPRITE/2, SPRITE/2, SPRITE/2);
  grad.addColorStop(0, color);
  grad.addColorStop(0.25, color + 'b0');
  grad.addColorStop(0.6, color + '30');
  grad.addColorStop(1, color + '00');
  g.fillStyle = grad;
  g.fillRect(0, 0, SPRITE, SPRITE);
  return c;
}
for (const [t, c] of Object.entries(TYPE_COLOR)) sprites[t] = makeSprite(c);
sprites.risk = makeSprite(RISK);
sprites.fallback = makeSprite(FALLBACK_COLOR);

const sx = x => (x - view.x) * view.k + W / 2;
const sy = y => (y - view.y) * view.k + H / 2;

/* ---------- render ---------- */
let lastFrame = -1e9;
function render(now) {
  requestAnimationFrame(render);
  const animating = !REDUCED;
  if (!dirty && !animating) return;
  if (now - lastFrame < 30) return;
  lastFrame = now; dirty = false;

  ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
  ctx.fillStyle = '#070b12';
  ctx.fillRect(0, 0, W, H);

  const focusSet = selectedIdx >= 0 ? new Set(adj[selectedIdx]) : null;

  /* links — density LOD: at overview zoom only the strongest strands draw;
     zooming in earns the full web. Focus and health override per-edge. */
  const zoomShare = view.k < 0.8 ? 0.08 : view.k < 1.4 ? 0.35 : 1.0;
  const drawCount = Math.min(edges.length,
    Math.max(400, Math.floor(edges.length * zoomShare)));
  const densityDim = Math.min(1, 900 / Math.max(1, drawCount));
  ctx.lineWidth = 1;
  for (let r = 0; r < edges.length; r++) {
    const i = edgeRank[r];
    const e = edges[i];
    const inLod = r < drawCount;
    const focused = focusSet && focusSet.has(i);
    if (!inLod && !focused && lens !== 'health') continue;

    const A = nodes[e.a], B = nodes[e.b];
    const x1 = sx(A.x), y1 = sy(A.y), x2 = sx(B.x), y2 = sy(B.y);
    if (Math.max(x1, x2) < 0 || Math.min(x1, x2) > W ||
        Math.max(y1, y2) < 0 || Math.min(y1, y2) > H) continue;

    let alpha, color = '122,152,199';
    if (lens === 'health') {
      if (!inLod && !e.cold) continue;           /* cold strands are the point */
      alpha = (e.cold ? 0.20 : 0.07) * Math.max(densityDim, 0.35);
      if (e.cold) color = '90,140,235';
    } else if (focused) {
      alpha = 0.55;
    } else if (selectedIdx >= 0 || searchSet) {
      alpha = 0.02 * densityDim;
    } else {
      alpha = (0.04 + e.eff * 0.14) * densityDim;
    }
    ctx.strokeStyle = `rgba(${color},${alpha})`;
    ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
  }

  /* stars */
  ctx.save();
  ctx.globalCompositeOperation = 'lighter';
  const t = now / 1000;
  for (let i = 0; i < nodes.length; i++) {
    const n = nodes[i];
    const x = sx(n.x), y = sy(n.y);
    if (x < -40 || x > W + 40 || y < -40 || y > H + 40) continue;

    /* brightness: retrievability is the long decay, activation the recent heat */
    let act = Math.max(0.06, Math.min(1, 0.2 * n.glow + 0.8 * n.retr));
    if (focusSet) {
      const isNeighbor = adj[i].some(li => focusSet.has(li));
      if (i !== selectedIdx && !isNeighbor) act *= 0.22;
    }
    if (searchSet && !searchSet.has(i) && i !== selectedIdx) act *= 0.15;
    if (!REDUCED) act *= 0.93 + 0.07 * Math.sin(t * 1.7 + n.twinkle);

    const layerBoost = n.layer === 'working' ? 1.25 : 1;
    const size = (3.2 + n.salience * 9) * view.k * layerBoost;
    const sprite = (lens === 'health' && n.degree === 0) ? sprites.risk
                 : (sprites[n.type] || sprites.fallback);
    ctx.globalAlpha = Math.min(1, 0.12 + act * 0.88);
    ctx.drawImage(sprite, x - size, y - size, size * 2, size * 2);

    ctx.globalAlpha = Math.min(1, 0.35 + act * 0.6);
    ctx.fillStyle = '#dfe9f8';
    ctx.fillRect(x - 0.7, y - 0.7, 1.4, 1.4);
  }
  ctx.restore();

  /* isolated rings (health lens) */
  if (lens === 'health') {
    ctx.strokeStyle = 'rgba(224,82,82,0.5)';
    ctx.lineWidth = 1;
    for (let i = 0; i < nodes.length; i++) {
      if (nodes[i].degree !== 0) continue;
      const x = sx(nodes[i].x), y = sy(nodes[i].y);
      if (x < -20 || x > W + 20 || y < -20 || y > H + 20) continue;
      ctx.beginPath(); ctx.arc(x, y, 9 * view.k + 4, 0, 7); ctx.stroke();
    }
  }

  /* hover + selection rings */
  for (const [idx, alpha] of [[hoverIdx, 0.85], [selectedIdx, 1.0]]) {
    if (idx < 0) continue;
    const n = nodes[idx];
    ctx.strokeStyle = `rgba(216,225,239,${alpha})`;
    ctx.lineWidth = idx === selectedIdx ? 1.6 : 1.2;
    ctx.beginPath();
    ctx.arc(sx(n.x), sy(n.y), (3.2 + n.salience * 9) * view.k + 5, 0, 7);
    ctx.stroke();
  }
}
requestAnimationFrame(render);

/* ---------- memory card (pinned on select) ---------- */
const card = document.getElementById('card');
async function openCard(i) {
  const n = nodes[i];
  const color = TYPE_COLOR[n.type] || FALLBACK_COLOR;
  document.getElementById('card-chip').style.background = color;
  document.getElementById('card-type').textContent =
    n.type + (n.rim ? ' · rim (no embedding)' : '');
  document.getElementById('card-id').textContent = n.id.slice(0, 18);
  document.getElementById('card-tags').innerHTML =
    n.tags.map(t => `<span class="tag"></span>`).join('');
  document.querySelectorAll('#card .tag').forEach((el, k) => { el.textContent = n.tags[k]; });
  setText('card-sal', n.salience.toFixed(2));
  setText('card-act', n.glow.toFixed(2));
  setText('card-ret', n.retr.toFixed(2));
  setText('card-acc', n.access);
  setText('card-layer', n.layer);
  setText('card-born', (n.created || '').slice(0, 10));
  /* index first (the head), then the full body */
  document.getElementById('card-content').textContent =
    n.head + (n.chars > n.head.length ? '…' : '');
  card.classList.add('open');
  if (n.chars > n.head.length) {
    try {
      const full = await api('/memory/' + encodeURIComponent(n.id) +
        (AGENT ? '?agent_id=' + encodeURIComponent(AGENT) : ''));
      if (selectedIdx === i) {
        document.getElementById('card-content').textContent = full.content;
      }
    } catch { /* head stays — honest partial */ }
  }
}
function deselect() {
  selectedIdx = -1;
  card.classList.remove('open');
  dirty = true;
}
document.getElementById('card-close').addEventListener('click', deselect);

function selectNode(i) {
  selectedIdx = i;
  openCard(i);
  const n = nodes[i];
  view.x += (n.x - view.x) * 0.35;
  view.y += (n.y - view.y) * 0.35;
  dirty = true;
}

/* ---------- interaction: pan / zoom / hover / click ---------- */
const tooltip = document.getElementById('tooltip');
let dragging = false, moved = false, px = 0, py = 0;
canvas.addEventListener('pointerdown', e => {
  dragging = true; moved = false; px = e.clientX; py = e.clientY;
  tooltip.style.display = 'none';
  canvas.classList.add('dragging'); canvas.setPointerCapture(e.pointerId);
});
canvas.addEventListener('pointermove', e => {
  if (dragging) {
    const dx = e.clientX - px, dy = e.clientY - py;
    if (Math.abs(dx) + Math.abs(dy) > 3) moved = true;
    view.x -= dx / view.k; view.y -= dy / view.k;
    px = e.clientX; py = e.clientY; dirty = true;
  } else {
    let best = -1, bestD = 196;
    for (let i = 0; i < nodes.length; i++) {
      const dx = sx(nodes[i].x) - e.clientX, dy = sy(nodes[i].y) - e.clientY;
      const d = dx * dx + dy * dy;
      if (d < bestD) { bestD = d; best = i; }
    }
    if (best !== hoverIdx) {
      hoverIdx = best; dirty = true;
      canvas.style.cursor = best >= 0 ? 'pointer' : 'grab';
      if (best >= 0) {
        const n = nodes[best];
        tooltip.textContent = n.type + ' · ' + n.head.slice(0, 46) + (n.chars > 46 ? '…' : '');
        tooltip.style.display = 'block';
      } else {
        tooltip.style.display = 'none';
      }
    }
    if (best >= 0) {
      tooltip.style.left = (e.clientX + 14) + 'px';
      tooltip.style.top  = (e.clientY + 12) + 'px';
    }
  }
});
canvas.addEventListener('pointerup', () => {
  dragging = false; canvas.classList.remove('dragging');
  if (!moved) {
    if (hoverIdx >= 0) selectNode(hoverIdx);
    else deselect();
  }
});
canvas.addEventListener('wheel', e => {
  e.preventDefault();
  const k0 = view.k;
  view.k = Math.min(3.2, Math.max(0.18, view.k * (e.deltaY < 0 ? 1.12 : 0.89)));
  const mx = e.clientX - W / 2, my = e.clientY - H / 2;
  view.x += mx / k0 - mx / view.k;
  view.y += my / k0 - my / view.k;
  dirty = true;
}, { passive: false });
addEventListener('keydown', e => {
  if (e.key === 'Escape') { deselect(); clearSearch(); }
});

/* ---------- recall (Atlas search — the ripple animation is U2) ---------- */
async function runRecall() {
  const query = document.getElementById('query').value.trim();
  if (!query) return;
  let results;
  try {
    results = await api('/recall', {
      method: 'POST',
      body: JSON.stringify(Object.assign(
        { query, top_k: 12 }, AGENT ? { agent_id: AGENT } : {},
      )),
    });
  } catch { return; }

  const list = document.getElementById('results-list');
  list.innerHTML = '';
  searchSet = new Set();
  for (const r of results) {
    const mem = r.memory, idx = byId.get(mem.id);
    if (idx !== undefined) searchSet.add(idx);
    const li = document.createElement('li');
    const color = TYPE_COLOR[mem.memory_type] || FALLBACK_COLOR;
    li.innerHTML = `<span class="chip" style="background:${color}"></span>` +
      `<span class="head"></span><span class="score">${(r.score ?? 0).toFixed(2)}</span>`;
    li.querySelector('.head').textContent = mem.content;
    if (idx !== undefined) li.addEventListener('click', () => selectNode(idx));
    list.appendChild(li);
  }
  document.getElementById('results-meta').textContent =
    results.length + ' recalled · real pipeline (spread + rank)';
  deselect();                                       /* hides the card only */
  document.getElementById('results').classList.add('open');
  dirty = true;
}
function clearSearch() {
  searchSet = null;
  document.getElementById('results').classList.remove('open');
  dirty = true;
}
document.getElementById('recall-btn').addEventListener('click', runRecall);
document.getElementById('query').addEventListener('keydown', e => {
  if (e.key === 'Enter') runRecall();
});
document.getElementById('results-clear').addEventListener('click', clearSearch);

/* ---------- lenses ---------- */
const PLACARDS = {
  thought: ['U2', 'Thought — recall replay',
    'Type a query and watch the actual spreading-activation wavefront propagate hop by hop along the real walked edges, then crystallize into the ranked constellation. The trace exists in the engine (spread_traced) — this lens ships when /recall/trace lands.'],
  dream: ['U4', 'Dream observatory',
    'Dream reports on a timeline. Scrub a cycle to watch phase effects as overlay diffs: pruned memories collapse to embers, a newborn schema flares with rays to its sources, the skill-competition champion takes a halo while dominated rivals dim toward the prune floor.'],
  live: ['U3', 'Live — the EEG',
    'The observatory tails the audit log over SSE. When any agent remembers, recalls, or dreams against this brain, the event ripples across the field within a second or two. The shared SQLite is already the bus.'],
};
const placard = document.getElementById('placard');
function setLens(name) {
  lens = name;
  document.body.dataset.lens = name;
  document.querySelectorAll('#lenses button').forEach(b =>
    b.setAttribute('aria-pressed', String(b.dataset.lens === name)));
  const pl = PLACARDS[name];
  if (pl) {
    document.getElementById('placard-phase').textContent = 'CHARTER · ' + pl[0];
    document.getElementById('placard-title').textContent = pl[1];
    document.getElementById('placard-body').textContent = pl[2];
    placard.classList.add('open');
  } else {
    placard.classList.remove('open');
  }
  dirty = true;
}
document.querySelectorAll('#lenses button').forEach(b =>
  b.addEventListener('click', () => setLens(b.dataset.lens)));
placard.addEventListener('click', () => setLens('atlas'));

/* shareable boot params: ?lens=health opens a lens, ?q=… runs a recall */
boot().then(() => {
  const lensParam = urlParams.get('lens');
  if (lensParam && document.querySelector(`#lenses [data-lens="${lensParam}"]`))
    setLens(lensParam);
  const q = urlParams.get('q');
  if (q) {
    document.getElementById('query').value = q;
    runRecall();
  }
});
