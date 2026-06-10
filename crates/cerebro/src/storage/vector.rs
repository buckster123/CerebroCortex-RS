use anyhow::Result;

use crate::{storage::SqliteStore, types::MemoryId};

/// Vector similarity search via sqlite-vec extension + FTS5 keyword fallback.
/// sqlite-vec: <https://github.com/asg017/sqlite-vec>
/// The extension (.so) is loaded at runtime; path is resolved from SQLITE_VEC_PATH env var
/// or the system default. Falls back gracefully to FTS5 if extension is unavailable.
pub struct VectorStore {
    embed_model: String,
    vec_available: bool,
}

impl VectorStore {
    pub async fn new(_sqlite: &SqliteStore, embed_model: &str) -> Result<Self> {
        // TODO: attempt to load sqlite-vec extension, set vec_available accordingly
        // TODO: initialise fastembed model (first run downloads ~33MB)
        Ok(Self {
            embed_model: embed_model.to_string(),
            vec_available: false,
        })
    }

    /// Embed content and store vector in memories.embedding column.
    pub async fn embed_and_store(&self, _memory_id: &MemoryId, _content: &str) -> Result<Vec<f32>> {
        // TODO: implement in build-order step 4
        Ok(vec![])
    }

    /// Return top-k most similar memory IDs with cosine similarity scores.
    /// Falls back to FTS5 keyword search if sqlite-vec is not loaded.
    pub async fn search(
        &self,
        _query: &str,
        _k: usize,
        _scope_sql: &str,
        _scope_params: &[String],
    ) -> Result<Vec<(MemoryId, f32)>> {
        // TODO: implement in build-order step 4
        Ok(vec![])
    }

    pub fn is_vec_available(&self) -> bool { self.vec_available }
}
