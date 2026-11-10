// AUTHR_OS — Embedded Vector RAG (Phase 5 / Sprint 12, build plan §8).
//
// Hybrid retrieval over `project.db`:
//   dense  — sqlite-vec `vec0` virtual table (384-dim float vectors, L2 on
//            unit-normalized embeddings, so ordering equals cosine ordering)
//   sparse — FTS5 BM25 (`porter unicode61`) over a sidecar index
//   fusion — Reciprocal Rank Fusion (RRF, k = 60)
//
// Sidecar artifacts live INSIDE project.db (created lazily by
// `ensure_rag_schema`): `rag_chunks` (documents + embeddings as BLOBs),
// `rag_fts`, `rag_vec`. They are inert rows for any SQLite client without the
// vec module and travel with every `.authr` bundle export.

use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use rusqlite::Connection;
use serde_json::{json, Value};

/// Embedding dimensionality of the local embedder contract (build plan §8:
/// `all-MiniLM-L6-v2` produces 384-dim dense vectors).
pub const RAG_DIM: usize = 384;

pub const SOURCE_LORE: &str = "lore";
pub const SOURCE_CHARACTERS: &str = "characters";
pub const SOURCE_SEGMENTS: &str = "segments";
pub const ALL_SOURCES: &[&str] = &[SOURCE_LORE, SOURCE_CHARACTERS, SOURCE_SEGMENTS];

// ── sqlite-vec runtime registration ──────────────────────────────────────────

/// Register the bundled `sqlite-vec` loadable module as an automatic
/// extension. Must run before ANY connection is opened; `db::open_db` calls
/// this once per process.
pub fn register_vec_module() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        use rusqlite::auto_extension::RawAutoExtension;
        let raw: RawAutoExtension =
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
        // Best effort: every later connection resolves vec0 on success.
        let _ = rusqlite::auto_extension::register_auto_extension(raw);
    });
}

// ── sidecar schema ───────────────────────────────────────────────────────────

/// Lazily create the RAG sidecar tables (idempotent). Returns `true` when the
/// artifacts were created by this call.
pub fn ensure_rag_schema(conn: &Connection) -> Result<bool, AppError> {
    let present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rag_chunks'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
    if present > 0 {
        return Ok(false);
    }
    register_vec_module();
    conn.execute_batch(
        r#"
        -- Document store: one row per indexed chunk, embedding kept as LE f32 BLOB
        -- so foreign clients can always rebuild `rag_vec` from plain data.
        CREATE TABLE rag_chunks (
            chunk_id   TEXT PRIMARY KEY,     -- "<source>:<ref_key>"
            source     TEXT NOT NULL,        -- lore | characters | segments
            ref_key    TEXT NOT NULL,
            chapter_id INTEGER,
            title      TEXT NOT NULL DEFAULT '',
            content    TEXT NOT NULL,
            dim        INTEGER NOT NULL,
            embedding  BLOB NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );
        CREATE INDEX idx_rag_chunks_source ON rag_chunks(source, ref_key);
        CREATE INDEX idx_rag_chunks_chapter ON rag_chunks(chapter_id);

        -- Sparse half: BM25 over title + content.
        CREATE VIRTUAL TABLE rag_fts USING fts5(
            doc_id UNINDEXED,
            title,
            content,
            tokenize = 'porter unicode61'
        );

        -- Dense half: sqlite-vec KNN index.
        CREATE VIRTUAL TABLE rag_vec USING vec0(embedding float[384]);
        "#,
    )
    .map_err(|e| AppError::storage(format!("rag schema: {e}")))?;
    Ok(true)
}

fn f32_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Unit-normalize in place so vec0's L2 distance ordering equals cosine.
fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

// ── document collection ──────────────────────────────────────────────────────

struct Doc {
    source: &'static str,
    ref_key: String,
    chapter_id: Option<i64>,
    title: String,
    content: String,
}

