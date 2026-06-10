use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use crate::{
    config::Config,
    engines::{
        AffectEngine, DreamEngine, EpisodicEngine, ExecutiveEngine, GatingEngine,
        LinkEngine, ProceduralEngine, SchemaEngine, SemanticEngine,
    },
    models::{AssociativeLink, MemoryNode},
    storage::StorageCoordinator,
    types::{MemoryId, MemoryType, VisibilityScope},
};

/// CerebroCortex — top-level coordinator.
/// Owns all 9 engines + the storage coordinator.
/// This is what `cerebro-mcp` and `cerebro-api` construct and share via Arc.
pub struct CerebroCortex {
    pub storage:     Arc<RwLock<StorageCoordinator>>,
    // Engines
    pub thalamus:    GatingEngine,
    pub amygdala:    AffectEngine,
    pub temporal:    SemanticEngine,
    pub hippocampus: EpisodicEngine,
    pub association: LinkEngine,
    pub cerebellum:  ProceduralEngine,
    pub prefrontal:  ExecutiveEngine,
    pub neocortex:   SchemaEngine,
    pub dream:       DreamEngine,
}

impl CerebroCortex {
    pub async fn new(config: Config) -> Result<Self> {
        let storage = StorageCoordinator::new(&config).await?;
        Ok(Self {
            storage:     Arc::new(RwLock::new(storage)),
            thalamus:    GatingEngine::new(),
            amygdala:    AffectEngine::new(),
            temporal:    SemanticEngine::new(),
            hippocampus: EpisodicEngine::new(),
            association: LinkEngine::new(),
            cerebellum:  ProceduralEngine::new(),
            prefrontal:  ExecutiveEngine::new(),
            neocortex:   SchemaEngine::new(),
            dream:       DreamEngine::new(config.anthropic_key),
        })
    }

    // -----------------------------------------------------------------------
    // Core operations (build-order step 7)
    // -----------------------------------------------------------------------

    /// Store a new memory, running it through all relevant engines.
    pub async fn remember(
        &self,
        content: impl Into<String>,
        memory_type: MemoryType,
        scope: VisibilityScope,
    ) -> Result<MemoryNode> {
        let content = content.into();
        let mut node = MemoryNode::new(content, memory_type);

        // Amygdala: classify emotional valence and apply salience modulation
        node = self.amygdala.apply_emotion(node);

        // Apply visibility scope
        if let Some(ref agent_id) = scope.agent_id {
            node.agent_id = Some(agent_id.clone());
        }

        let storage = self.storage.write().await;
        storage.sqlite.insert_memory(&node).await?;
        // TODO: embed and store vector (step 4)
        // TODO: add to graph (step 5)

        tracing::info!(id = %node.id.0, memory_type = ?node.memory_type, "memory stored");
        Ok(node)
    }

    /// Recall memories matching a query string.
    pub async fn recall(
        &self,
        query: &str,
        k: usize,
        scope: VisibilityScope,
    ) -> Result<Vec<(MemoryNode, f32)>> {
        // TODO: implement full recall pipeline (step 7):
        //   1. vector search  → candidate IDs + similarity scores
        //   2. ACT-R + FSRS   → activation scores per candidate
        //   3. spreading activation from top candidates
        //   4. weighted recall_score blend
        //   5. threshold + sort + top-k
        let _ = (query, k, scope);
        Ok(vec![])
    }

    /// Associate two existing memories with a typed link.
    pub async fn associate(
        &self,
        source: MemoryId,
        target: MemoryId,
        link: AssociativeLink,
    ) -> Result<()> {
        let storage = self.storage.write().await;
        // TODO: insert link into SQLite + graph (step 5)
        let _ = (source, target, link, storage);
        Ok(())
    }
}
