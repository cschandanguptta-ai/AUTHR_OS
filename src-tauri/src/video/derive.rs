// AUTHR_OS — Draft-time media derivation (contract v1.5, ADR-0011).
//
// THE "NO DOUBLE WORK" CONTRACT: everything the audio, video and image
// pipelines need from the manuscript is derived deterministically, ONCE, at
// the moment the words are written — not re-analyzed later by an LLM or by
// each downstream stage.
//
//   * `refresh_chapter_media` runs on every draft write path
//     (`segment_append`, `sync_lexical_ast_mutation`) and rewrites the
//     canonical `video_scenes` rows: scene boundaries (one per narration
//     segment — novelcast's "one illustration per scene"), per-scene
//     duration (stored `audio_duration_ms` when synthesized, else the
//     canonical `audio::scheduler::estimate_duration_ms` — the same single
//     estimator the audio pipeline uses), mentioned characters (deterministic
//     name scan, ≤3 refs — the novelcast reference ladder cap) and focal
//     defaults.
//   * The derivation is idempotent: when the computed state already matches
//     what is stored, nothing is written (`refreshed:false`), so the draft
//     hook is O(1)-ish on no-op saves and the stage entry-points can call it
//     as a cheap catch-up without redoing work.
//   * `chapter_delete` purges the rows (no orphan scenes).
//
// Errors here never block a draft write — the hook is best-effort and the
// stage entry-points re-run it as catch-up (documented in ADR-0011).

use crate::audio::scheduler::estimate_duration_ms;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

/// Additive storage (same pattern as `chapter_retention` — eager, idempotent,
/// no frozen DDL touched). `video_assets.chapter_number = 0` is the book-level
/// sentinel for character reference sheets (novelcast: one sheet per character
/// per book, `char_{id}.png`).
pub fn ensure_video_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS video_scenes (
            chapter_id INTEGER NOT NULL,
            book_id INTEGER NOT NULL,
            scene_index INTEGER NOT NULL,
            segment_id TEXT NOT NULL,
            sequence_order INTEGER NOT NULL,
            speaker_id TEXT NOT NULL,
            text_chars INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL,
            characters_json TEXT NOT NULL DEFAULT '[]',
            focal TEXT NOT NULL DEFAULT 'center',
            audio_cache_path TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (chapter_id, scene_index)
        );
        CREATE INDEX IF NOT EXISTS idx_video_scenes_book ON video_scenes(book_id);
        CREATE TABLE IF NOT EXISTS video_assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_id INTEGER NOT NULL,
            chapter_number INTEGER NOT NULL,
            kind TEXT NOT NULL,
            ref_key TEXT NOT NULL,
            engine TEXT NOT NULL,
            path TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            bytes INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(book_id, chapter_number, kind, ref_key)
        );
        CREATE INDEX IF NOT EXISTS idx_video_assets_book ON video_assets(book_id);",
    )
    .map_err(|e| AppError::storage(format!("video schema: {e}")))?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneRow {
    pub scene_index: usize,
    pub segment_id: String,
    pub sequence_order: i64,
    pub speaker_id: String,
    pub text_chars: usize,
    pub duration_ms: u64,
    pub characters: Vec<String>,
    pub focal: String,
    pub audio_cache_path: String,
}

