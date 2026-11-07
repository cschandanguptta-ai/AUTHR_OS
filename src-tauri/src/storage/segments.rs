// AUTHR_OS — Segment storage: AST node CRUD, FTS5 BM25 search, and the
// `sync_lexical_ast_mutation` transactional bridge (build plan §6.4).

use super::db;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

fn next_sequence_order(conn: &Connection, chapter_id: i64) -> Result<i64, AppError> {
    conn.query_row(
        "SELECT COALESCE(MAX(sequence_order) + 1, 0) FROM chapter_segments WHERE chapter_id = ?1",
        rusqlite::params![chapter_id],
        |r| r.get(0),
    )
    .map_err(|e| AppError::storage(e.to_string()))
}

pub fn append(ctx: &mut crate::ipc::Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = crate::ipc::args::req_i64(a, "chapter_id")?;
    let seg_type = crate::ipc::args::req_str(a, "segment_type")?;
    let speaker = crate::ipc::args::req_str(a, "speaker_id")?;
    let text = crate::ipc::args::req_str(a, "text_content")?;
    let emotion = crate::ipc::args::get_str(a, "emotion").unwrap_or_else(|| "neutral".into());
    let rate = crate::ipc::args::get_f64(a, "prosody_rate").unwrap_or(1.0);
    let pitch = crate::ipc::args::get_f64(a, "prosody_pitch").unwrap_or(1.0);
    let ambient = crate::ipc::args::get_str_opt(a, "ambient_cue");
    let sfx = crate::ipc::args::get_str_opt(a, "sfx_cue");
    let seq = match crate::ipc::args::get_i64(a, "sequence_order") {
        Some(s) => s,
        None => next_sequence_order(&ctx.conn, chapter_id)?,
    };
    let segment_id = uuid::Uuid::new_v4().to_string();

    ctx.conn.execute(
        "INSERT INTO chapter_segments(segment_id, chapter_id, sequence_order, segment_type,
                                      speaker_id, emotion, prosody_rate, prosody_pitch,
                                      text_content, ambient_cue, sfx_cue)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![segment_id, chapter_id, seq, seg_type, speaker, emotion,
                          rate, pitch, text, ambient, sfx],
    )?;
    super::chapters::refresh_word_count(&ctx.conn, chapter_id)?;
    let row = get(&ctx.conn, &segment_id)?;
    ctx.after_commit()?; // manifest touch BEFORE shadow/git sync
    crate::shadow::render_chapter(ctx, chapter_id)?;
    crate::storage::git::auto_sync_commit(ctx, chapter_id)?;
    Ok(row)
}

pub fn get(conn: &Connection, segment_id: &str) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM chapter_segments WHERE segment_id = ?1",
        db::SEGMENT_COLS.join(", ")
    );
    conn.query_row(&sql, rusqlite::params![segment_id], |r| db::row_json(r, db::SEGMENT_COLS))
        .map_err(|_| AppError::not_found(format!("segment `{segment_id}` not found")))
}

pub fn list(conn: &Connection, chapter_id: i64) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM chapter_segments WHERE chapter_id = ?1 ORDER BY sequence_order",
        db::SEGMENT_COLS.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![chapter_id], |r| db::row_json(r, db::SEGMENT_COLS))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "segments": rows }))
}