fn collect_lore(conn: &Connection) -> Result<Vec<Doc>, AppError> {
    let mut stmt = conn.prepare("SELECT key, content FROM lorebook ORDER BY key")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Doc {
                source: SOURCE_LORE,
                ref_key: r.get::<_, String>(0)?,
                chapter_id: None,
                title: r.get::<_, String>(0)?,
                content: r.get::<_, String>(1)?,
            })
        })
        .map_err(|e| AppError::storage(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn collect_characters(conn: &Connection) -> Result<Vec<Doc>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT character_key, display_name, role, bio FROM character_profiles ORDER BY character_key",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let key: String = r.get(0)?;
            let display: String = r.get(1)?;
            let role: String = r.get(2)?;
            let bio: String = r.get(3)?;
            let content = if bio.trim().is_empty() {
                format!("{display} — {role} in the voice cast.")
            } else {
                format!("{display} ({role}): {bio}")
            };
            Ok(Doc {
                source: SOURCE_CHARACTERS,
                ref_key: key,
                chapter_id: None,
                title: display,
                content,
            })
        })
        .map_err(|e| AppError::storage(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn collect_segments(conn: &Connection) -> Result<Vec<Doc>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT s.segment_id, s.chapter_id, c.chapter_number, c.title, s.text_content
         FROM chapter_segments s
         JOIN chapters c ON c.id = s.chapter_id
         ORDER BY s.chapter_id, s.sequence_order",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let sid: String = r.get(0)?;
            let chapter_id: i64 = r.get(1)?;
            let number: i64 = r.get(2)?;
            let ctitle: String = r.get(3)?;
            let text: String = r.get(4)?;
            Ok(Doc {
                source: SOURCE_SEGMENTS,
                ref_key: sid,
                chapter_id: Some(chapter_id),
                title: format!("ch{number:02} — {ctitle}"),
                content: text,
            })
        })
        .map_err(|e| AppError::storage(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── command bodies ───────────────────────────────────────────────────────────

/// `rag_reindex` — rebuild the hybrid index for the requested sources.
pub fn reindex(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let requested: Vec<String> = match a.get("sources") {
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for it in items {
                let s = it
                    .as_str()
                    .ok_or_else(|| AppError::storage("sources entries must be strings"))?
                    .to_string();
                if !ALL_SOURCES.contains(&s.as_str()) {
                    return Err(AppError::storage(format!(
                        "unknown source `{s}` — valid sources: {}",
                        ALL_SOURCES.join(", ")
                    )));
                }
                out.push(s);
            }
            out
        }
        _ => ALL_SOURCES.iter().map(|s| s.to_string()).collect(),
    };

    ensure_rag_schema(&ctx.conn)?;
    let embedder = crate::ai::embedder::default_embedder()?;
    if embedder.dim() != RAG_DIM {
        return Err(AppError::storage(format!(
            "embedder `{}` produces dim {} — contract requires {RAG_DIM}",
            embedder.name(),
            embedder.dim()
        )));
    }

    // Full rebuild keeps the index homogeneous under one embedder.
    ctx.conn
        .execute_batch("DELETE FROM rag_vec; DELETE FROM rag_fts; DELETE FROM rag_chunks;")
        .map_err(|e| AppError::storage(e.to_string()))?;

    let mut chunks: i64 = 0;
    for source in &requested {
        let docs = match source.as_str() {
            SOURCE_LORE => collect_lore(&ctx.conn)?,
            SOURCE_CHARACTERS => collect_characters(&ctx.conn)?,
            SOURCE_SEGMENTS => collect_segments(&ctx.conn)?,
            _ => unreachable!("validated above"),
        };
        for doc in docs {
            let mut emb = embedder.embed(&doc.content);
            normalize(&mut emb);
            let chunk_id = format!("{}:{}", doc.source, doc.ref_key);
            let blob = f32_blob(&emb);
            ctx.conn
                .execute(
                    "INSERT INTO rag_chunks(chunk_id, source, ref_key, chapter_id, title, content, dim, embedding)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        chunk_id,
                        doc.source,
                        doc.ref_key,
                        doc.chapter_id,
                        doc.title,
                        doc.content,
                        emb.len() as i64,
                        blob
                    ],
                )
                .map_err(|e| AppError::storage(e.to_string()))?;
            let rowid = ctx.conn.last_insert_rowid();
            ctx.conn
                .execute(
                    "INSERT INTO rag_vec(rowid, embedding) VALUES (?1, ?2)",
                    rusqlite::params![rowid, blob],
                )
                .map_err(|e| AppError::storage(e.to_string()))?;
            ctx.conn
                .execute(
                    "INSERT INTO rag_fts(doc_id, title, content) VALUES (?1, ?2, ?3)",
                    rusqlite::params![chunk_id, doc.title, doc.content],
                )
                .map_err(|e| AppError::storage(e.to_string()))?;
            chunks += 1;
        }
    }

    ctx.after_commit()?;
    Ok(json!({
        "indexed": true,
        "chunks": chunks,
        "dim": RAG_DIM,
        "embedder": embedder.name(),
        "sources": requested,
        "reindexed": chunks,
    }))
}