/// Deterministic single-pass derivation for one chapter. Returns the derived
/// summary; `refreshed` is false when the stored rows already match (no-op).
pub fn refresh_chapter_media(conn: &Connection, chapter_id: i64) -> Result<Value, AppError> {
    ensure_video_schema(conn)?;

    // Chapter must exist; pull book linkage.
    let (book_id, chapter_number): (i64, i64) = conn
        .query_row(
            "SELECT book_id, chapter_number FROM chapters WHERE id = ?1",
            [chapter_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| AppError::not_found(format!("chapter {chapter_id} does not exist")))?;

    // Narration segments in sequence order.
    let mut stmt = conn.prepare(
        "SELECT segment_id, sequence_order, speaker_id, text_content,
                audio_duration_ms, COALESCE(audio_cache_path, '')
         FROM chapter_segments WHERE chapter_id = ?1 ORDER BY sequence_order",
    )?;
    let segs = stmt
        .query_map([chapter_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)? as u64,
                r.get::<_, String>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Cast for the deterministic mention scan (novelcast ladder cap: 3 refs).
    let mut cstmt = conn.prepare(
        "SELECT character_key, display_name, role FROM character_profiles",
    )?;
    let cast = cstmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(cstmt);

    let mut scenes: Vec<SceneRow> = Vec::with_capacity(segs.len());
    let mut durations_filled = 0u64;
    for (i, (seg_id, seq, speaker, text, stored_ms, cache_path)) in segs.iter().enumerate() {
        let dur = if *stored_ms > 0 {
            *stored_ms
        } else {
            durations_filled += 1;
            estimate_duration_ms(text)
        };
        let mut characters = Vec::new();
        if speaker != "NARRATOR" {
            characters.push(speaker.clone());
        }
        let lower = text.to_lowercase();
        for (key, display, _role) in &cast {
            if characters.len() >= 3 {
                break;
            }
            if key == speaker || characters.contains(key) || display.len() < 3 {
                continue;
            }
            // Whole-word, case-insensitive mention scan.
            if lower.split_whitespace().any(|w| {
                w.trim_matches(|c: char| !c.is_alphanumeric())
                    == display.to_lowercase()
            }) {
                characters.push(key.clone());
            }
        }
        scenes.push(SceneRow {
            scene_index: i,
            segment_id: seg_id.clone(),
            sequence_order: *seq,
            speaker_id: speaker.clone(),
            text_chars: text.chars().count(),
            duration_ms: dur,
            characters,
            focal: "center".into(),
            audio_cache_path: cache_path.clone(),
        });
    }

    // Idempotence: skip the write when the stored rows already match.
    // The persisted `focal` is USER state (VideoStudio selector), not derived
    // state — it is inherited by segment identity across re-derives and
    // never reset to the default by bookkeeping.
    let existing = load_rows(conn, chapter_id)?;
    let focal_by_segment: std::collections::HashMap<&str, &str> = existing
        .iter()
        .map(|r| (r.segment_id.as_str(), r.focal.as_str()))
        .collect();
    for s in &mut scenes {
        if let Some(f) = focal_by_segment.get(s.segment_id.as_str()) {
            s.focal = (*f).to_string();
        }
    }
    if existing.len() == scenes.len()
        && existing
            .iter()
            .zip(scenes.iter())
            .all(|(a, b)| {
                a.scene_index == b.scene_index
                    && a.segment_id == b.segment_id
                    && a.duration_ms == b.duration_ms
                    && a.speaker_id == b.speaker_id
                    && a.text_chars == b.text_chars
                    && a.characters == b.characters
                    && a.audio_cache_path == b.audio_cache_path
                    && a.focal == b.focal
            })
    {
        return Ok(json!({
            "chapter_id": chapter_id, "book_id": book_id,
            "chapter_number": chapter_number,
            "scenes": scenes.len(), "durations_filled": 0,
            "refreshed": false,
        }));
    }

    conn.execute("DELETE FROM video_scenes WHERE chapter_id = ?1", [chapter_id])?;
    {
        let mut ins = conn.prepare(
            "INSERT INTO video_scenes(chapter_id, book_id, scene_index, segment_id,
                sequence_order, speaker_id, text_chars, duration_ms, characters_json,
                focal, audio_cache_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;
        for s in &scenes {
            ins.execute(
                rusqlite::params![
                    chapter_id,
                    book_id,
                    s.scene_index as i64,
                    s.segment_id,
                    s.sequence_order,
                    s.speaker_id,
                    s.text_chars as i64,
                    s.duration_ms as i64,
                    serde_json::to_string(&s.characters).unwrap_or_else(|_| "[]".into()),
                    s.focal,
                    s.audio_cache_path,
                ],
            )?;
        }
    }

    Ok(json!({
        "chapter_id": chapter_id, "book_id": book_id,
        "chapter_number": chapter_number,
        "scenes": scenes.len(), "durations_filled": durations_filled,
        "refreshed": true,
    }))
}

fn load_rows(conn: &Connection, chapter_id: i64) -> Result<Vec<SceneRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT scene_index, segment_id, sequence_order, speaker_id, text_chars,
                duration_ms, characters_json, focal, audio_cache_path
         FROM video_scenes WHERE chapter_id = ?1 ORDER BY scene_index",
    )?;
    let rows = stmt
        .query_map([chapter_id], |r| {
            Ok((
                r.get::<_, i64>(0)? as usize,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)? as usize,
                r.get::<_, i64>(5)? as u64,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|(i, seg, seq, spk, chars, dur, cj, focal, path)| SceneRow {
            scene_index: i,
            segment_id: seg,
            sequence_order: seq,
            speaker_id: spk,
            text_chars: chars,
            duration_ms: dur,
            characters: serde_json::from_str(&cj).unwrap_or_default(),
            focal,
            audio_cache_path: path,
        })
        .collect())
}

/// Canonical read path for every downstream stage — plan/render/assemble all
/// consume these rows; nothing re-derives.
pub fn scenes_for(conn: &Connection, chapter_id: i64) -> Result<Vec<SceneRow>, AppError> {
    ensure_video_schema(conn)?;
    let rows = load_rows(conn, chapter_id)?;
    if rows.is_empty() {
        // Draft-time hook missed (pre-v1.5 chapter or a swallowed error):
        // one-shot catch-up so stages never see stale state.
        refresh_chapter_media(conn, chapter_id)?;
        return load_rows(conn, chapter_id);
    }
    Ok(rows)
}

/// Purge on chapter delete (called from `storage::chapters::delete`).
pub fn purge_chapter(conn: &Connection, chapter_id: i64, book_id: i64, chapter_number: i64) -> Result<(), AppError> {
    ensure_video_schema(conn)?;
    conn.execute("DELETE FROM video_scenes WHERE chapter_id = ?1", [chapter_id])?;
    conn.execute(
        "DELETE FROM video_assets WHERE book_id = ?1 AND chapter_number = ?2 AND kind != 'SHEET'",
        rusqlite::params![book_id, chapter_number],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_video_schema(&conn).unwrap();
        ensure_video_schema(&conn).unwrap(); // second call must not fail
    }

    #[test]
    fn refresh_matches_only_when_state_equal() {
        let a = SceneRow {
            scene_index: 0,
            segment_id: "s1".into(),
            sequence_order: 0,
            speaker_id: "NARRATOR".into(),
            text_chars: 12,
            duration_ms: 900,
            characters: vec!["KIRA".into()],
            focal: "center".into(),
            audio_cache_path: String::new(),
        };
        let mut b = a.clone();
        b.duration_ms = 901;
        assert_ne!(a, b);
        assert_eq!(a, a.clone());
    }
}
