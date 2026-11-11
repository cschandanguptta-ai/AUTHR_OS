// AUTHR_OS — Chekhov's Gun ledger (unresolved_threads): open / list / resolve.

use super::db;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn open(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    let title = crate::ipc::args::req_str(a, "thread_title")?;
    let introduced = crate::ipc::args::req_i64(a, "introduced_in_chapter")?;
    let payoff = crate::ipc::args::req_str(a, "payoff_requirement")?;
    let ttype = crate::ipc::args::get_str(a, "thread_type").unwrap_or_else(|| "CHEKHOV_GUN".into());
    let status = crate::ipc::args::get_str(a, "status").unwrap_or_else(|| "INTRODUCED".into());

    conn.execute(
        "INSERT INTO unresolved_threads(thread_title, thread_type, introduced_in_chapter,
                                        payoff_requirement, status)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![title, ttype, introduced, payoff, status],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM unresolved_threads WHERE id = ?1",
        db::THREAD_COLS.join(", ")
    );
    conn.query_row(&sql, rusqlite::params![id], |r| db::row_json(r, db::THREAD_COLS))
        .map_err(|_| AppError::not_found(format!("thread {id} not found")))
}

pub fn list(conn: &Connection, status: Option<&str>) -> Result<Value, AppError> {
    let cols = db::THREAD_COLS.join(", ");
    let mut stmt;
    let rows = match status {
        Some(s) => {
            let sql = format!("SELECT {cols} FROM unresolved_threads WHERE status = ?1 ORDER BY id");
            stmt = conn.prepare(&sql)?;
            stmt.query_map(rusqlite::params![s], |r| db::row_json(r, db::THREAD_COLS))?
                .collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let sql = format!("SELECT {cols} FROM unresolved_threads ORDER BY id");
            stmt = conn.prepare(&sql)?;
            stmt.query_map([], |r| db::row_json(r, db::THREAD_COLS))?
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(json!({ "threads": rows }))
}

pub fn resolve(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    let id = crate::ipc::args::req_i64(a, "id")?;
    let resolved = crate::ipc::args::req_i64(a, "resolved_in_chapter")?;
    let status = crate::ipc::args::get_str(a, "status").unwrap_or_else(|| "RESOLVED".into());
    get(conn, id)?;
    conn.execute(
        "UPDATE unresolved_threads
         SET resolved_in_chapter = ?2, status = ?3,
             last_modified_at = strftime('%s','now')
         WHERE id = ?1",
        rusqlite::params![id, resolved, status],
    )?;
    get(conn, id)
}
