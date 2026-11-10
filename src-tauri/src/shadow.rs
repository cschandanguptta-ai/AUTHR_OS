// AUTHR_OS — Shadow Markdown layer (F14): `manuscript/chNN.md` + companion
// `chNN.ast.json`, atomic `.tmp` writes (build plan §6.4 step 2, TEST_INFRA §5.3).

use crate::error::AppError;
use crate::ipc::Ctx;
use rusqlite::Connection;
use serde_json::{json, Value};

/// Primary book = lowest book id (bundles are single-narrative by default;
/// secondary books get a `ch{book}x{nn}` prefix to stay collision-free).
pub fn shadow_rel_path(conn: &Connection, chapter_row: &Value) -> Result<String, AppError> {
    let book_id = chapter_row
        .get("book_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::storage("chapter row missing book_id"))?;
    let chapter_number = chapter_row
        .get("chapter_number")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let primary: i64 = conn
        .query_row("SELECT MIN(id) FROM books", [], |r| r.get(0))
        .map_err(|e| AppError::storage(e.to_string()))?;
    let nn = format!("{:02}", chapter_number);
    if book_id == primary {
        Ok(format!("manuscript/ch{nn}.md"))
    } else {
        Ok(format!("manuscript/ch{book_id}x{nn}.md"))
    }
}

fn segments_for_chapter(conn: &Connection, chapter_id: i64) -> Result<Vec<Value>, AppError> {
    let sql = format!(
        "SELECT {} FROM chapter_segments WHERE chapter_id = ?1 ORDER BY sequence_order",
        crate::storage::db::SEGMENT_COLS.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![chapter_id], |r| {
            crate::storage::db::row_json(r, crate::storage::db::SEGMENT_COLS)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn display_name(conn: &Connection, speaker_id: &str) -> String {
    conn.query_row(
        "SELECT display_name FROM character_profiles WHERE character_key = ?1",
        rusqlite::params![speaker_id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_else(|_| speaker_id.to_string())
}

/// Render both shadow files for one chapter (no-op for missing chapters).
pub fn render_chapter(ctx: &Ctx, chapter_id: i64) -> Result<(), AppError> {
    let row = match crate::storage::chapters::get(&ctx.conn, chapter_id) {
        Ok(r) => r,
        Err(_) => return Ok(()), // chapter vanished mid-transaction: nothing to render
    };
    let segments = segments_for_chapter(&ctx.conn, chapter_id)?;

    let rel = shadow_rel_path(&ctx.conn, &row)?;
    let md = render_markdown(&row, &segments, &ctx.conn)?;
    let ast = render_ast(&segments)?;

    crate::bundle::manifest::atomic_write(&ctx.root.join(&rel), md.as_bytes())?;
    let ast_rel = rel.trim_end_matches(".md").to_string() + ".ast.json";
    crate::bundle::manifest::atomic_write(
        &ctx.root.join(&ast_rel),
        ast.as_bytes(),
    )?;
    Ok(())
}

/// Contract §7 body rendering:
/// NARRATION → verbatim · DIALOGUE → `**Name**: "Text"` · INTERNAL_MONOLOGUE → `*Text*`.
fn render_markdown(chapter: &Value, segments: &[Value], conn: &Connection) -> Result<String, AppError> {
    let chapter_id = chapter["id"].as_i64().unwrap_or(0);
    let chapter_number = chapter["chapter_number"].as_i64().unwrap_or(0);
    let title = chapter["title"].as_str().unwrap_or("");
    let act = chapter["act_number"].as_i64().unwrap_or(1);
    let status = chapter["status"].as_str().unwrap_or("PLANNED");
    let book_id = chapter["book_id"].as_i64().unwrap_or(0);
    let nn = format!("{:02}", chapter_number);

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("chapter_id: {chapter_id}\n"));
    out.push_str(&format!("chapter_number: {chapter_number}\n"));
    out.push_str(&format!("title: \"{title}\"\n"));
    out.push_str(&format!("act_number: {act}\n"));
    out.push_str(&format!("status: {status}\n"));
    out.push_str(&format!("book_id: {book_id}\n"));
    out.push_str("---\n\n");
    out.push_str(&format!("# Chapter {nn}: {title}\n\n"));

    for seg in segments {
        let text = seg["text_content"].as_str().unwrap_or("");
        let seg_type = seg["segment_type"].as_str().unwrap_or("NARRATION");
        match seg_type {
            "DIALOGUE" => {
                let speaker = seg["speaker_id"].as_str().unwrap_or("NARRATOR");
                let name = display_name(conn, speaker);
                out.push_str(&format!("**{name}**: \"{text}\"\n\n"));
            }
            "INTERNAL_MONOLOGUE" => {
                out.push_str(&format!("*{text}*\n\n"));
            }
            _ => {
                out.push_str(text);
                out.push_str("\n\n");
            }
        }
    }
    Ok(out)
}

/// `chNN.ast.json` — array of NarrativeSegmentAst objects (§5.2).
fn render_ast(segments: &[Value]) -> Result<String, AppError> {
    let mut arr = Vec::new();
    for seg in segments {
        arr.push(json!({
            "segment_id": seg["segment_id"],
            "chapter_id": seg["chapter_id"],
            "sequence_order": seg["sequence_order"],
            "segment_type": seg["segment_type"],
            "speaker_id": seg["speaker_id"],
            "emotion": seg["emotion"],
            "prosody_rate": seg["prosody_rate"],
            "prosody_pitch": seg["prosody_pitch"],
            "text_content": seg["text_content"],
            "ambient_cue": seg["ambient_cue"],
            "sfx_cue": seg["sfx_cue"],
            "prev_request_id": seg["prev_request_id"],
            "audio_hash": seg["audio_hash"],
            "audio_start_ms": seg["audio_start_ms"],
            "audio_duration_ms": seg["audio_duration_ms"],
        }));
    }
    Ok(serde_json::to_string_pretty(&arr)?)
}

/// Remove shadow files (chapter_delete cascade).
pub fn remove_chapter(ctx: &Ctx, chapter_row: &Value) -> Result<(), AppError> {
    let rel = shadow_rel_path(&ctx.conn, chapter_row)?;
    let ast_rel = std::path::PathBuf::from(&rel).with_extension("ast.json");
    let _ = std::fs::remove_file(ctx.root.join(&rel));
    let _ = std::fs::remove_file(ctx.root.join(ast_rel));
    Ok(())
}