/// FTS5 BM25 search with the porter unicode61 tokenizer.
/// Lesson from Phase 1: MATCH must target the real table name — no aliases.
pub fn search(conn: &Connection, query: &str, limit: i64) -> Result<Value, AppError> {
    let sql = "SELECT s.segment_id, s.chapter_id, s.text_content,
                      bm25(chapter_segments_fts) AS rank
               FROM chapter_segments_fts
               JOIN chapter_segments s ON s.segment_id = chapter_segments_fts.segment_id
               WHERE chapter_segments_fts MATCH ?1
               ORDER BY rank
               LIMIT ?2";
    let mut stmt = conn.prepare(sql)?;
    let hits = stmt
        .query_map(rusqlite::params![query, limit], |r| {
            Ok(json!({
                "segment_id": r.get::<_, String>(0)?,
                "chapter_id": r.get::<_, i64>(1)?,
                "text_content": r.get::<_, String>(2)?,
                "rank": r.get::<_, f64>(3)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "hits": hits }))
}

/// One `BEGIN IMMEDIATE` transaction applying the Lexical AST mutation
/// (§6.4): upserts (update text/order or insert new UUID), deletes,
/// sequence re-index, audio_hash invalidation on changed text only.
pub fn apply_mutation(ctx: &mut crate::ipc::Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = crate::ipc::args::req_i64(a, "chapter_id")?;
    let empty_vec = Vec::new();
    let upserts = a
        .get("upserts")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);
    let deletes = a
        .get("deletes")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    // Chapter must exist.
    super::chapters::get(&ctx.conn, chapter_id)?;

    let tx = ctx
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| AppError::storage(e.to_string()))?;

    let mut applied: i64 = 0;

    for d in deletes {
        let sid = d
            .as_str()
            .ok_or_else(|| AppError::storage("deletes entries must be segment_id strings"))?;
        tx.execute(
            "DELETE FROM chapter_segments WHERE segment_id = ?1 AND chapter_id = ?2",
            rusqlite::params![sid, chapter_id],
        )?;
        applied += 1;
    }

    for u in upserts {
        let sid = u
            .get("segment_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::storage("upserts entries require segment_id"))?;
        let text = u
            .get("text_content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::storage("upserts entries require text_content"))?;
        let seg_type = u
            .get("segment_type")
            .and_then(|v| v.as_str())
            .unwrap_or("NARRATION");
        let speaker = u
            .get("speaker_id")
            .and_then(|v| v.as_str())
            .unwrap_or("NARRATOR");
        let emotion = u.get("emotion").and_then(|v| v.as_str()).unwrap_or("neutral");
        let rate = u.get("prosody_rate").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let pitch = u.get("prosody_pitch").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let ambient = u.get("ambient_cue").and_then(|v| v.as_str());
        let sfx = u.get("sfx_cue").and_then(|v| v.as_str());
        let seq = u.get("sequence_order").and_then(|v| v.as_i64()).unwrap_or(0);

        let old_text: Option<String> = tx
            .query_row(
                "SELECT text_content FROM chapter_segments WHERE segment_id = ?1",
                rusqlite::params![sid],
                |r| r.get(0),
            )
            .ok();

        if old_text.is_some() {
            // Audio invalidation decided in Rust: SQLite evaluates later SET
            // expressions against new values of earlier assignments, so a
            // `CASE WHEN text_content IS ?2` inside one statement is unsafe.
            let text_changed = old_text.as_deref() != Some(text);
            let audio_sql = if text_changed {
                "audio_hash = NULL, audio_cache_path = NULL,"
            } else {
                "audio_hash = audio_hash, audio_cache_path = audio_cache_path,"
            };
            let sql = format!(
                "UPDATE chapter_segments SET
                   text_content = ?2,
                   sequence_order = ?3,
                   segment_type = ?4,
                   speaker_id = ?5,
                   emotion = ?6,
                   prosody_rate = ?7,
                   prosody_pitch = ?8,
                   ambient_cue = ?9,
                   sfx_cue = ?10,
                   {audio_sql}
                   last_modified_at = strftime('%s','now')
                 WHERE segment_id = ?1"
            );
            let changed = tx.execute(
                &sql,
                rusqlite::params![sid, text, seq, seg_type, speaker, emotion, rate, pitch, ambient, sfx],
            )?;
            applied += changed as i64;
        } else {
            tx.execute(
                "INSERT INTO chapter_segments(segment_id, chapter_id, sequence_order,
                                              segment_type, speaker_id, emotion,
                                              prosody_rate, prosody_pitch, text_content,
                                              ambient_cue, sfx_cue)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![sid, chapter_id, seq, seg_type, speaker, emotion,
                                  rate, pitch, text, ambient, sfx],
            )?;
            applied += 1;
        }
    }

    // Re-index sequence_order 0..n preserving the payload ordering.
    reindex_in_tx(&tx, chapter_id)?;
    tx.commit()
        .map_err(|e| AppError::storage(format!("commit: {e}")))?;

    super::chapters::refresh_word_count(&ctx.conn, chapter_id)?;
    ctx.after_commit()?; // manifest touch BEFORE shadow/git sync
    crate::shadow::render_chapter(ctx, chapter_id)?;
    let commit = crate::storage::git::auto_sync_commit(ctx, chapter_id)?;
    Ok(json!({ "applied": applied, "commit": commit }))
}

/// Renumber `sequence_order` 0..n-1 with a two-phase shift that never trips
/// the `UNIQUE(chapter_id, sequence_order)` constraint mid-update.
fn reindex_in_tx(tx: &rusqlite::Transaction<'_>, chapter_id: i64) -> Result<(), AppError> {
    tx.execute(
        "UPDATE chapter_segments SET sequence_order = sequence_order + 1000000 WHERE chapter_id = ?1",
        rusqlite::params![chapter_id],
    )?;
    tx.execute(
        "UPDATE chapter_segments SET sequence_order = (
            SELECT COUNT(*) FROM chapter_segments s2
            WHERE s2.chapter_id = chapter_segments.chapter_id
              AND (s2.rowid < chapter_segments.rowid))
         WHERE chapter_id = ?1",
        rusqlite::params![chapter_id],
    )?;
    Ok(())
}
