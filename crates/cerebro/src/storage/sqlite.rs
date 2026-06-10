use std::{path::Path, sync::{Arc, OnceLock}};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::Mutex;

use crate::{
    models::{AssociativeLink, MemoryNode, StrengthState},
    types::{AgentId, LinkType, MemoryId, VisibilityScope},
};

/// Register the sqlite-vec extension exactly once for the process.
/// Uses sqlite3_auto_extension so every subsequent Connection::open* call has vec0.
/// Returns true if the extension was successfully registered.
fn register_sqlite_vec() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| {
        unsafe {
            use rusqlite::ffi::sqlite3_auto_extension;
            use sqlite_vec::sqlite3_vec_init;
            // sqlite3_auto_extension expects void(*)(void); transmute is the
            // canonical way to bridge the extension init signature in Rust.
            sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
        }
        true
    })
}

/// SQLite backend — single source of truth for all persistent state.
/// Graph and vector index are derived from this; never written independently.
pub struct SqliteStore {
    conn:              Arc<Mutex<Connection>>,
    /// True when sqlite-vec was successfully registered and the vec0 table exists.
    pub vec_available: bool,
}

/// Filters for `list_memories_scoped`.
pub struct ListFilter {
    pub memory_type:     Option<crate::types::MemoryType>,
    pub limit:           usize,
    pub offset:          usize,
    pub include_deleted: bool,
}

impl Default for ListFilter {
    fn default() -> Self {
        Self { memory_type: None, limit: 50, offset: 0, include_deleted: false }
    }
}

// ---------------------------------------------------------------------------
// Enum helpers — store as plain snake_case strings (no JSON quotes) matching
// the Python schema storage format and the SQL filter literals.
// ---------------------------------------------------------------------------

fn enum_to_str<T: Serialize>(val: &T) -> Result<String> {
    let json = serde_json::to_string(val)?;
    Ok(json.trim_matches('"').to_string())
}

fn str_to_enum<T: DeserializeOwned>(s: &str) -> Result<T> {
    Ok(serde_json::from_str(&format!("\"{}\"", s))?)
}

// ---------------------------------------------------------------------------
// Raw row type — extracts primitive values from a rusqlite Row without any
// fallible serde parsing (keeping the closure return type clean).
// Post-process with .into_memory_node() outside the query closure.
// ---------------------------------------------------------------------------

struct RawMemoryRow {
    id:                    String,
    content:               String,
    memory_type_str:       String,
    layer_str:             String,
    salience:              f32,
    tags_json:             String,
    agent_id:              Option<String>,
    visibility_str:        String,
    thread_id:             Option<String>,
    emotional_valence_str: Option<String>,
    emotional_intensity:   f32,
    created_at_str:        String,
    updated_at_str:        String,
    access_count:          i64,
    access_times_json:     String,
    fsrs_stability:        f32,
    fsrs_difficulty:       f32,
    fsrs_last_review_str:  Option<String>,
    metadata_json:         String,
}

