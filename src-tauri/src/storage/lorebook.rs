// AUTHR_OS — Lorebook CRUD (world-building rules & future RAG store).

use super::db;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn list(conn: &Connection) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM lorebook ORDER BY category, key",
        db::LORE_COLS.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| db::row_json(r, db::LORE_COLS))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "lore": rows }))
}

pub fn upsert(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    let key = crate::ipc::args::req_str(a, "key")?;
    let content = crate::ipc::args::req_str(a, "content")?;
    let category = crate::ipc::args::get_str(a, "category").unwrap_or_else(|| "GENERAL".into());
    let ipa = crate::ipc::args::get_str(a, "ipa_phonetic").unwrap_or_default();

    conn.execute(
        "INSERT INTO lorebook(key, category, content, ipa_phonetic)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(key) DO UPDATE SET
           category = excluded.category,
           content = excluded.content,
           ipa_phonetic = excluded.ipa_phonetic,
           last_modified_at = strftime('%s','now')",
        rusqlite::params![key, category, content, ipa],
    )?;
    get(conn, &key)
}

pub fn get(conn: &Connection, key: &str) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM lorebook WHERE key = ?1",
        db::LORE_COLS.join(", ")
    );
    conn.query_row(&sql, rusqlite::params![key], |r| db::row_json(r, db::LORE_COLS))
        .map_err(|_| AppError::not_found(format!("lore entry `{key}` not found")))
}

pub fn delete(conn: &Connection, key: &str) -> Result<Value, AppError> {
    get(conn, key)?;
    conn.execute("DELETE FROM lorebook WHERE key = ?1", rusqlite::params![key])?;
    Ok(json!({ "deleted": key }))
}
