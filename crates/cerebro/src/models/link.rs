use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{LinkType, MemoryId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociativeLink {
    pub source_id:    MemoryId,
    pub target_id:    MemoryId,
    pub link_type:    LinkType,
    /// Base weight [0,1] — modified by link decay during spreading
    pub weight:       f32,
    pub created_at:   DateTime<Utc>,
    pub last_traversed: Option<DateTime<Utc>>,
    pub traversal_count: u32,
}

impl AssociativeLink {
    pub fn new(source_id: MemoryId, target_id: MemoryId, link_type: LinkType, weight: f32) -> Self {
        Self {
            source_id,
            target_id,
            link_type,
            weight: weight.clamp(0.0, 1.0),
            created_at: Utc::now(),
            last_traversed: None,
            traversal_count: 0,
        }
    }

    /// Effective weight accounting for link age decay (mirrors Python link_decay logic).
    pub fn effective_weight(&self, now: DateTime<Utc>, halflife_days: f32) -> f32 {
        let days_since = match self.last_traversed {
            Some(t) => (now - t).num_seconds() as f32 / 86400.0,
            None    => (now - self.created_at).num_seconds() as f32 / 86400.0,
        };
        let decay = 0.5_f32.powf(days_since / halflife_days);
        self.weight * decay
    }
}