fn row_to_raw(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawMemoryRow> {
    Ok(RawMemoryRow {
        id:                    row.get(0)?,
        content:               row.get(1)?,
        memory_type_str:       row.get(2)?,
        layer_str:             row.get(3)?,
        salience:             (row.get::<_, f64>(4)? as f32),
        tags_json:             row.get(5)?,
        agent_id:              row.get(6)?,
        visibility_str:        row.get(7)?,
        thread_id:             row.get(8)?,
        emotional_valence_str: row.get(9)?,
        emotional_intensity:  (row.get::<_, f64>(10)? as f32),
        created_at_str:        row.get(11)?,
        updated_at_str:        row.get(12)?,
        access_count:          row.get(13)?,
        access_times_json:     row.get(14)?,
        fsrs_stability:       (row.get::<_, f64>(15)? as f32),
        fsrs_difficulty:      (row.get::<_, f64>(16)? as f32),
        fsrs_last_review_str:  row.get(17)?,
        metadata_json:         row.get(18)?,
    })
}

impl RawMemoryRow {
    fn into_memory_node(self) -> Result<MemoryNode> {
        Ok(MemoryNode {
            id:                  MemoryId(self.id),
            content:             self.content,
            memory_type:         str_to_enum(&self.memory_type_str)?,
            layer:               str_to_enum(&self.layer_str)?,
            salience:            self.salience,
            tags:                serde_json::from_str(&self.tags_json)?,
            agent_id:            self.agent_id.map(AgentId),
            visibility:          str_to_enum(&self.visibility_str)?,
            thread_id:           self.thread_id,
            emotional_valence:   self.emotional_valence_str
                                     .as_deref()
                                     .map(str_to_enum)
                                     .transpose()?,
            emotional_intensity: self.emotional_intensity,
            created_at:  DateTime::parse_from_rfc3339(&self.created_at_str)?.with_timezone(&Utc),
            updated_at:  DateTime::parse_from_rfc3339(&self.updated_at_str)?.with_timezone(&Utc),
            access_count:        self.access_count as u32,
            access_times:        serde_json::from_str(&self.access_times_json)?,
            strength:            StrengthState {
                stability:   self.fsrs_stability,
                difficulty:  self.fsrs_difficulty,
                last_review: self.fsrs_last_review_str
                                 .as_deref()
                                 .map(|s| {
                                     DateTime::parse_from_rfc3339(s)
                                         .map(|dt| dt.with_timezone(&Utc))
                                 })
                                 .transpose()?,
            },
            metadata:            serde_json::from_str(&self.metadata_json)?,
        })
    }
}

struct RawLinkRow {
    source_id:      String,
    target_id:      String,
    link_type_str:  String,
    weight:         f32,
    created_at_str: String,
    last_traversed: Option<String>,
    traversal_count: i64,
}

fn row_to_raw_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawLinkRow> {
    Ok(RawLinkRow {
        source_id:      row.get(0)?,
        target_id:      row.get(1)?,
        link_type_str:  row.get(2)?,
        weight:        (row.get::<_, f64>(3)? as f32),
        created_at_str: row.get(4)?,
        last_traversed: row.get(5)?,
        traversal_count: row.get(6)?,
    })
}

impl RawLinkRow {
    fn into_link(self) -> Result<AssociativeLink> {
        let link_type: LinkType = str_to_enum(&self.link_type_str)?;
        let created_at = DateTime::parse_from_rfc3339(&self.created_at_str)?.with_timezone(&Utc);
        let last_traversed = self.last_traversed
            .as_deref()
            .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()?;
        let mut link = AssociativeLink::new(
            MemoryId(self.source_id),
            MemoryId(self.target_id),
            link_type,
            self.weight,
        );
        link.created_at     = created_at;
        link.last_traversed = last_traversed;
        link.traversal_count = self.traversal_count as u32;
        Ok(link)
    }
}

// Column order used in all memory SELECT queries
const SELECT_COLS: &str =
    "id, content, memory_type, layer, salience, tags, agent_id, visibility, \
     thread_id, emotional_valence, emotional_intensity, created_at, updated_at, \
     access_count, access_times, fsrs_stability, fsrs_difficulty, fsrs_last_review, metadata";

impl SqliteStore {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Register sqlite-vec before opening so the extension is available on this connection.
        register_sqlite_vec();
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        // Base schema (no vec0 dependency)
        conn.execute_batch(SCHEMA_SQL)?;

        // Try to create the vec0 virtual table; works only if sqlite-vec loaded successfully.
        let vec_available = conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_vectors USING vec0(embedding float[384]);"
        ).is_ok();
        if vec_available {
            tracing::info!("sqlite-vec loaded — vector search enabled");
        } else {
            tracing::warn!("vec0 table init failed — falling back to FTS5 keyword search");
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            vec_available,
        })
    }

    /// Clone the shared connection Arc — used by VectorStore and GraphStore.
    pub fn shared_conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    // -----------------------------------------------------------------------
    // Memory CRUD
    // -----------------------------------------------------------------------

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
                enum_to_str(&node.memory_type)?,
                enum_to_str(&node.layer)?,
                node.salience as f64,
                serde_json::to_string(&node.tags)?,
                node.agent_id.as_ref().map(|a| &a.0),
                enum_to_str(&node.visibility)?,
                node.thread_id,
                node.emotional_valence.as_ref().map(|v| enum_to_str(v).unwrap()),
                node.emotional_intensity as f64,
                node.created_at.to_rfc3339(),
                node.updated_at.to_rfc3339(),
                node.access_count as i64,
                serde_json::to_string(&node.access_times)?,
                node.strength.stability as f64,
                node.strength.difficulty as f64,
                node.strength.last_review.map(|t| t.to_rfc3339()),
                serde_json::to_string(&node.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub async fn get_memory(&self, id: &MemoryId, scope: &VisibilityScope) -> Result<Option<MemoryNode>> {
        let conn = self.conn.lock().await;
        let (scope_sql, scope_params) = scope.sql_filter();
        let sql = format!(
            "SELECT {SELECT_COLS} FROM memories \
             WHERE id = ? AND {scope_sql} AND deleted_at IS NULL"
        );
        let id_str = id.0.clone();
        let mut dyn_params: Vec<&dyn rusqlite::ToSql> = vec![&id_str];
        for s in &scope_params {
            dyn_params.push(s);
        }
        let mut stmt = conn.prepare(&sql)?;
        let result = stmt.query_row(dyn_params.as_slice(), row_to_raw);
        match result {
            Ok(raw) => Ok(Some(raw.into_memory_node()?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Soft-delete a memory. Returns true if the memory existed and was deleted.
    pub async fn delete_memory(&self, id: &MemoryId) -> Result<bool> {
        let conn = self.conn.lock().await;
        let changed = conn.execute(
            "UPDATE memories SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![Utc::now().to_rfc3339(), id.0],
        )?;
        Ok(changed > 0)
    }

    pub async fn update_memory(&self, node: &MemoryNode) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE memories SET \
             content=?2, memory_type=?3, layer=?4, salience=?5, tags=?6, agent_id=?7, \
             visibility=?8, thread_id=?9, emotional_valence=?10, emotional_intensity=?11, \
             updated_at=?12, access_count=?13, access_times=?14, \
             fsrs_stability=?15, fsrs_difficulty=?16, fsrs_last_review=?17, metadata=?18 \
             WHERE id=?1 AND deleted_at IS NULL",
            params![
                node.id.0,
                node.content,
                enum_to_str(&node.memory_type)?,
                enum_to_str(&node.layer)?,
                node.salience as f64,
                serde_json::to_string(&node.tags)?,
                node.agent_id.as_ref().map(|a| &a.0),
                enum_to_str(&node.visibility)?,
                node.thread_id,
                node.emotional_valence.as_ref().map(|v| enum_to_str(v).unwrap()),
                node.emotional_intensity as f64,
                Utc::now().to_rfc3339(),
                node.access_count as i64,
                serde_json::to_string(&node.access_times)?,
                node.strength.stability as f64,
                node.strength.difficulty as f64,
                node.strength.last_review.map(|t| t.to_rfc3339()),
                serde_json::to_string(&node.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub async fn list_memories_scoped(
        &self,
        scope: &VisibilityScope,
        filter: &ListFilter,
    ) -> Result<Vec<MemoryNode>> {
        let conn = self.conn.lock().await;
        let (scope_sql, scope_params) = scope.sql_filter();

        let type_str: Option<String> = filter.memory_type
            .as_ref()
            .map(enum_to_str)
            .transpose()?;

        let deleted_clause = if filter.include_deleted { "" } else { "AND deleted_at IS NULL" };
        let type_clause    = if type_str.is_some() { "AND memory_type = ?" } else { "" };

        let sql = format!(
            "SELECT {SELECT_COLS} FROM memories \
             WHERE {scope_sql} {deleted_clause} {type_clause} \
             ORDER BY salience DESC, created_at DESC \
             LIMIT ? OFFSET ?"
        );

        let limit_val  = filter.limit  as i64;
        let offset_val = filter.offset as i64;

        let mut dyn_params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for s in &scope_params { dyn_params.push(s); }
        if let Some(ref ts) = type_str { dyn_params.push(ts); }
        dyn_params.push(&limit_val);
        dyn_params.push(&offset_val);

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(dyn_params.as_slice(), row_to_raw)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?.into_memory_node()?);
        }
        Ok(results)
    }

    // -----------------------------------------------------------------------
    // Link CRUD
    // -----------------------------------------------------------------------

    pub async fn insert_link(&self, link: &AssociativeLink) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO links \
             (source_id, target_id, link_type, weight, created_at, last_traversed, traversal_count) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                link.source_id.0,
                link.target_id.0,
                enum_to_str(&link.link_type)?,
                link.weight as f64,
                link.created_at.to_rfc3339(),
                link.last_traversed.map(|t| t.to_rfc3339()),
                link.traversal_count as i64,
            ],
        )?;
        Ok(())
    }

    pub async fn list_links_from(&self, id: &MemoryId) -> Result<Vec<AssociativeLink>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, link_type, weight, created_at, last_traversed, traversal_count \
             FROM links WHERE source_id = ?"
        )?;
        let rows = stmt.query_map(params![id.0], row_to_raw_link)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?.into_link()?);
        }
        Ok(results)
    }

    /// Hard-delete a single memory (use after backup/purge confirmation).
    pub async fn purge_memory(&self, id: &MemoryId) -> Result<bool> {
        let conn = self.conn.lock().await;
        let changed = conn.execute("DELETE FROM memories WHERE id = ?", params![id.0])?;
        Ok(changed > 0)
    }

    /// Hard-delete all soft-deleted memories.
    pub async fn purge_all_deleted(&self) -> Result<usize> {
        let conn = self.conn.lock().await;
        let changed = conn.execute("DELETE FROM memories WHERE deleted_at IS NOT NULL", [])?;
        Ok(changed)
    }

    /// Restore a soft-deleted memory.
    pub async fn restore_memory(&self, id: &MemoryId) -> Result<bool> {
        let conn = self.conn.lock().await;
        let changed = conn.execute(
            "UPDATE memories SET deleted_at = NULL WHERE id = ? AND deleted_at IS NOT NULL",
            params![id.0],
        )?;
        Ok(changed > 0)
    }

    /// Aggregate counts: (total_live, total_deleted, count_per_type as JSON).
    pub async fn memory_stats(&self) -> Result<serde_json::Value> {
        let conn = self.conn.lock().await;
        let live: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE deleted_at IS NULL", [], |r| r.get(0))?;
        let deleted: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE deleted_at IS NOT NULL", [], |r| r.get(0))?;
        let links: i64 = conn.query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))?;

        let mut by_type: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        let mut stmt = conn.prepare(
            "SELECT memory_type, COUNT(*) FROM memories WHERE deleted_at IS NULL GROUP BY memory_type"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (t, count) = row?;
            by_type.insert(t, serde_json::Value::Number(count.into()));
        }

        Ok(serde_json::json!({
            "total_memories": live,
            "deleted_memories": deleted,
            "total_links": links,
            "by_type": by_type,
        }))
    }

    /// Bulk-load memories by ID list — used by the recall pipeline.
    pub async fn get_memories_by_ids(&self, ids: &[MemoryId], scope: &VisibilityScope) -> Result<Vec<MemoryNode>> {
        if ids.is_empty() { return Ok(vec![]); }
        let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let (scope_sql, scope_params) = scope.sql_filter();
        let sql = format!(
            "SELECT {SELECT_COLS} FROM memories \
             WHERE id IN ({placeholders}) AND {scope_sql} AND deleted_at IS NULL"
        );
        let conn = self.conn.lock().await;
        let id_strs: Vec<&str> = ids.iter().map(|id| id.0.as_str()).collect();
        let mut dyn_params: Vec<&dyn rusqlite::ToSql> =
            id_strs.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        for s in &scope_params { dyn_params.push(s); }
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(dyn_params.as_slice(), row_to_raw)?;
        let mut results = Vec::new();
        for row in rows { results.push(row?.into_memory_node()?); }
        Ok(results)
    }

    /// All non-deleted memory IDs — used by GraphStore::rebuild_from_db.
    pub async fn list_all_memory_ids(&self) -> Result<Vec<MemoryId>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id FROM memories WHERE deleted_at IS NULL"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(MemoryId(row?));
        }
        Ok(ids)
    }

    /// All links whose both endpoints are non-deleted memories — for graph rebuild.
    pub async fn list_all_links(&self) -> Result<Vec<AssociativeLink>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT l.source_id, l.target_id, l.link_type, l.weight, \
                    l.created_at, l.last_traversed, l.traversal_count \
             FROM links l \
             JOIN memories ms ON ms.id = l.source_id AND ms.deleted_at IS NULL \
             JOIN memories mt ON mt.id = l.target_id AND mt.deleted_at IS NULL"
        )?;
        let rows = stmt.query_map([], row_to_raw_link)?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?.into_link()?);
        }
        Ok(results)
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
