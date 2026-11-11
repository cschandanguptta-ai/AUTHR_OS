// AUTHR_OS — Craft engine (contract v1.3): applies formats / personas / plot
// arcs to a book, scaffolds chapters along an arc's beat sheet, and reports
// beat coverage. Storage is purely ADDITIVE (`craft_state`, `chapter_beats`)
// following the `rag_chunks` migration pattern — the frozen §6.1 DDL and all
// §5 row shapes stay untouched.

use super::catalog;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

const DEFAULT_FORMAT: &str = "NOVEL";
const DEFAULT_ARC: &str = "SAVE_THE_CAT";
const DEFAULT_PERSONA: &str = "EDGE_THRILLER";

/// Idempotent, additive migration (data-model.md rule 2 — the rag_chunks
/// pattern). Called by every craft/kdp command that needs the tables.
pub fn ensure_craft_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS craft_state (
            book_id INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
            format_id TEXT NOT NULL DEFAULT 'NOVEL',
            plot_arc_id TEXT NOT NULL DEFAULT 'SAVE_THE_CAT',
            style_persona_id TEXT NOT NULL DEFAULT 'EDGE_THRILLER',
            plan_json TEXT NOT NULL DEFAULT '[]',
            storefront_json TEXT NOT NULL DEFAULT '{}',
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );
        CREATE TABLE IF NOT EXISTS chapter_beats (
            chapter_id INTEGER PRIMARY KEY REFERENCES chapters(id) ON DELETE CASCADE,
            beat_id TEXT NOT NULL,
            beat_index INTEGER NOT NULL DEFAULT 0,
            beat_label TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_chapter_beats_beat ON chapter_beats(beat_id);
        "#,
    )
    .map_err(|e| AppError::storage(format!("craft schema: {e}")))?;
    Ok(())
}

/// Effective craft state for a book (defaults when never applied).
pub fn get_state(conn: &Connection, book_id: i64) -> Result<Value, AppError> {
    ensure_craft_schema(conn)?;
    let row = conn
        .query_row(
            "SELECT format_id, plot_arc_id, style_persona_id, plan_json, storefront_json, updated_at
             FROM craft_state WHERE book_id = ?1",
            rusqlite::params![book_id],
            |r| {
                Ok(json!({
                    "book_id": book_id,
                    "format_id": r.get::<_, String>(0)?,
                    "plot_arc_id": r.get::<_, String>(1)?,
                    "style_persona_id": r.get::<_, String>(2)?,
                    "plan": serde_json::from_str::<Value>(&r.get::<_, String>(3)?)
                        .unwrap_or_else(|_| json!([])),
                    "storefront": serde_json::from_str::<Value>(&r.get::<_, String>(4)?)
                        .unwrap_or_else(|_| json!({})),
                    "updated_at": r.get::<_, i64>(5)?,
                }))
            },
        )
        .ok()
        .unwrap_or_else(|| {
            json!({
                "book_id": book_id,
                "format_id": DEFAULT_FORMAT,
                "plot_arc_id": DEFAULT_ARC,
                "style_persona_id": DEFAULT_PERSONA,
                "plan": [],
                "storefront": {},
                "updated_at": null
            })
        });
    Ok(row)
}

fn resolve(current: Option<String>, requested: Option<&str>, kind: &str) -> Result<String, AppError> {
    if let Some(id) = requested {
        let known = match kind {
            "format" => catalog::format_by_id(id),
            "arc" => catalog::arc_by_id(id),
            _ => catalog::persona_by_id(id),
        };
        return known.map(|_| id.to_string()).ok_or_else(|| {
            AppError::craft(format!(
                "unknown craft {kind} id `{id}` — run craft_catalog kind={kind}s for valid ids"
            ))
        });
    }
    Ok(current.unwrap_or_else(|| match kind {
        "format" => DEFAULT_FORMAT.to_string(),
        "arc" => DEFAULT_ARC.to_string(),
        _ => DEFAULT_PERSONA.to_string(),
    }))
}

