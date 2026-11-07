// AUTHR_OS — Maintenance: orphan purge + FTS reindex (contract #33).

use crate::error::AppError;
use serde_json::json;
use std::path::Path;

pub fn sweep(conn: &rusqlite::Connection, root: &Path) -> Result<serde_json::Value, AppError> {
    let purged_segments = conn
        .execute(
            "DELETE FROM chapter_segments
             WHERE chapter_id NOT IN (SELECT id FROM chapters)",
            [],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
    let purged_threads = conn
        .execute(
            "DELETE FROM unresolved_threads
             WHERE introduced_in_chapter NOT IN (SELECT id FROM chapters)",
            [],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;

    // Full FTS rebuild from the authoritative row set.
    conn.execute_batch("DELETE FROM chapter_segments_fts;")
        .map_err(|e| AppError::storage(e.to_string()))?;
    conn.execute_batch(
        "INSERT INTO chapter_segments_fts(segment_id, text_content)
         SELECT segment_id, text_content FROM chapter_segments;",
    )
    .map_err(|e| AppError::storage(e.to_string()))?;

    let _ = root;
    Ok(json!({
        "purged_segments": purged_segments,
        "purged_threads": purged_threads,
        "fts_rebuilt": true
    }))
}
