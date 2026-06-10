#!/usr/bin/env python3
"""Generate ground-truth activation fixtures from the Python CerebroCortex.

Run against the Python install BEFORE writing Rust activation code.
The output JSON is loaded by tests/integration_test.rs step-2 assertions.

Usage (from CerebroCortex-RS root):
    PYTHONPATH=../CerebroCortex/src python scripts/gen_activation_fixtures.py
"""
import json
import math
import sys
from pathlib import Path

try:
    from cerebro.activation.strength import (
        base_level_activation,
        retrievability,
        update_stability_on_recall,
        update_stability_on_lapse,
        update_difficulty_on_recall,
        recall_probability,
    )
    from cerebro.config import ACTR_DECAY_RATE, ACTR_NOISE, ACTR_RETRIEVAL_THRESHOLD
    from cerebro.activation.spreading import effective_link_weight
except ImportError as e:
    print(f"ERROR: cannot import cerebro: {e}", file=sys.stderr)
    print("Set PYTHONPATH to the Python CerebroCortex src/.", file=sys.stderr)
    sys.exit(1)

# Fixed reference time — 2025-01-01 12:00:00 UTC as Unix timestamp
NOW = 1735732800.0  # python: datetime(2025,1,1,12,0,0,tzinfo=timezone.utc).timestamp()

# ---------------------------------------------------------------------------
# ACT-R base_level_activation fixtures
# Note: base_level_activation has NO noise parameter — noise is only in
# recall_probability(). All ACT-R values here are deterministic.
# ---------------------------------------------------------------------------
actr_cases = [
    # (timestamps_ago_secs, decay)
    ([60],                           0.5),   # single recent access
    ([60, 3600, 86400],              0.5),   # three accesses spread out
    ([10, 10, 10],                   0.5),   # three rapid accesses
    ([86400 * 30],                   0.5),   # one very old access
    ([3600, 7200],                   0.5),   # two accesses, same hour
    ([1],                            0.5),   # edge: floor-clamped to 1s
    ([60, 3600, 86400],              0.3),   # lower decay rate
    ([60, 3600, 86400],              0.7),   # higher decay rate
]

actr_fixtures = []
for times_ago, decay in actr_cases:
    timestamps = [NOW - t for t in times_ago]
    # compressed_count=0, compressed_avg_interval=0.0 (simple cases only)
    val = base_level_activation(timestamps, NOW, 0, 0.0, decay)
    actr_fixtures.append({
        "access_times_ago_secs": times_ago,
        "decay": decay,
        "actr": val,
    })

# ---------------------------------------------------------------------------
# FSRS retrievability fixtures
# R(t, S) = (1 + t / (9*S))^{-1}
# ---------------------------------------------------------------------------
fsrs_cases = [
    # (elapsed_days, stability)
    (0.0,   1.0),    # just accessed → 1.0
    (1.0,   1.0),    # 1 day, S=1
    (7.0,   2.0),    # 1 week, S=2
    (30.0,  5.0),    # 1 month, S=5
    (30.0, 10.0),    # 1 month, S=10 (high stability)
    (90.0, 30.0),    # 3 months, S=30
    (0.5,   1.0),    # half day, S=1
    (365.0, 365.0),  # 1 year, S=1yr — should be ~50%
]

fsrs_fixtures = []
for elapsed, stability in fsrs_cases:
    val = retrievability(elapsed, stability)
    fsrs_fixtures.append({
        "elapsed_days": elapsed,
        "stability":    stability,
        "retrievability": val,
    })

# ---------------------------------------------------------------------------
# FSRS update_stability_on_recall fixtures
# ---------------------------------------------------------------------------
stab_recall_cases = [
    # (stability, difficulty, current_retrievability)
    (1.0,  5.0, 0.9),   # easy recall (high R)
    (1.0,  5.0, 0.5),   # medium recall (mid R)
    (1.0,  5.0, 0.1),   # hard recall (low R) — biggest gain
    (5.0,  5.0, 0.9),
    (5.0,  8.0, 0.5),   # high difficulty dampens gain
    (30.0, 3.0, 0.7),   # high stability, easy
]

stab_recall_fixtures = []
for s, d, r in stab_recall_cases:
    new_s = update_stability_on_recall(s, d, r)
    stab_recall_fixtures.append({
        "stability":         s,
        "difficulty":        d,
        "retrievability":    r,
        "new_stability":     new_s,
    })

# ---------------------------------------------------------------------------
# FSRS update_stability_on_lapse fixtures
# ---------------------------------------------------------------------------
stab_lapse_cases = [
    (1.0,  5.0),
    (5.0,  5.0),
    (30.0, 3.0),
    (10.0, 8.0),
]

stab_lapse_fixtures = []
for s, d in stab_lapse_cases:
    new_s = update_stability_on_lapse(s, d)
    stab_lapse_fixtures.append({
        "stability":     s,
        "difficulty":    d,
        "new_stability": new_s,
    })