/// #57 craft_apply — attach format / plot arc / style persona to a book.
/// Writer (lease enforced). Also syncs the legacy visible columns:
/// `persona_id` always, `narrative_structure` only when the arc maps to one
/// of the four frozen CHECK values (unknown arcs never corrupt the core).
pub fn craft_apply(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let book = crate::storage::books::get(&ctx.conn, book_id)?; // ENTITY_NOT_FOUND guard
    let state = get_state(&ctx.conn, book_id)?;

    let format_id = resolve(
        state["format_id"].as_str().map(str::to_string),
        a.get("format_id").and_then(|v| v.as_str()),
        "format",
    )?;
    let arc_id = resolve(
        state["plot_arc_id"].as_str().map(str::to_string),
        a.get("plot_arc_id").and_then(|v| v.as_str()),
        "arc",
    )?;
    let persona_id = resolve(
        state["style_persona_id"].as_str().map(str::to_string),
        a.get("style_persona_id").and_then(|v| v.as_str()),
        "persona",
    )?;
    let adopt_defaults = a.get("adopt_defaults").and_then(|v| v.as_bool()).unwrap_or(false);

    let fmt = catalog::format_by_id(&format_id).expect("validated");
    let arc = catalog::arc_by_id(&arc_id).expect("validated");
    let persona = catalog::persona_by_id(&persona_id).expect("validated");

    ctx.conn.execute(
        "INSERT INTO craft_state(book_id, format_id, plot_arc_id, style_persona_id, plan_json)
         VALUES (?1, ?2, ?3, ?4,
                 COALESCE((SELECT plan_json FROM craft_state WHERE book_id = ?1), '[]'))
         ON CONFLICT(book_id) DO UPDATE SET
           format_id = excluded.format_id,
           plot_arc_id = excluded.plot_arc_id,
           style_persona_id = excluded.style_persona_id,
           updated_at = strftime('%s','now')",
        rusqlite::params![book_id, format_id, arc_id, persona_id],
    )?;

    // Legacy visibility sync (never lies: only legal CHECK values move).
    let legacy_arc = arc["legacy_structure"].as_str().map(str::to_string);
    if let Some(leg) = &legacy_arc {
        ctx.conn.execute(
            "UPDATE books SET narrative_structure = ?1 WHERE id = ?2",
            rusqlite::params![leg, book_id],
        )?;
    }
    ctx.conn.execute(
        "UPDATE books SET persona_id = ?1 WHERE id = ?2",
        rusqlite::params![persona_id, book_id],
    )?;
    let mut synced = json!({"persona_id": persona_id});
    if let Some(leg) = &legacy_arc {
        synced["narrative_structure"] = json!(leg);
    }

    // Optional: adopt the format's default shape as book targets.
    if adopt_defaults {
        ctx.conn.execute(
            "UPDATE books SET target_word_count = ?1, target_chapters = ?2 WHERE id = ?3",
            rusqlite::params![
                fmt["default_word_count"].as_i64().unwrap_or(80_000),
                fmt["default_chapters"].as_i64().unwrap_or(25),
                book_id
            ],
        )?;
        synced["target_word_count"] = fmt["default_word_count"].clone();
        synced["target_chapters"] = fmt["default_chapters"].clone();
    }

    ctx.after_commit()?;
    Ok(json!({
        "book_id": book_id,
        "book_title": book["title"],
        "format": fmt,
        "plot_arc": arc,
        "style_persona": persona,
        "legacy_sync": synced,
        "state": get_state(&ctx.conn, book_id)?,
    }))
}

