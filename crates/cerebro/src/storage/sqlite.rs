use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};
use tokio::sync::Mutex;

use crate::{
    models::{AssociativeLink, MemoryNode},
    types::{MemoryId, VisibilityScope},
};

/// SQLite backend — single source of truth for all persistent state.
/// Graph and vector index are derived from this; never written independently.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let store = Self { conn: Mutex::new(conn) };
        store.init_schema().await?;
        Ok(store)
    }

    async fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(())
    }

    pub async fn insert_memory(&self, node: &MemoryNode) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO memories \
             (id, content, memory_type, layer, salience, tags, agent_id, visibility, \
              thread_id, emotional_valence, emotional_intensity, \
              created_at, updated_at, access_count, access_times, \
              fsrs_stability, fsrs_difficulty, fsrs_last_review, metadata) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            params![
                node.id.0,
                node.content,
                serde_json::to_string(&node.memory_type)?,
                serde_json::to_string(&node.layer)?,
                node.salience,
                serde_json::to_string(&node.tags)?,
                node.agent_id.as_ref().map(|a| &a.0),
                serde_json::to_string(&node.visibility)?,
                node.thread_id,
                node.emotional_valence.as_ref().map(|v| serde_json::to_string(v).unwrap()),
                node.emotional_intensity,
                node.created_at.to_rfc3339(),
                node.updated_at.to_rfc3339(),
                node.access_count,
                serde_json::to_string(&node.access_times)?,
                node.strength.stability,
                node.strength.difficulty,
                node.strength.last_review.map(|t| t.to_rfc3339()),
                serde_json::to_string(&node.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub async fn get_memory(&self, id: &MemoryId, scope: &VisibilityScope) -> Result<Option<MemoryNode>> {
        // TODO: implement in build-order step 3
        let _ = (id, scope);
        Ok(None)
    }

    pub async fn list_links_from(&self, id: &MemoryId) -> Result<Vec<AssociativeLink>> {
        // TODO: implement in build-order step 3
        let _ = id;
        Ok(vec![])
    }
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS memories (
    id                  TEXT PRIMARY KEY,
    content             TEXT NOT NULL,
    memory_type         TEXT NOT NULL,
    layer               TEXT NOT NULL DEFAULT 'working',
    salience            REAL NOT NULL DEFAULT 0.5,
    tags                TEXT NOT NULL DEFAULT '[]',
    agent_id            TEXT,
    visibility          TEXT NOT NULL DEFAULT 'shared',
    thread_id           TEXT,
    emotional_valence   TEXT,
    emotional_intensity REAL NOT NULL DEFAULT 0.0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    access_count        INTEGER NOT NULL DEFAULT 0,
    access_times        TEXT NOT NULL DEFAULT '[]',
    fsrs_stability      REAL NOT NULL DEFAULT 1.0,
    fsrs_difficulty     REAL NOT NULL DEFAULT 5.0,
    fsrs_last_review    TEXT,
    metadata            TEXT NOT NULL DEFAULT 'null',
    embedding           BLOB,
    deleted_at          TEXT
);

CREATE TABLE IF NOT EXISTS links (
    source_id           TEXT NOT NULL,
    target_id           TEXT NOT NULL,
    link_type           TEXT NOT NULL,
    weight              REAL NOT NULL DEFAULT 0.5,
    created_at          TEXT NOT NULL,
    last_traversed      TEXT,
    traversal_count     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (source_id, target_id, link_type),
    FOREIGN KEY (source_id) REFERENCES memories(id),
    FOREIGN KEY (target_id) REFERENCES memories(id)
);

CREATE TABLE IF NOT EXISTS agents (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT,
    registered_at   TEXT NOT NULL,
    last_seen       TEXT,
    metadata        TEXT NOT NULL DEFAULT 'null'
);

CREATE TABLE IF NOT EXISTS episodes (
    id          TEXT PRIMARY KEY,
    title       TEXT,
    agent_id    TEXT,
    thread_id   TEXT,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    summary     TEXT,
    memory_ids  TEXT NOT NULL DEFAULT '[]',
    metadata    TEXT NOT NULL DEFAULT 'null'
);

CREATE TABLE IF NOT EXISTS episode_steps (
    episode_id  TEXT NOT NULL,
    step_index  INTEGER NOT NULL,
    description TEXT NOT NULL,
    memory_id   TEXT,
    timestamp   TEXT NOT NULL,
    PRIMARY KEY (episode_id, step_index),
    FOREIGN KEY (episode_id) REFERENCES episodes(id)
);

CREATE TABLE IF NOT EXISTS tags (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memory_tags (
    memory_id   TEXT NOT NULL,
    tag_id      INTEGER NOT NULL,
    PRIMARY KEY (memory_id, tag_id),
    FOREIGN KEY (memory_id) REFERENCES memories(id),
    FOREIGN KEY (tag_id)    REFERENCES tags(id)
);

CREATE TABLE IF NOT EXISTS audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL,
    agent_id    TEXT,
    action      TEXT NOT NULL,
    memory_id   TEXT,
    details     TEXT
);

CREATE TABLE IF NOT EXISTS dream_reports (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    phases      TEXT NOT NULL DEFAULT '[]',
    metadata    TEXT NOT NULL DEFAULT 'null'
);

-- FTS5 virtual table for keyword search (FTS5 fallback when vector search unavailable)
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id UNINDEXED,
    content,
    tags,
    content='memories',
    content_rowid='rowid'
);

CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, id, content, tags)
    VALUES (new.rowid, new.id, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.content, old.tags);
END;

CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.content, old.tags);
    INSERT INTO memories_fts(rowid, id, content, tags)
    VALUES (new.rowid, new.id, new.content, new.tags);
END;

-- Indices for common query patterns
CREATE INDEX IF NOT EXISTS idx_memories_agent    ON memories(agent_id);
CREATE INDEX IF NOT EXISTS idx_memories_type     ON memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_layer    ON memories(layer);
CREATE INDEX IF NOT EXISTS idx_memories_vis      ON memories(visibility);
CREATE INDEX IF NOT EXISTS idx_memories_thread   ON memories(thread_id);
CREATE INDEX IF NOT EXISTS idx_memories_deleted  ON memories(deleted_at);
CREATE INDEX IF NOT EXISTS idx_links_source      ON links(source_id);
CREATE INDEX IF NOT EXISTS idx_links_target      ON links(target_id);
CREATE INDEX IF NOT EXISTS idx_audit_ts          ON audit_log(timestamp);
"#;
