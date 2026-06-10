/// LinkEngine — Hebbian association.
/// Creates and strengthens links between co-activated memories.
/// Mirrors Python engines/association.py.
pub struct LinkEngine {
    pub hebbian_threshold: f32,
}

impl LinkEngine {
    pub fn new(hebbian_threshold: f32) -> Self {
        Self { hebbian_threshold }
    }
}
