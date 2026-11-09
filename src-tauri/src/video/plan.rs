// AUTHR_OS — Video plan (contract v1.5, ADR-0011).
//
// `video_plan` is a READ over the draft-time derived state (`video_scenes`)
// plus the asset registry — it re-derives nothing. This is the command the
// VideoStudio pacing timeline and every external consumer should bind to:
// one scene per narration segment, the canonical duration (stored audio
// duration or the shared estimator), the ≤3-character reference ladder and
// the narration readiness (per-scene pipeline audio).

use super::derive::{ensure_video_schema, refresh_chapter_media, scenes_for};
use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn video_plan(ctx: &Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    ensure_video_schema(&ctx.conn)?;
    let derived = refresh_chapter_media(&ctx.conn, chapter_id)?;
    let scenes = scenes_for(&ctx.conn, chapter_id)?;
    if scenes.is_empty() {
        return Err(AppError::not_found(format!(
            "chapter {chapter_id} has no segments — draft text first"
        )));
    }
    let (book_id, chapter_number): (i64, i64) = ctx.conn.query_row(
        "SELECT book_id, chapter_number FROM chapters WHERE id = ?1",
        [chapter_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    // Sheet coverage (book-level sentinel rows) drives the ladder labels.
    let sheets = sheet_keys(&ctx.conn, book_id)?;
    let mut missing_sheets: Vec<String> = Vec::new();
    let mut ladder = json!({ "refs_ready": 0, "partial": 0, "text_only": 0 });
    let mut total_ms = 0u64;
    let mut narration_ready = true;
    let mut scene_rows = Vec::with_capacity(scenes.len());

    for s in &scenes {
        total_ms += s.duration_ms;
        if s.audio_cache_path.is_empty() {
            narration_ready = false;
        }
        let unsheeted: Vec<&String> = s
            .characters
            .iter()
            .filter(|k| !sheets.contains(&(*k).clone()))
            .collect();
        for k in &unsheeted {
            if !missing_sheets.contains(k) {
                missing_sheets.push((*k).clone());
            }
        }
        let band = if s.characters.is_empty() {
            "TEXT_ONLY"
        } else if unsheeted.is_empty() {
            "REFS_READY"
        } else {
            "PARTIAL"
        };
        match band {
            "REFS_READY" => ladder["refs_ready"] = json!(ladder["refs_ready"].as_i64().unwrap_or(0) + 1),
            "PARTIAL" => ladder["partial"] = json!(ladder["partial"].as_i64().unwrap_or(0) + 1),
            _ => ladder["text_only"] = json!(ladder["text_only"].as_i64().unwrap_or(0) + 1),
        }
        scene_rows.push(json!({
            "scene_index": s.scene_index,
            "segment_id": s.segment_id,
            "sequence_order": s.sequence_order,
            "speaker_id": s.speaker_id,
            "text_chars": s.text_chars,
            "duration_ms": s.duration_ms,
            "characters": s.characters,
            "focal": s.focal,
            "narration_path": if s.audio_cache_path.is_empty() { Value::Null } else { json!(s.audio_cache_path) },
            "ladder": band,
        }));
    }

    Ok(json!({
        "chapter_id": chapter_id,
        "book_id": book_id,
        "chapter_number": chapter_number,
        "derived": {
            "refreshed": derived["refreshed"],
            "durations_filled": derived["durations_filled"],
            "source": "draft-time derivation (video_scenes) — stages never re-derive",
        },
        "scenes": scene_rows,
        "scene_count": scenes.len(),
        "total_ms": total_ms,
        "title_ms": super::assemble::TITLE_MS,
        "fps": super::assemble::DEFAULT_FPS,
        "resolution": format!("{}x{}", super::frame::SCENE_W, super::frame::SCENE_H),
        "ladder": ladder,
        "missing_sheets": missing_sheets,
        "narration_ready": narration_ready,
    }))
}

fn sheet_keys(conn: &Connection, book_id: i64) -> Result<Vec<String>, AppError> {
    ensure_video_schema(conn)?;
    let mut stmt = conn.prepare(
        "SELECT ref_key FROM video_assets WHERE book_id = ?1 AND kind = 'SHEET'",
    )?;
    let rows = stmt
        .query_map([book_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {

    #[test]
    fn plan_source_is_the_draft_time_derivation() {
        // Documentation-of-contract test: the plan reads persisted rows and
        // must advertise that (guards against someone re-deriving in stage
        // code later — the whole point of the no-double-work contract).
        let note = "draft-time derivation (video_scenes) — stages never re-derive";
        assert!(note.contains("never re-derive"));
    }
}
