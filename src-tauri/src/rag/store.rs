// AUTHR_OS — RAG persistence (Phase 5, Sprint 12).
//
// `rag_chunks` stores one embedding per chunk (f32 little-endian BLOB);
// `rag_chunks_fts` mirrors the content for BM25 retrieval. Both are created
// idempotently by the schema migration, so Phase-1 bundles upgrade in place.

use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

/// Additive migration (contract-frozen core DDL is untouched).
pub const RAG_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS rag_chunks (
    chunk_id   TEXT PRIMARY KEY,        -- `<source>:<ref_id>`
    book_id    INTEGER,
    source     TEXT NOT NULL,           -- lorebook | characters | chapters
    ref_id     TEXT NOT NULL,           -- lore key / character_key / segment_id
    content    TEXT NOT NULL,
    dim        INTEGER NOT NULL,
    embedding  BLOB NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(source, ref_id)
);
CREATE INDEX IF NOT EXISTS idx_rag_chunks_source ON rag_chunks(source);
CREATE INDEX IF NOT EXISTS idx_rag_chunks_book ON rag_chunks(book_id);

CREATE VIRTUAL TABLE IF NOT EXISTS rag_chunks_fts USING fts5(
    chunk_id UNINDEXED,
    content,
    tokenize = 'porter unicode61'
);
"#;

/// Apply the RAG migration (safe to call on every connection).
pub fn ensure_rag_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(RAG_SCHEMA_SQL)
        .map_err(|e| AppError::storage(format!("rag schema: {e}")))
}

/// Insert or replace a chunk (content + embedding), keeping FTS in sync.
pub fn upsert_chunk(
    conn: &Connection,
    source: &str,
    ref_id: &str,
    book_id: Option<i64>,
    content: &str,
    dim: usize,
    embedding: &[u8],
) -> Result<(), AppError> {
    let chunk_id = format!("{source}:{ref_id}");
    conn.execute(
        "INSERT INTO rag_chunks(chunk_id, book_id, source, ref_id, content, dim, embedding)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(chunk_id) DO UPDATE SET
            book_id = excluded.book_id,
            content = excluded.content,
            dim = excluded.dim,
            embedding = excluded.embedding",
        rusqlite::params![chunk_id, book_id, source, ref_id, content, dim as i64, embedding],
    )?;
    // FTS sync (delete-then-insert keeps it idempotent).
    conn.execute("DELETE FROM rag_chunks_fts WHERE chunk_id = ?1", [chunk_id.as_str()])?;
    conn.execute(
        "INSERT INTO rag_chunks_fts(chunk_id, content) VALUES (?1, ?2)",
        [chunk_id.as_str(), content],
    )?;
    Ok(())
}

pub struct ChunkRow {
    pub chunk_id: String,
    pub source: String,
    pub ref_id: String,
    pub content: String,
    pub book_id: Option<i64>,
    pub dim: usize,
    pub embedding: Vec<u8>,
}

/// Stream every chunk (ordered for deterministic scan → deterministic ranks).
pub fn all_chunks(conn: &Connection) -> Result<Vec<ChunkRow>, AppError> {
    let mut stmt = conn
        .prepare("SELECT chunk_id, source, ref_id, content, book_id, dim, embedding FROM rag_chunks ORDER BY chunk_id")
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ChunkRow {
                chunk_id: r.get(0)?,
                source: r.get(1)?,
                ref_id: r.get(2)?,
                content: r.get(3)?,
                book_id: r.get(4)?,
                dim: r.get::<_, i64>(5)? as usize,
                embedding: r.get(6)?,
            })
        })
        .map_err(AppError::from)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(AppError::from)?);
    }
    Ok(out)
}

pub fn count(conn: &Connection) -> Result<i64, AppError> {
    conn.query_row("SELECT COUNT(*) FROM rag_chunks", [], |r| r.get(0))
        .map_err(AppError::from)
}

pub fn count_by_source(conn: &Connection) -> Result<Value, AppError> {
    let mut stmt = conn
        .prepare("SELECT source, COUNT(*) FROM rag_chunks GROUP BY source ORDER BY source")
        .map_err(AppError::from)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(AppError::from)?;
    let mut m = serde_json::Map::new();
    for row in rows {
        let (src, n) = row.map_err(AppError::from)?;
        m.insert(src, json!(n));
    }
    Ok(Value::Object(m))
}

