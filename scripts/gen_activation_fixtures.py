#!/usr/bin/env python3
"""Generate ground-truth activation fixtures from the Python CerebroCortex.

Run against the Python install BEFORE writing Rust activation code.
The output JSON is loaded by tests/integration_test.rs step-2 assertions.

Usage (from CerebroCortex-RS root):
    PYTHONPATH=../CerebroCortex/src python scripts/gen_activation_fixtures.py
"""
import json
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

try:
    from cerebro.activation.strength import (
        compute_actr_activation,
        compute_fsrs_retrievability,
    )
except ImportError:
    print("ERROR: cannot import cerebro. Set PYTHONPATH to the Python CerebroCortex src/.",
          file=sys.stderr)
    sys.exit(1)

NOW = datetime(2025, 1, 1, 12, 0, 0, tzinfo=timezone.utc)

# (access_times_ago_secs, actr_decay, fsrs_stability, days_since_review)
CASES = [
    ([60, 3600, 86400],  0.5, 5.0,  1.0),
    ([10, 10, 10],       0.5, 1.0,  0.5),
    ([86400 * 30],       0.5, 10.0, 30.0),
    ([3600, 7200],       0.5, 2.0,  7.0),
    ([1],                0.5, 1.0,  0.0),   # edge: single very-recent access
]

fixtures = []
for times_ago, decay, stability, days in CASES:
    access_times = [NOW - timedelta(seconds=s) for s in times_ago]
    actr = compute_actr_activation(access_times, NOW, decay, noise=0.0)
    fsrs = compute_fsrs_retrievability(
        stability, NOW - timedelta(days=days), NOW
    )
    fixtures.append({
        "access_times_ago_secs": times_ago,
        "decay":                 decay,
        "stability":             stability,
        "days_since_review":     days,
        "actr":                  actr,
        "fsrs":                  fsrs,
    })

out = Path(__file__).parent.parent / "tests" / "fixtures" / "activation.json"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(fixtures, indent=2))
print(f"wrote {len(fixtures)} fixtures to {out}")