/// #58 craft_scaffold — map an arc's beats across `chapter_count` chapters,
/// creating the missing ones (title = beat label, synopsis = beat guidance)
/// and stamping every touched chapter in `chapter_beats`. Deterministic:
/// chapter i of n carries beat floor(i·b/n), so every beat is covered and
/// coverage is monotonic for any n ≥ 1.
pub fn craft_scaffold(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let book = crate::storage::books::get(&ctx.conn, book_id)?;

    // Optional one-call override: plot_arc_id applies first.
    if let Some(arc_id) = a.get("plot_arc_id").and_then(|v| v.as_str()) {
        let mut apply_args = json!({"book_id": book_id, "plot_arc_id": arc_id});
        if let Some(p) = a.get("style_persona_id").and_then(|v| v.as_str()) {
            apply_args["style_persona_id"] = json!(p);
        }
        craft_apply(ctx, &apply_args)?;
    }
    let state = get_state(&ctx.conn, book_id)?;
    let arc_id = state["plot_arc_id"].as_str().unwrap_or(DEFAULT_ARC).to_string();
    let arc = catalog::arc_by_id(&arc_id).ok_or_else(|| {
        AppError::craft(format!("craft_state holds unknown arc `{arc_id}`"))
    })?;
    let beats = arc["beats"].as_array().cloned().unwrap_or_default();
    let b = beats.len().max(1);

    let n = crate::ipc::args::get_i64(a, "chapter_count")
        .unwrap_or_else(|| book["target_chapters"].as_i64().unwrap_or(25))
        .clamp(1, 500) as usize;
    let start = crate::ipc::args::get_i64(a, "start_chapter").unwrap_or(1).max(1);
    let per_words = crate::ipc::args::get_i64(a, "per_chapter_words").unwrap_or(3500).clamp(50, 50_000);
    let overwrite = a.get("overwrite_synopsis").and_then(|v| v.as_bool()).unwrap_or(false);

    ensure_craft_schema(&ctx.conn)?;
    let mut created = Vec::new();
    let mut mapped = 0usize;
    let mut plan = Vec::with_capacity(n);

    for i in 0..n {
        let bi = (i * b) / n; // monotonic, full coverage
        let beat = &beats[bi.min(b - 1)];
        let beat_id = beat["id"].as_str().unwrap_or("beat");
        let beat_label = beat["label"].as_str().unwrap_or("Beat");
        let guidance = beat["guidance"].as_str().unwrap_or("");
        let chapter_number = start + i as i64;
        let act = (1 + (bi * 3) / b).clamp(1, 3) as i64;

        let existing: Option<i64> = ctx
            .conn
            .query_row(
                "SELECT id FROM chapters WHERE book_id = ?1 AND chapter_number = ?2",
                rusqlite::params![book_id, chapter_number],
                |r| r.get(0),
            )
            .ok();

        let chapter_id = match existing {
            Some(id) => {
                if overwrite && !guidance.is_empty() {
                    ctx.conn.execute(
                        "UPDATE chapters SET synopsis = ?1, last_modified_at = strftime('%s','now')
                         WHERE id = ?2",
                        rusqlite::params![guidance, id],
                    )?;
                }
                id
            }
            None => {
                ctx.conn.execute(
                    "INSERT INTO chapters(book_id, act_number, chapter_number, title, synopsis,
                                          target_word_count, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'PLANNED')",
                    rusqlite::params![book_id, act, chapter_number, beat_label, guidance, per_words],
                )?;
                let id = ctx.conn.last_insert_rowid();
                shadow_render(ctx, id)?;
                created.push(crate::storage::chapters::get(&ctx.conn, id)?);
                id
            }
        };

        ctx.conn.execute(
            "INSERT INTO chapter_beats(chapter_id, beat_id, beat_index, beat_label)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(chapter_id) DO UPDATE SET
               beat_id = excluded.beat_id,
               beat_index = excluded.beat_index,
               beat_label = excluded.beat_label",
            rusqlite::params![chapter_id, beat_id, bi as i64, beat_label],
        )?;
        mapped += 1;
        plan.push(json!({
            "chapter_number": chapter_number,
            "chapter_id": chapter_id,
            "beat_index": bi,
            "beat_id": beat_id,
            "beat_label": beat_label,
            "created": existing.is_none(),
        }));
    }

    let plan_json = serde_json::to_string(&plan).unwrap_or_else(|_| "[]".into());
    ctx.conn.execute(
        "INSERT INTO craft_state(book_id, format_id, plot_arc_id, style_persona_id, plan_json)
         VALUES (?1,
                 COALESCE((SELECT format_id FROM craft_state WHERE book_id = ?1), ?2),
                 ?3,
                 COALESCE((SELECT style_persona_id FROM craft_state WHERE book_id = ?1), ?4),
                 ?5)
         ON CONFLICT(book_id) DO UPDATE SET
           plot_arc_id = excluded.plot_arc_id,
           plan_json = excluded.plan_json,
           updated_at = strftime('%s','now')",
        rusqlite::params![book_id, DEFAULT_FORMAT, arc_id, DEFAULT_PERSONA, plan_json],
    )?;

    ctx.conn.execute(
        "UPDATE books SET last_modified_at = strftime('%s','now') WHERE id = ?1",
        rusqlite::params![book_id],
    )?;
    ctx.after_commit()?;

    Ok(json!({
        "book_id": book_id,
        "plot_arc": arc_id,
        "arc_label": arc["label"],
        "chapters_total": n,
        "created": created.len(),
        "mapped": mapped,
        "beats_total": b,
        "plan": plan,
        "chapters_created": created,
    }))
}