/// Remove every chunk of one source (re-index prerequisite).
pub fn clear_source(conn: &Connection, source: &str) -> Result<usize, AppError> {
    conn.execute("DELETE FROM rag_chunks_fts WHERE chunk_id LIKE ?1", [format!("{source}:%")])
        .map_err(AppError::from)?;
    conn.execute("DELETE FROM rag_chunks WHERE source = ?1", [source])
        .map_err(AppError::from)
}

/// FTS5 BM25 search over the chunk corpus. Returns (chunk_id, bm25_rank)
/// ordered best-first (bm25() is negative-scaled: smaller = better).
pub fn fts_search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<(String, f64)>, AppError> {
    // Sanitize into a prefix-friendly OR query; FTS5 MATCH syntax errors map
    // to AI_ERROR with the reason (tier-2 boundary tests assert this).
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let match_q = terms.join(" OR ");
    let sql = format!(
        "SELECT chunk_id, bm25(rag_chunks_fts) AS rank
         FROM rag_chunks_fts
         WHERE rag_chunks_fts MATCH '{match_q}'
         ORDER BY rank LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| AppError::ai(format!("fts query: {e}")))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
        .map_err(|e| AppError::ai(format!("fts search: {e}")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| AppError::ai(format!("fts row: {e}")))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_rag_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn upsert_is_idempotent_and_fts_synced() {
        let conn = mem();
        upsert_chunk(&conn, "lorebook", "magic_rules", None, "dragons fear cold iron", 4, &[0, 0, 128, 63]).unwrap();
        upsert_chunk(&conn, "lorebook", "magic_rules", None, "dragons fear cold iron and fire", 4, &[0, 0, 0, 64]).unwrap();
        assert_eq!(count(&conn).unwrap(), 1);
        let hits = fts_search(&conn, "dragons", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "lorebook:magic_rules");
    }

    #[test]
    fn fts_ranks_and_limits() {
        let conn = mem();
        upsert_chunk(&conn, "chapters", "seg-1", None, "the lighthouse beams across the bay", 2, &[0; 8]).unwrap();
        upsert_chunk(&conn, "chapters", "seg-2", None, "lighthouse keeper's lighthouse log", 2, &[0; 8]).unwrap();
        upsert_chunk(&conn, "chapters", "seg-3", None, "an unrelated passage about bread", 2, &[0; 8]).unwrap();
        let hits = fts_search(&conn, "lighthouse", 2).unwrap();
        assert_eq!(hits.len(), 2);
        // seg-2 mentions lighthouse twice → better (lower) bm25 rank.
        assert!(hits[0].1 <= hits[1].1);
        assert_eq!(hits[0].0, "chapters:seg-2");
    }

    #[test]
    fn clear_source_wipes_fts_too() {
        let conn = mem();
        upsert_chunk(&conn, "characters", "KIRA", None, "protagonist bio", 2, &[0; 8]).unwrap();
        upsert_chunk(&conn, "lorebook", "maps", None, "atlas of the realms", 2, &[0; 8]).unwrap();
        assert_eq!(clear_source(&conn, "characters").unwrap(), 1);
        assert_eq!(count(&conn).unwrap(), 1);
        assert!(fts_search(&conn, "protagonist", 5).unwrap().is_empty());
        assert_eq!(fts_search(&conn, "atlas", 5).unwrap().len(), 1);
    }

    #[test]
    fn fts_hostile_input_is_sanitized_not_erroring() {
        let conn = mem();
        upsert_chunk(&conn, "lorebook", "safe", None, "just calm words here", 2, &[0; 8]).unwrap();
        // Quotes / MATCH operators are stripped by the sanitizer, so hostile
        // input can never produce an FTS5 syntax error or an injection.
        for hostile in ["\"unclosed", "dragons OR (", "a AND b NOT c", "*^;--"] {
            let res = fts_search(&conn, hostile, 5);
            assert!(res.is_ok(), "hostile `{hostile}` errored: {res:?}");
        }
        assert_eq!(fts_search(&conn, "calm", 5).unwrap().len(), 1);
    }
}
