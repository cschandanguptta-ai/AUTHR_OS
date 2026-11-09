// AUTHR_OS — Books CRUD (build plan §3 mapping of BookDao.kt).

use super::db;
use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn create(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    let title = crate::ipc::args::req_str(a, "title")?;
    let author = crate::ipc::args::get_str(a, "author").unwrap_or_else(|| "Anonymous".into());
    let genre = crate::ipc::args::get_str(a, "genre").unwrap_or_else(|| "Sci-Fi".into());
    let premise = crate::ipc::args::get_str(a, "premise").unwrap_or_default();
    let theme = crate::ipc::args::get_str(a, "theme").unwrap_or_default();
    let structure =
        crate::ipc::args::get_str(a, "narrative_structure").unwrap_or_else(|| "SAVE_THE_CAT".into());
    let twc = crate::ipc::args::get_i64(a, "target_word_count").unwrap_or(80_000);
    let tch = crate::ipc::args::get_i64(a, "target_chapters").unwrap_or(25);

    conn.execute(
        "INSERT INTO books(title, author, genre, premise, theme, narrative_structure,
                           target_word_count, target_chapters)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![title, author, genre, premise, theme, structure, twc, tch],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)
}

pub fn get(conn: &Connection, id: i64) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM books WHERE id = ?1",
        db::BOOK_COLS.join(", ")
    );
    conn.query_row(&sql, rusqlite::params![id], |r| db::row_json(r, db::BOOK_COLS))
        .map_err(|_| AppError::not_found(format!("book {id} not found")))
}

pub fn list(conn: &Connection) -> Result<Value, AppError> {
    let sql = format!(
        "SELECT {} FROM books ORDER BY id",
        db::BOOK_COLS.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| db::row_json(r, db::BOOK_COLS))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "books": rows }))
}

pub fn update(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    let id = crate::ipc::args::req_i64(a, "id")?;
    get(conn, id)?; // ENTITY_NOT_FOUND guard
    let fields = [
        "title",
        "author",
        "genre",
        "premise",
        "theme",
        "narrative_structure",
        "target_word_count",
        "target_chapters",
        "persona_id",
        "cover_prompt",
        "marketing_copy",
        "isbn",
        "cover_image_path",
        "readability_score",
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
        let sql = format!("UPDATE books SET {} WHERE id = {}", sets.join(", "), id);
        let refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        conn.execute(&sql, refs.as_slice())?;
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<Value, AppError> {
    get(conn, id)?;
    conn.execute("DELETE FROM books WHERE id = ?1", rusqlite::params![id])?;
    Ok(json!({ "deleted": id }))
}