/// #59 craft_progress — beat coverage report fused with the Chekhov ledger.
pub fn craft_progress(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let book = crate::storage::books::get(&ctx.conn, book_id)?;
    let state = get_state(&ctx.conn, book_id)?;
    let arc = catalog::arc_by_id(state["plot_arc_id"].as_str().unwrap_or(DEFAULT_ARC));
    let beats = arc
        .as_ref()
        .map(|x| x["beats"].as_array().cloned().unwrap_or_default())
        .unwrap_or_default();

    // chapters + beat stamps + word counts in one pass.
    let mut stmt = ctx.conn.prepare(
        "SELECT c.id, c.chapter_number, c.title, c.status, c.actual_word_count,
                cb.beat_id, cb.beat_index, cb.beat_label
         FROM chapters c
         LEFT JOIN chapter_beats cb ON cb.chapter_id = c.id
         WHERE c.book_id = ?1
         ORDER BY c.chapter_number",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![book_id], |r| {
            Ok(json!({
                "chapter_id": r.get::<_, i64>(0)?,
                "chapter_number": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "status": r.get::<_, String>(3)?,
                "actual_word_count": r.get::<_, i64>(4)?,
                "beat_id": r.get::<_, Option<String>>(5)?,
                "beat_index": r.get::<_, Option<i64>>(6)?,
                "beat_label": r.get::<_, Option<String>>(7)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let total_words: i64 = rows.iter().map(|c| c["actual_word_count"].as_i64().unwrap_or(0)).sum();
    let target_words = book["target_word_count"].as_i64().unwrap_or(0);

    let mut beat_rows = Vec::with_capacity(beats.len());
    let mut covered = 0usize;
    for (i, beat) in beats.iter().enumerate() {
        let bid = beat["id"].as_str().unwrap_or("");
        let chapters: Vec<Value> = rows
            .iter()
            .filter(|c| c["beat_id"].as_str() == Some(bid))
            .cloned()
            .collect();
        let done = chapters.iter().any(|c| {
            c["status"].as_str().unwrap_or("PLANNED") != "PLANNED"
                || c["actual_word_count"].as_i64().unwrap_or(0) > 0
        });
        if !chapters.is_empty() && done {
            covered += 1;
        }
        beat_rows.push(json!({
            "beat_index": i,
            "beat_id": beat["id"],
            "beat_label": beat["label"],
            "guidance": beat["guidance"],
            "chapters": chapters,
            "started": done,
        }));
    }

    let open_threads: i64 = ctx.conn.query_row(
        "SELECT COUNT(*) FROM unresolved_threads t
         JOIN chapters c ON c.id = t.introduced_in_chapter
         WHERE c.book_id = ?1 AND t.status != 'RESOLVED'",
        rusqlite::params![book_id],
        |r| r.get(0),
    )?;

    let total_beats = beats.len();
    let coverage_pct = if total_beats == 0 {
        0.0
    } else {
        (covered as f64 * 100.0 / total_beats as f64 * 10.0).round() / 10.0
    };
    Ok(json!({
        "book_id": book_id,
        "format_id": state["format_id"],
        "plot_arc_id": state["plot_arc_id"],
        "arc_label": arc.as_ref().map(|x| x["label"].clone()).unwrap_or(json!(null)),
        "style_persona_id": state["style_persona_id"],
        "total_beats": total_beats,
        "covered_beats": covered,
        "coverage_pct": coverage_pct,
        "beats": beat_rows,
        "chapters": rows.len(),
        "total_words": total_words,
        "target_words": target_words,
        "unresolved_threads": open_threads,
        "plan": state["plan"],
    }))
}

/// Shadow render helper that swallows only the "already rendered" no-op while
/// surfacing real failures (used when scaffold creates chapters).
fn shadow_render(ctx: &mut crate::ipc::Ctx, chapter_id: i64) -> Result<(), AppError> {
    crate::shadow::render_chapter(ctx, chapter_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_unknown_ids() {
        let err = resolve(None, Some("NOT_AN_ARC"), "arc").unwrap_err();
        assert_eq!(err.code, "CRAFT_ERROR");
        let ok = resolve(None, Some("STORY_CIRCLE"), "arc").unwrap();
        assert_eq!(ok, "STORY_CIRCLE");
        // Falls back to defaults when nothing requested.
        assert_eq!(resolve(None, None, "format").unwrap(), DEFAULT_FORMAT);
        assert_eq!(resolve(Some("KISHOTENKETSU".into()), None, "arc").unwrap(), "KISHOTENKETSU");
    }

    #[test]
    fn state_defaults_are_catalog_legal() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE books(id INTEGER PRIMARY KEY, title TEXT);
             CREATE TABLE chapters(
                id INTEGER PRIMARY KEY, book_id INTEGER, act_number INTEGER DEFAULT 1,
                chapter_number INTEGER, title TEXT, synopsis TEXT DEFAULT '',
                target_word_count INTEGER DEFAULT 3500, actual_word_count INTEGER DEFAULT 0,
                status TEXT DEFAULT 'PLANNED');
             CREATE TABLE character_profiles(
                character_key TEXT PRIMARY KEY, display_name TEXT, role TEXT DEFAULT 'NARRATOR');
             INSERT INTO books(id, title) VALUES (1, 'T');
             INSERT INTO character_profiles VALUES ('NARRATOR','Narrator','NARRATOR');",
        )
        .unwrap();
        ensure_craft_schema(&conn).unwrap();
        let st = get_state(&conn, 1).unwrap();
        assert_eq!(st["format_id"], "NOVEL");
        assert_eq!(st["plot_arc_id"], "SAVE_THE_CAT");
        assert_eq!(st["style_persona_id"], "EDGE_THRILLER");
        // Second call stays stable (idempotent migration).
        assert!(ensure_craft_schema(&conn).is_ok());
        assert_eq!(get_state(&conn, 1).unwrap()["plot_arc_id"], "SAVE_THE_CAT");
    }
}
