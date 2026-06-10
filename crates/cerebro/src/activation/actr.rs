use chrono::{DateTime, Utc};
use rand::Rng;

use crate::config::{ACTR_DECAY_RATE, ACTR_MIN_TIME_SECONDS, ACTR_NOISE};

/// B(t) = ln( Σ t_k^{-d} ) + ε
/// where t_k is seconds since k-th access and ε ~ Uniform(-noise, noise).
/// Matches Python compute_actr_activation() in activation/strength.py.
pub fn base_level_activation(
    access_times: &[DateTime<Utc>],
    now: DateTime<Utc>,
    decay: f32,
    noise: f32,
) -> f32 {
    if access_times.is_empty() {
        return f32::NEG_INFINITY;
    }
    let sum: f32 = access_times
        .iter()
        .map(|t| {
            let secs = (now - *t).num_seconds().max(1) as f32;
            secs.max(ACTR_MIN_TIME_SECONDS).powf(-decay)
        })
        .sum();
    let base = sum.ln();
    let epsilon: f32 = rand::thread_rng().gen_range(-noise..noise);
    base + epsilon
}

/// Convenience wrapper using default decay + noise from config.
pub fn actr_activation(access_times: &[DateTime<Utc>], now: DateTime<Utc>) -> f32 {
    base_level_activation(access_times, now, ACTR_DECAY_RATE, ACTR_NOISE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn single_recent_access_is_finite() {
        let now = Utc::now();
        let times = vec![now - Duration::seconds(60)];
        let b = actr_activation(&times, now);
        assert!(b.is_finite());
    }

    #[test]
    fn empty_access_is_neg_inf() {
        let b = actr_activation(&[], Utc::now());
        assert_eq!(b, f32::NEG_INFINITY);
    }

    #[test]
    fn more_recent_access_wins() {
        let now = Utc::now();
        let recent  = vec![now - Duration::seconds(10)];
        let stale   = vec![now - Duration::seconds(86400 * 30)];
        // Run many times to average out noise
        let avg = |times: &[DateTime<Utc>]| -> f32 {
            (0..50).map(|_| base_level_activation(times, now, 0.5, 0.0)).sum::<f32>() / 50.0
        };
        assert!(avg(&recent) > avg(&stale));
    }
}
