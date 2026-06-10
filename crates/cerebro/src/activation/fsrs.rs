use chrono::{DateTime, Utc};

use crate::config::{FSRS_MAX_STABILITY, FSRS_MIN_STABILITY};

/// R(t, S) = (1 + t / (9 × S))^{-1}
/// Matches Python compute_fsrs_retrievability() in activation/strength.py.
pub fn retrievability(stability: f32, last_review: DateTime<Utc>, now: DateTime<Utc>) -> f32 {
    let t_days = (now - last_review).num_seconds().max(0) as f32 / 86400.0;
    (1.0 + t_days / (9.0 * stability)).powi(-1)
}

/// Update FSRS stability after a retrieval attempt.
/// Mirrors Python update_memory_strength().
pub fn update_stability(stability: f32, difficulty: f32, retrieved: bool) -> f32 {
    let s = if retrieved {
        stability * (1.0 + 0.1 * (11.0 - difficulty))
    } else {
        stability * 0.5
    };
    s.clamp(FSRS_MIN_STABILITY, FSRS_MAX_STABILITY)
}

/// Update FSRS difficulty (1–10 scale).
pub fn update_difficulty(difficulty: f32, retrieved: bool) -> f32 {
    let d = if retrieved { difficulty - 0.1 } else { difficulty + 0.2 };
    d.clamp(1.0, 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn just_reviewed_is_near_one() {
        let now = Utc::now();
        let r = retrievability(1.0, now - Duration::seconds(1), now);
        assert!(r > 0.99);
    }

    #[test]
    fn high_stability_decays_slower() {
        let now = Utc::now();
        let ago = now - Duration::days(7);
        let low  = retrievability(1.0,  ago, now);
        let high = retrievability(30.0, ago, now);
        assert!(high > low);
    }

    #[test]
    fn stability_increases_on_successful_retrieval() {
        let s0 = 2.0_f32;
        let s1 = update_stability(s0, 5.0, true);
        assert!(s1 > s0);
    }

    #[test]
    fn stability_decreases_on_failure() {
        let s0 = 2.0_f32;
        let s1 = update_stability(s0, 5.0, false);
        assert!(s1 < s0);
    }
}
