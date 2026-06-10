/// GatingEngine — thalamus.
/// Determines which inputs reach working memory (salience gating).
/// Mirrors Python engines/thalamus.py.
pub struct GatingEngine {
    pub salience_threshold: f32,
}

impl GatingEngine {
    pub fn new(salience_threshold: f32) -> Self {
        Self { salience_threshold }
    }

    /// Returns true if input salience clears the gate.
    pub fn gate(&self, salience: f32) -> bool {
        salience >= self.salience_threshold
    }

    /// Adjust salience based on emotional context (stub — full logic in step 6).
    pub fn adjust_salience(&self, base: f32, emotional_intensity: f32) -> f32 {
        (base + emotional_intensity * 0.1).clamp(0.0, 1.0)
    }
}