/// `rag_search` — hybrid dense + BM25 retrieval fused with RRF.
pub fn search(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    let t0 = std::time::Instant::now();
    let query = args::req_str(a, "query")?;
    if query.trim().is_empty() {
        return Err(AppError::storage("query must not be empty"));
    }
    let limit = args::get_i64(a, "limit").unwrap_or(8).clamp(1, 50);
    let source_filter: Option<Vec<String>> = match a.get("sources") {
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for it in items {
                let s = it
                    .as_str()
                    .ok_or_else(|| AppError::storage("sources entries must be strings"))?
                    .to_string();
                if !ALL_SOURCES.contains(&s.as_str()) {
                    return Err(AppError::storage(format!(
                        "unknown source `{s}` — valid sources: {}",
                        ALL_SOURCES.join(", ")
                    )));
                }
                out.push(s);
            }
            Some(out)
        }
        _ => None,
    };

    ensure_rag_schema(conn)?;
    let indexed: i64 = conn
        .query_row("SELECT COUNT(*) FROM rag_chunks", [], |r| r.get(0))
        .map_err(|e| AppError::storage(e.to_string()))?;
    if indexed == 0 {
        return Ok(json!({
            "query": query,
            "hits": [],
            "indexed": 0,
            "dim": RAG_DIM,
            "note": "index empty — run rag_reindex first",
        }));
    }

    // Dimensionality guard: a foreign embedder or tampered index must fail
    // loudly instead of corrupting the KNN query.
    let stored_dim: i64 = conn
        .query_row("SELECT MIN(dim) FROM rag_chunks", [], |r| r.get(0))
        .map_err(|e| AppError::storage(e.to_string()))?;
    let embedder = crate::ai::embedder::default_embedder()?;
    if stored_dim != RAG_DIM as i64 || embedder.dim() != RAG_DIM {
        return Err(AppError::storage(format!(
            "embedding dimension mismatch (index dim {stored_dim}, embedder dim {}) — rerun rag_reindex",
            embedder.dim()
        )));
    }

    // ── dense half: sqlite-vec KNN ──────────────────────────────────────────
    let pool = (limit * 3).clamp(8, 64);
    let mut q = embedder.embed(&query);
    normalize(&mut q);
    let qblob = f32_blob(&q);
    let mut dense: Vec<(i64, f64)> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT rowid, distance FROM rag_vec WHERE embedding MATCH ?1 AND k = ?2")
            .map_err(|e| AppError::storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![qblob, pool], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(|e| AppError::storage(e.to_string()))?;
        for row in rows {
            dense.push(row.map_err(|e| AppError::storage(e.to_string()))?);
        }
    }

    // ── sparse half: FTS5 BM25 (OR-joined quoted terms) ─────────────────────
    let mut sparse: Vec<(String, f64)> = Vec::new();
    let fts_q = fts_or_query(&query);
    if !fts_q.is_empty() {
        let sql = format!(
            "SELECT doc_id, bm25(rag_fts) AS rank FROM rag_fts \
             WHERE rag_fts MATCH ?1 ORDER BY rank LIMIT ?2"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params![fts_q, pool], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
            })
            .map_err(|e| AppError::storage(e.to_string()))?;
        for row in rows {
            sparse.push(row.map_err(|e| AppError::storage(e.to_string()))?);
        }
    }

    // ── RRF fusion (k = 60) ─────────────────────────────────────────────────
    const RRF_K: f64 = 60.0;
    #[derive(Default)]
    struct Acc {
        dense_rank: Option<usize>,
        sparse_rank: Option<usize>,
        distance: Option<f64>,
        score: f64,
    }
    let mut fused: std::collections::HashMap<i64, Acc> = std::collections::HashMap::new();
    for (rank, (rowid, dist)) in dense.iter().enumerate() {
        let acc = fused.entry(*rowid).or_default();
        acc.dense_rank = Some(rank + 1);
        acc.distance = Some(*dist);
        acc.score += 1.0 / (RRF_K + (rank + 1) as f64);
    }
    // Sparse rows key on doc_id; map doc_id → chunk rowid.
    for (rank, (doc_id, _bm25)) in sparse.iter().enumerate() {
        let rowid: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM rag_chunks WHERE chunk_id = ?1",
                rusqlite::params![doc_id],
                |r| r.get(0),
            )
            .ok();
        if let Some(rowid) = rowid {
            let acc = fused.entry(rowid).or_default();
            acc.sparse_rank = Some(rank + 1);
            acc.score += 1.0 / (RRF_K + (rank + 1) as f64);
        }
    }
    if let Some(filter) = &source_filter {
        let ids: Vec<i64> = fused.keys().copied().collect();
        for rowid in ids {
            let source: String = conn
                .query_row(
                    "SELECT source FROM rag_chunks WHERE rowid = ?1",
                    rusqlite::params![rowid],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            if !filter.contains(&source) {
                fused.remove(&rowid);
            }
        }
    }

    let mut scored: Vec<(i64, Acc)> = fused.into_iter().collect();
    scored.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit as usize);

    let mut hits = Vec::with_capacity(scored.len());
    for (rowid, acc) in scored {
        let row = conn
            .query_row(
                "SELECT chunk_id, source, ref_key, chapter_id, title, content
                 FROM rag_chunks WHERE rowid = ?1",
                rusqlite::params![rowid],
                |r| {
                    Ok(json!({
                        "chunk_id": r.get::<_, String>(0)?,
                        "source": r.get::<_, String>(1)?,
                        "ref_key": r.get::<_, String>(2)?,
                        "chapter_id": r.get::<_, Option<i64>>(3)?,
                        "title": r.get::<_, String>(4)?,
                        "content": snippet(&r.get::<_, String>(5)?, 240),
                        "score": acc.score,
                        "dense_rank": acc.dense_rank,
                        "sparse_rank": acc.sparse_rank,
                        "distance": acc.distance,
                    }))
                },
            )
            .map_err(|e| AppError::storage(e.to_string()))?;
        hits.push(row);
    }

    Ok(json!({
        "query": query,
        "indexed": indexed,
        "dim": RAG_DIM,
        "embedder": embedder.name(),
        "backends": { "dense": "sqlite-vec", "sparse": "fts5-bm25" },
        "fusion": "rrf-k60",
        "took_ms": t0.elapsed().as_millis() as i64,
        "hits": hits,
    }))
}

