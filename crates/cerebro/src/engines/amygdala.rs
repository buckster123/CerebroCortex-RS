use crate::types::EmotionalValence;

/// AffectEngine — amygdala.
/// Scores emotional salience of new memories and adjusts retrieval weighting.
/// Mirrors Python engines/amygdala.py.
pub struct AffectEngine;

impl AffectEngine {
    pub fn new() -> Self { Self }

    /// Classify emotional valence from content heuristics (stub).
    /// Full implementation uses keyword lists + optional sentiment model.
    pub fn classify_valence(&self, _content: &str) -> EmotionalValence {
        EmotionalValence::Neutral
    }

    /// Compute emotional salience boost [0.0, 0.5].
    pub fn salience_boost(&self, valence: EmotionalValence, intensity: f32) -> f32 {
        match valence {
            EmotionalValence::Neutral => 0.0,
            _ => (intensity * 0.5).clamp(0.0, 0.5),
        }
    }
}