# ---------------------------------------------------------------------------
# FSRS update_difficulty_on_recall fixtures
# ---------------------------------------------------------------------------
diff_cases = [
    (5.0, 0.9),   # easy recall → difficulty drops
    (5.0, 0.1),   # hard recall → difficulty rises
    (5.0, 0.5),   # neutral
    (2.0, 0.9),
    (9.0, 0.1),
]

diff_fixtures = []
for d, r in diff_cases:
    new_d = update_difficulty_on_recall(d, r)
    diff_fixtures.append({
        "difficulty":         d,
        "retrievability":     r,
        "new_difficulty":     new_d,
    })

# ---------------------------------------------------------------------------
# recall_probability (ACT-R sigmoid) fixtures
# ---------------------------------------------------------------------------
recall_prob_cases = [
    # (activation, threshold, noise)
    (0.0,   0.0, 0.4),    # at threshold
    (1.0,   0.0, 0.4),    # above threshold
    (-1.0,  0.0, 0.4),    # below threshold
    (0.5,   0.0, 0.4),
    (-2.0,  0.0, 0.4),
]

recall_prob_fixtures = []
for act, tau, noise in recall_prob_cases:
    p = recall_probability(act, tau, noise)
    recall_prob_fixtures.append({
        "activation":  act,
        "threshold":   tau,
        "noise":       noise,
        "probability": p,
    })

# ---------------------------------------------------------------------------
# effective_link_weight (link decay) fixtures
# Python uses FSRS-style: w * (1 + age/9H)^{-1}
# ---------------------------------------------------------------------------
link_decay_cases = [
    # (stored_weight, age_days, halflife_days)
    (1.0,  0.0,  30.0),   # brand new → full weight
    (1.0,  30.0, 30.0),   # at halflife → ~50% (actually 1/(1+30/270) ≈ 0.9)
    (1.0,  270.0, 30.0),  # 9×halflife → 0.5 (FSRS halflife point)
    (0.8,  7.0,  30.0),
    (0.5,  90.0, 30.0),
]

link_decay_fixtures = []
for w, age, halflife in link_decay_cases:
    # Python effective_link_weight uses datetime.now() internally;
    # replicate the formula directly for deterministic fixtures
    if age <= 0 or halflife <= 0:
        eff = w
    else:
        decay = (1.0 + age / (9.0 * halflife)) ** (-1.0)
        eff = w * decay
    link_decay_fixtures.append({
        "stored_weight":  w,
        "age_days":       age,
        "halflife_days":  halflife,
        "effective_weight": eff,
    })

# ---------------------------------------------------------------------------
# Verify Python formula matches expectations in comments
# ---------------------------------------------------------------------------
print("Verifying fixtures against known values...")
# FSRS: R(0, S) should be 1.0
assert abs(fsrs_fixtures[0]["retrievability"] - 1.0) < 1e-9, "R(0,S)!=1"
# ACT-R: single access 1s ago → B = ln(1^{-0.5}) = 0
single_1s = base_level_activation([NOW - 1.0], NOW, 0, 0.0, 0.5)
assert abs(single_1s - 0.0) < 1e-6, f"B(1s)={single_1s}, expected 0"
print("  ✓ R(0,S) = 1.0")
print(f"  ✓ B(1s ago) = {single_1s:.6f} (expected 0.0)")

# ---------------------------------------------------------------------------
# Write output
# ---------------------------------------------------------------------------
out = {
    "meta": {
        "now_unix": NOW,
        "now_iso":  "2025-01-01T12:00:00Z",
        "actr_decay_default": ACTR_DECAY_RATE,
        "actr_noise_default": ACTR_NOISE,
        "actr_threshold_default": ACTR_RETRIEVAL_THRESHOLD,
    },
    "actr":              actr_fixtures,
    "fsrs_retrievability": fsrs_fixtures,
    "fsrs_update_recall":  stab_recall_fixtures,
    "fsrs_update_lapse":   stab_lapse_fixtures,
    "fsrs_update_difficulty": diff_fixtures,
    "recall_probability":  recall_prob_fixtures,
    "link_decay":          link_decay_fixtures,
}

path = Path(__file__).parent.parent / "crates" / "cerebro" / "tests" / "fixtures" / "activation.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(out, indent=2))
print(f"\nWrote {path}")
print(f"  {len(actr_fixtures)} ACT-R cases")
print(f"  {len(fsrs_fixtures)} FSRS retrievability cases")
print(f"  {len(stab_recall_fixtures)} stability-on-recall cases")
print(f"  {len(stab_lapse_fixtures)} stability-on-lapse cases")
print(f"  {len(diff_fixtures)} difficulty-update cases")
print(f"  {len(recall_prob_fixtures)} recall-probability cases")
print(f"  {len(link_decay_fixtures)} link-decay cases")