/// `rag_stats` — index telemetry.
pub fn stats(conn: &Connection) -> Result<Value, AppError> {
    ensure_rag_schema(conn)?;
    let count = |sql: &str| -> Result<i64, AppError> {
        conn.query_row(sql, [], |r| r.get(0))
            .map_err(|e| AppError::storage(e.to_string()))
    };
    let indexed = count("SELECT COUNT(*) FROM rag_chunks")?;
    let lore = count(&format!("SELECT COUNT(*) FROM rag_chunks WHERE source = '{SOURCE_LORE}'"))?;
    let chars =
        count(&format!("SELECT COUNT(*) FROM rag_chunks WHERE source = '{SOURCE_CHARACTERS}'"))?;
    let segs =
        count(&format!("SELECT COUNT(*) FROM rag_chunks WHERE source = '{SOURCE_SEGMENTS}'"))?;
    let embedder = crate::ai::embedder::default_embedder()?;
    let vec_version: String = conn
        .query_row("SELECT vec_version()", [], |r| r.get(0))
        .unwrap_or_else(|_| "unavailable".into());
    Ok(json!({
        "indexed": indexed,
        "dim": RAG_DIM,
        "embedder": embedder.name(),
        "vec_backend": format!("sqlite-vec {vec_version}"),
        "fts_backend": "fts5 porter unicode61",
        "fusion": "rrf-k60",
        "sources": {
            SOURCE_LORE: lore,
            SOURCE_CHARACTERS: chars,
            SOURCE_SEGMENTS: segs,
        },
    }))
}

// ── invalidation hooks (called by the entity delete paths) ───────────────────

