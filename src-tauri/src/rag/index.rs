// AUTHR_OS — RAG indexing pipelines (Phase 5, Sprint 12).
//
// Sources (each chunk = one natural row):
//   lorebook    → one chunk per lore entry   (ref_id = key)
//   characters  → one chunk per profile      (ref_id = character_key)
//   chapters    → one chunk per AST segment  (ref_id = segment_id)
// Re-indexing a source is atomic in effect: clear-then-insert per source.

use crate::error::AppError;
use rayon::prelude::*;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::time::Instant;

use super::embed::{pack, Embedder};

pub const SOURCES: &[&str] = &["lorebook", "characters", "chapters"];

/// One `character_profiles` row as read by `index_characters`.
type CharacterProfileRow = (String, String, String, String, String, String, String, String);

fn index_lorebook(conn: &Connection, emb: &dyn Embedder) -> Result<usize, AppError> {
    let mut stmt = conn
        .prepare("SELECT key, category, content FROM lorebook ORDER BY key")
        .map_err(AppError::from)?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(AppError::from)?
        .collect::<Result<_, _>>()
        .map_err(AppError::from)?;
    // Workerised (ADR-0008): embed in parallel (Embedder: Send + Sync, pure),
    // then upsert sequentially in the original ORDER BY key order — the
    // chunk insertion order stays deterministic.
    let items: Vec<(String, String)> = rows
        .into_iter()
        .map(|(key, category, content)| {
            let text = format!("[{category}] {key}: {content}");
            (key, text)
        })
        .collect();
    let vectors: Vec<Vec<f32>> = items.par_iter().map(|(_, text)| emb.embed(text)).collect();
    let n = items.len();
    for ((key, text), vec) in items.into_iter().zip(vectors) {
        super::store::upsert_chunk(conn, "lorebook", &key, None, &text, emb.dim(), &pack(&vec))?;
    }
    Ok(n)
}

fn index_characters(conn: &Connection, emb: &dyn Embedder) -> Result<usize, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT character_key, display_name, role, bio, ipa_phonetic, voice_engine, voice_id, cadence_notes
             FROM character_profiles ORDER BY character_key",
        )
        .map_err(AppError::from)?;
    let rows: Vec<CharacterProfileRow> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })
        .map_err(AppError::from)?
        .collect::<Result<_, _>>()
        .map_err(AppError::from)?;
    // Workerised (ADR-0008): parallel embed, deterministic sequential upsert.
    let items: Vec<(String, String)> = rows
        .into_iter()
        .map(|(key, display, role, bio, ipa, engine, voice_id, cadence)| {
            let text = format!(
                "[voice cast] {display} ({key}) — role {role}; bio: {bio}; voice {engine}/{voice_id}; ipa {ipa}; cadence {cadence}"
            );
            (key, text)
        })
        .collect();
    let vectors: Vec<Vec<f32>> = items.par_iter().map(|(_, text)| emb.embed(text)).collect();
    let n = items.len();
    for ((key, text), vec) in items.into_iter().zip(vectors) {
        super::store::upsert_chunk(conn, "characters", &key, None, &text, emb.dim(), &pack(&vec))?;
    }
    Ok(n)
}

fn index_chapters(conn: &Connection, emb: &dyn Embedder, book_id: Option<i64>) -> Result<usize, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT s.segment_id, s.chapter_id, s.segment_type, s.speaker_id, s.emotion, s.text_content
             FROM chapter_segments s
             JOIN chapters c ON c.id = s.chapter_id
             WHERE (?1 IS NULL OR c.book_id = ?1)
             ORDER BY s.chapter_id, s.sequence_order",
        )
        .map_err(AppError::from)?;
    let rows: Vec<(String, i64, String, String, String, String)> = stmt
        .query_map(rusqlite::params![book_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .map_err(AppError::from)?
        .collect::<Result<_, _>>()
        .map_err(AppError::from)?;
    // Workerised (ADR-0008): parallel embed, deterministic sequential upsert.
    let items: Vec<(String, i64, String)> = rows
        .into_iter()
        .map(|(segment_id, chapter_id, seg_type, speaker, emotion, text)| {
            let content = format!("[{seg_type}|{speaker}|{emotion}] {text}");
            (segment_id, chapter_id, content)
        })
        .collect();
    let vectors: Vec<Vec<f32>> = items.par_iter().map(|(_, _, content)| emb.embed(content)).collect();
    let n = items.len();
    for ((segment_id, chapter_id, content), vec) in items.into_iter().zip(vectors) {
        super::store::upsert_chunk(conn, "chapters", &segment_id, Some(chapter_id), &content, emb.dim(), &pack(&vec))?;
    }
    Ok(n)
}

/// `rag_index` — index the requested sources (default: all three).
pub fn index(conn: &Connection, sources: &[String], book_id: Option<i64>, emb: &dyn Embedder) -> Result<Value, AppError> {
    for s in sources {
        if !SOURCES.contains(&s.as_str()) {
            return Err(AppError::ai(format!(
                "unknown rag source `{s}` (expected lorebook|characters|chapters)"
            )));
        }
    }
    let t0 = Instant::now();
    let mut per_source = serde_json::Map::new();
    for src in sources {
        let n = match src.as_str() {
            "lorebook" => index_lorebook(conn, emb)?,
            "characters" => index_characters(conn, emb)?,
            "chapters" => index_chapters(conn, emb, book_id)?,
            _ => unreachable!(),
        };
        per_source.insert(src.clone(), json!(n));
    }
    Ok(json!({
        "indexed": Value::Object(per_source),
        "total_chunks": super::store::count(conn)?,
        "dim": emb.dim(),
        "embedder": emb.name(),
        "elapsed_ms": t0.elapsed().as_millis() as u64,
    }))
}
