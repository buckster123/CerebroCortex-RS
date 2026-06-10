pub mod actr;
pub mod fsrs;
pub mod spreading;

pub use actr::base_level_activation;
pub use fsrs::{retrievability, update_stability};
pub use spreading::spread;

/// Combined recall score — mirrors Python's weighted blend exactly.
/// vector_sim ∈ [0,1], actr is log-scale (can be negative), fsrs ∈ [0,1], salience ∈ [0,1].
pub fn recall_score(vector_sim: f32, actr: f32, fsrs: f32, salience: f32) -> f32 {
    use crate::config::{SCORE_WEIGHT_ACTIVATION, SCORE_WEIGHT_RETRIEVABILITY,
                        SCORE_WEIGHT_SALIENCE, SCORE_WEIGHT_VECTOR};
    SCORE_WEIGHT_VECTOR       * vector_sim
        + SCORE_WEIGHT_ACTIVATION * actr.clamp(-5.0, 5.0).mul_add(0.1, 0.5) // normalise to [0,1]
        + SCORE_WEIGHT_RETRIEVABILITY * fsrs
        + SCORE_WEIGHT_SALIENCE * salience
}