/// Drop one document chunk from all three sidecar tables.
pub fn invalidate(conn: &Connection, source: &str, ref_key: &str) -> Result<(), AppError> {
    let chunk_id = format!("{source}:{ref_key}");
    ensure_rag_schema(conn)?;
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| AppError::storage(e.to_string()))?;
    let res = (|| {
        conn.execute("DELETE FROM rag_vec WHERE rowid IN (SELECT rowid FROM rag_chunks WHERE chunk_id = ?1)", rusqlite::params![chunk_id])
            .map_err(|e| AppError::storage(e.to_string()))?;
        conn.execute("DELETE FROM rag_fts WHERE doc_id = ?1", rusqlite::params![chunk_id])
            .map_err(|e| AppError::storage(e.to_string()))?;
        conn.execute("DELETE FROM rag_chunks WHERE chunk_id = ?1", rusqlite::params![chunk_id])
            .map_err(|e| AppError::storage(e.to_string()))?;
        Ok(())
    })();
    match res {
        Ok(()) => conn.execute_batch("COMMIT;").map_err(|e| AppError::storage(e.to_string())),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

/// Drop every segment chunk belonging to one chapter (chapter cascade).
pub fn invalidate_chapter(conn: &Connection, chapter_id: i64) -> Result<(), AppError> {
    ensure_rag_schema(conn)?;
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| AppError::storage(e.to_string()))?;
    let res = (|| {
        conn.execute(
            "DELETE FROM rag_vec WHERE rowid IN (
                SELECT rowid FROM rag_chunks WHERE chapter_id = ?1)",
            rusqlite::params![chapter_id],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM rag_fts WHERE doc_id IN (
                SELECT chunk_id FROM rag_chunks WHERE chapter_id = ?1)",
            rusqlite::params![chapter_id],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM rag_chunks WHERE chapter_id = ?1",
            rusqlite::params![chapter_id],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
        Ok(())
    })();
    match res {
        Ok(()) => conn.execute_batch("COMMIT;").map_err(|e| AppError::storage(e.to_string())),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

/// Reconcile sidecars with the authoritative row set (clean_sweep support):
/// drop chunks whose backing entity no longer exists. Returns rows removed.
pub fn prune_orphans(conn: &Connection) -> Result<i64, AppError> {
    ensure_rag_schema(conn)?;
    let stale_sql = "SELECT rowid FROM rag_chunks r WHERE
        (r.source = 'segments'   AND r.ref_key NOT IN (SELECT segment_id FROM chapter_segments))
     OR (r.source = 'lore'       AND r.ref_key NOT IN (SELECT key FROM lorebook))
     OR (r.source = 'characters' AND r.ref_key NOT IN (SELECT character_key FROM character_profiles))";
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| AppError::storage(e.to_string()))?;
    let res = (|| {
        conn.execute(
            "DELETE FROM rag_vec WHERE rowid IN (SELECT rowid FROM rag_chunks r WHERE
                (r.source = 'segments'   AND r.ref_key NOT IN (SELECT segment_id FROM chapter_segments))
             OR (r.source = 'lore'       AND r.ref_key NOT IN (SELECT key FROM lorebook))
             OR (r.source = 'characters' AND r.ref_key NOT IN (SELECT character_key FROM character_profiles)))",
            [],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
        conn.execute(
            "DELETE FROM rag_fts WHERE doc_id IN (SELECT chunk_id FROM rag_chunks r WHERE
                (r.source = 'segments'   AND r.ref_key NOT IN (SELECT segment_id FROM chapter_segments))
             OR (r.source = 'lore'       AND r.ref_key NOT IN (SELECT key FROM lorebook))
             OR (r.source = 'characters' AND r.ref_key NOT IN (SELECT character_key FROM character_profiles)))",
            [],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
        let n = conn
            .execute(&format!("DELETE FROM rag_chunks WHERE rowid IN ({stale_sql})"), [])
            .map_err(|e| AppError::storage(e.to_string()))?;
        Ok(n)
    })();
    match res {
        Ok(n) => conn
            .execute_batch("COMMIT;")
            .map(|_| n as i64)
            .map_err(|e| AppError::storage(e.to_string())),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────


/// Quote each alphanumeric term and OR-join — any-term BM25 matching that is
/// immune to FTS operator injection (Phase 1 lesson: MATCH needs the real
/// table name; this keeps the query string inert otherwise).
fn fts_or_query(input: &str) -> String {
    const MAX_TERMS: usize = 24;
    let mut terms: Vec<String> = Vec::new();
    for raw in input.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        let t: String = raw
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if t.is_empty() {
            continue;
        }
        terms.push(format!("\"{t}\""));
        if terms.len() >= MAX_TERMS {
            break;
        }
    }
    terms.join(" OR ")
}

fn snippet(content: &str, max_chars: usize) -> String {
    let mut out: String = content
        .chars()
        .take(max_chars)
        .collect();
    if content.chars().count() > max_chars {
        out.push('…');
    }
    out
}
