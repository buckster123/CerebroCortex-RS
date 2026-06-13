use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{AgentId, EmotionalValence, MemoryId, MemoryLayer, MemoryType, Visibility};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    pub id:            MemoryId,
    pub content:       String,
    pub memory_type:   MemoryType,
    pub layer:         MemoryLayer,
    pub salience:      f32,
    pub tags:          Vec<String>,
    pub agent_id:      Option<AgentId>,
    pub visibility:    Visibility,
    pub thread_id:     Option<String>,
    pub emotional_valence: Option<EmotionalValence>,
    pub emotional_intensity: f32,
    pub created_at:    DateTime<Utc>,
    pub updated_at:    DateTime<Utc>,
    pub access_count:  u32,
    /// ACT-R timestamps — capped at MAX_STORED_TIMESTAMPS (50)
    pub access_times:  Vec<DateTime<Utc>>,
    pub strength:      StrengthState,
    pub metadata:      serde_json::Value,
}

impl MemoryNode {
    pub fn new(content: impl Into<String>, memory_type: MemoryType) -> Self {
        let now = Utc::now();
        Self {
            id:                  MemoryId::new(),
            content:             content.into(),
            memory_type,
            layer:               MemoryLayer::Working,
            salience:            0.5,
            tags:                vec![],
            agent_id:            None,
            visibility:          Visibility::Shared,
            thread_id:           None,
            emotional_valence:   None,
            emotional_intensity: 0.0,
            created_at:          now,
            updated_at:          now,
            access_count:        0,
            access_times:        vec![now],
            strength:            StrengthState::default(),
            metadata:            serde_json::Value::Null,
        }
    }

    /// Record an access at `at`, bumping `access_count` and appending to
    /// `access_times` while enforcing the `MAX_STORED_TIMESTAMPS` cap (CB-030).
    ///
    /// The vec is the ACT-R retrieval history; keeping only the most-recent N
    /// entries bounds per-row growth on the recall hot path without changing the
    /// base-level-activation estimate (the oldest traces contribute least).
    pub fn record_access(&mut self, at: DateTime<Utc>) {
        self.access_count = self.access_count.saturating_add(1);
        self.access_times.push(at);
        let cap = crate::config::MAX_STORED_TIMESTAMPS;
        if self.access_times.len() > cap {
            // Drop the oldest, keep the `cap` most-recent timestamps.
            let drop = self.access_times.len() - cap;
            self.access_times.drain(0..drop);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrengthState {
    pub stability:   f32,  // FSRS S parameter
    pub difficulty:  f32,  // FSRS D parameter
    pub last_review: Option<DateTime<Utc>>,
}

impl Default for StrengthState {
    fn default() -> Self {
        Self {
            stability:   crate::config::FSRS_INITIAL_STABILITY,
            difficulty:  crate::config::FSRS_INITIAL_DIFFICULTY,
            last_review: None,
        }
    }
}
