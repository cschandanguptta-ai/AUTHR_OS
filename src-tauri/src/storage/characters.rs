// AUTHR_OS — Voice cast CRUD (character_profiles, global canonical keys).

use super::db;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn list(conn: &Connection) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM character_profiles ORDER BY character_key",
        db::CHARACTER_COLS.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| db::row_json(r, db::CHARACTER_COLS))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "characters": rows }))
}

pub fn upsert(ctx: &mut crate::ipc::Ctx, a: &Value) -> Result<Value, AppError> {
    let conn = &ctx.conn;
    let key = crate::ipc::args::req_str(a, "character_key")?;
    let display = crate::ipc::args::req_str(a, "display_name")?;
    let role = crate::ipc::args::get_str(a, "role").unwrap_or_else(|| "SUPPORTING".into());
    let bio = crate::ipc::args::get_str(a, "bio").unwrap_or_default();
    let ipa = crate::ipc::args::get_str(a, "ipa_phonetic").unwrap_or_default();
    let engine = crate::ipc::args::get_str(a, "voice_engine").unwrap_or_else(|| "KOKORO_LOCAL".into());
    let voice_id = crate::ipc::args::get_str(a, "voice_id").unwrap_or_else(|| "af_bella".into());
    let rate = crate::ipc::args::get_f64(a, "base_rate").unwrap_or(1.0);
    let pitch = crate::ipc::args::get_f64(a, "base_pitch").unwrap_or(1.0);
    let cadence = crate::ipc::args::get_str(a, "cadence_notes").unwrap_or_default();

    // Existing row? preserve unspecified fields.
    let exists: Option<String> = conn
        .query_row(
            "SELECT display_name FROM character_profiles WHERE character_key = ?1",
            rusqlite::params![key],
            |r| r.get(0),
        )
        .ok();
    match exists {
        Some(_) => {
            conn.execute(
                "UPDATE character_profiles SET
                   display_name = COALESCE(?2, display_name),
                   role = COALESCE(?3, role),
                   bio = COALESCE(?4, bio),
                   ipa_phonetic = COALESCE(?5, ipa_phonetic),
                   voice_engine = COALESCE(?6, voice_engine),
                   voice_id = COALESCE(?7, voice_id),
                   base_rate = COALESCE(?8, base_rate),
                   base_pitch = COALESCE(?9, base_pitch),
                   cadence_notes = COALESCE(?10, cadence_notes)
                 WHERE character_key = ?1",
                rusqlite::params![key, Some(display), Some(role), Some(bio), Some(ipa),
                    Some(engine), Some(voice_id), Some(rate), Some(pitch), Some(cadence)],
            )?;
        }
        None => {
            conn.execute(
                "INSERT INTO character_profiles(character_key, display_name, role, bio,
                                                ipa_phonetic, voice_engine, voice_id,
                                                base_rate, base_pitch, cadence_notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![key, display, role, bio, ipa, engine, voice_id, rate, pitch, cadence],
            )?;
        }
    }
    let row = get(conn, &key)?;

    // Re-render shadow files that mention this speaker (display_name / IPA
    // changes must propagate to manuscript/chNN.md).
    let mut chapters: Vec<i64> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT DISTINCT chapter_id FROM chapter_segments WHERE speaker_id = ?1")
            .map_err(|e| AppError::storage(e.to_string()))?;
        let mut rows = stmt
            .query_map(rusqlite::params![key], |r| r.get::<_, i64>(0))
            .map_err(|e| AppError::storage(e.to_string()))?;
        while let Some(row) = rows.next() {
            chapters.push(row.map_err(|e| AppError::storage(e.to_string()))?);
        }
    }
    for ch in chapters {
        crate::shadow::render_chapter(ctx, ch)?;
    }

    Ok(row)
}

pub fn get(conn: &Connection, key: &str) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM character_profiles WHERE character_key = ?1",
        db::CHARACTER_COLS.join(", ")
    );
    conn.query_row(&sql, rusqlite::params![key], |r| db::row_json(r, db::CHARACTER_COLS))
        .map_err(|_| AppError::not_found(format!("character `{key}` not found")))
}

pub fn delete(conn: &Connection, key: &str) -> Result<Value, AppError> {
    get(conn, key)?;
    // chapter_segments.speaker_id FK blocks deletion while referenced.
    conn.execute(
        "DELETE FROM character_profiles WHERE character_key = ?1",
        rusqlite::params![key],
    )?;
    Ok(json!({ "deleted": key }))
}
