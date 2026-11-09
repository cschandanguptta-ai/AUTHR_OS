// AUTHR_OS — Chapters CRUD + shadow-layer hooks + word-count maintenance.

use super::db;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

use crate::shadow;

pub fn create(ctx: &mut crate::ipc::Ctx, a: &Value) -> Result<Value, AppError> {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let chapter_number = crate::ipc::args::req_i64(a, "chapter_number")?;
    let title = crate::ipc::args::req_str(a, "title")?;
    let act = crate::ipc::args::get_i64(a, "act_number").unwrap_or(1);
    let synopsis = crate::ipc::args::get_str(a, "synopsis").unwrap_or_default();
    let twc = crate::ipc::args::get_i64(a, "target_word_count").unwrap_or(3500);
    let status = crate::ipc::args::get_str(a, "status").unwrap_or_else(|| "PLANNED".into());

    ctx.conn.execute(
        "INSERT INTO chapters(book_id, act_number, chapter_number, title, synopsis,
                              target_word_count, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![book_id, act, chapter_number, title, synopsis, twc, status],
    )?;
    let id = ctx.conn.last_insert_rowid();
    ctx.conn.execute(
        "UPDATE books SET last_modified_at = strftime('%s','now') WHERE id = ?1",
        rusqlite::params![book_id],
    )?;
    let row = get(&ctx.conn, id)?;
    ctx.after_commit()?;
    shadow::render_chapter(ctx, id)?;
    Ok(row)
}

pub fn get(conn: &Connection, id: i64) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM chapters WHERE id = ?1",
        db::CHAPTER_COLS.join(", ")
    );
    conn.query_row(&sql, rusqlite::params![id], |r| db::row_json(r, db::CHAPTER_COLS))
        .map_err(|_| AppError::not_found(format!("chapter {id} not found")))
}

pub fn list(conn: &Connection, book_id: Option<i64>) -> Result<Value, AppError> {
    let cols = db::CHAPTER_COLS.join(", ");
    let mut stmt;
    let rows = match book_id {
        Some(b) => {
            let sql = format!(
                "SELECT {cols} FROM chapters WHERE book_id = ?1
                 ORDER BY book_id, chapter_number"
            );
            stmt = conn.prepare(&sql)?;
            stmt.query_map(rusqlite::params![b], |r| db::row_json(r, db::CHAPTER_COLS))?
                .collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let sql = format!(
                "SELECT {cols} FROM chapters ORDER BY book_id, chapter_number"
            );
            stmt = conn.prepare(&sql)?;
            stmt.query_map([], |r| db::row_json(r, db::CHAPTER_COLS))?
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(json!({ "chapters": rows }))
}

pub fn update(ctx: &mut crate::ipc::Ctx, a: &Value) -> Result<Value, AppError> {
    let id = crate::ipc::args::req_i64(a, "id")?;
    get(&ctx.conn, id)?;
    let fields = [
        "title",
        "synopsis",
        "act_number",
        "chapter_number",
        "status",
        "target_word_count",
    ];
    let mut sets = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for name in fields {
        if let Some(v) = a.get(name) {
            if !v.is_null() {
                params.push(db::json_to_sql(v));
                sets.push(format!("{name} = ?{}", params.len()));
            }
        }
    }
    if !sets.is_empty() {
        sets.push("last_modified_at = strftime('%s','now')".to_string());
        let sql = format!("UPDATE chapters SET {} WHERE id = {}", sets.join(", "), id);
        let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        ctx.conn.execute(&sql, refs.as_slice())?;
        ctx.after_commit()?;
        // Re-render shadow (frontmatter carries title/status/act).
        shadow::render_chapter(ctx, id)?;
    }
    get(&ctx.conn, id)
}

pub fn delete(ctx: &mut crate::ipc::Ctx, id: i64) -> Result<Value, AppError> {
    let row = get(&ctx.conn, id)?;
    ctx.conn.execute("DELETE FROM chapters WHERE id = ?1", rusqlite::params![id])?;
    // v1.5 (ADR-0011): purge the derived media rows — no orphan scenes.
    if let (Some(b), Some(n)) = (
        row.get("book_id").and_then(|v| v.as_i64()),
        row.get("chapter_number").and_then(|v| v.as_i64()),
    ) {
        let _ = crate::video::derive::purge_chapter(&ctx.conn, id, b, n);
    }
    // Tech-debt T-4: deletions follow the same bookkeeping as every other
    // mutation — manifest touch BEFORE shadow/git sync (invariant §1.2),
    // then a contract-locked snapshot commit (§8 message reused) so the
    // removal lands in git immediately, not at the next chapter sync.
    ctx.after_commit()?;
    crate::shadow::remove_chapter(ctx, &row)?;
    let commit = if ctx.readonly {
        None
    } else {
        crate::storage::git::commit_all(&ctx.root, "chore: manual snapshot")?
    };
    Ok(json!({ "deleted": id, "commit": commit }))
}

/// Update `actual_word_count` for a chapter (called from the sync bridge).
pub fn refresh_word_count(conn: &Connection, chapter_id: i64) -> Result<i64, AppError> {
    conn.execute(
        "UPDATE chapters SET
           actual_word_count = (SELECT COALESCE(SUM((LENGTH(text_content) -
             LENGTH(REPLACE(text_content, ' ', '')) + 1)), 0)
             FROM chapter_segments WHERE chapter_id = ?1),
           last_modified_at = strftime('%s','now')
         WHERE id = ?1",
        rusqlite::params![chapter_id],
    )?;
    let n: i64 = conn
        .query_row("SELECT actual_word_count FROM chapters WHERE id = ?1", rusqlite::params![chapter_id], |r| r.get(0))
        .map_err(|_| AppError::not_found(format!("chapter {chapter_id} not found")))?;
    Ok(n)
}
