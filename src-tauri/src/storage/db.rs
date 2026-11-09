// AUTHR_OS — SQLite engine: verbatim schema (build plan §6.1), engine PRAGMAs
// (TEST_INFRA §5.1), row→JSON projection helpers and checkpointing.

use crate::error::AppError;
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde_json::{json, Map, Value};

/// Verbatim relational DDL — frozen by the build plan (§6.1). Do not edit.
pub const SCHEMA_SQL: &str = r#"
-- Books Table: Primary Novel Container
CREATE TABLE books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT 'Anonymous',
    genre TEXT NOT NULL DEFAULT 'Sci-Fi',
    premise TEXT NOT NULL DEFAULT '',
    theme TEXT NOT NULL DEFAULT '',
    narrative_structure TEXT NOT NULL DEFAULT 'SAVE_THE_CAT'
        CHECK(narrative_structure IN ('SAVE_THE_CAT', 'THREE_ACT', 'FICHTEAN_CURVE', 'HEROS_JOURNEY')),
    target_word_count INTEGER NOT NULL DEFAULT 80000,
    target_chapters INTEGER NOT NULL DEFAULT 25,
    persona_id TEXT NOT NULL DEFAULT 'Standard',
    cover_image_path TEXT,
    cover_prompt TEXT NOT NULL DEFAULT '',
    marketing_copy TEXT NOT NULL DEFAULT '',
    isbn TEXT NOT NULL DEFAULT '',
    readability_score REAL NOT NULL DEFAULT 0.0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_modified_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Chapters Table: Relational Acts and Chapter Milestones
CREATE TABLE chapters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    act_number INTEGER NOT NULL DEFAULT 1,
    chapter_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    synopsis TEXT NOT NULL DEFAULT '',
    target_word_count INTEGER NOT NULL DEFAULT 3500,
    actual_word_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'PLANNED'
        CHECK(status IN ('PLANNED', 'DRAFTING', 'POLISHED', 'AUDIO_READY', 'PUBLISHED')),
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_modified_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(book_id, chapter_number)
);

-- Character Profiles Table: Voice Casting & Pronunciation
CREATE TABLE character_profiles (
    character_key TEXT PRIMARY KEY, -- e.g. 'NARRATOR', 'CHAR_KIRA'
    display_name TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'SUPPORTING'
        CHECK(role IN ('PROTAGONIST', 'ANTAGONIST', 'SUPPORTING', 'NARRATOR')),
    bio TEXT NOT NULL DEFAULT '',
    ipa_phonetic TEXT NOT NULL DEFAULT '', -- Context-injected IPA pronunciation
    voice_engine TEXT NOT NULL DEFAULT 'KOKORO_LOCAL'
        CHECK(voice_engine IN ('KOKORO_LOCAL', 'EDGE_TTS', 'ELEVEN_LABS', 'CARTESIA', 'OPENAI_TTS')),
    voice_id TEXT NOT NULL DEFAULT 'af_bella',
    base_rate REAL NOT NULL DEFAULT 1.0,
    base_pitch REAL NOT NULL DEFAULT 1.0,
    cadence_notes TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Lorebook Table: World-Building Rules & Hybrid RAG Store
CREATE TABLE lorebook (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL DEFAULT 'GENERAL'
        CHECK(category IN ('GENERAL', 'LOCATION', 'FACTION', 'MAGIC_TECH', 'HISTORICAL')),
    content TEXT NOT NULL,
    ipa_phonetic TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_modified_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Unresolved Threads Table: Chekhov's Gun & Sub-Plot Ledger
CREATE TABLE unresolved_threads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_title TEXT NOT NULL,
    thread_type TEXT NOT NULL DEFAULT 'CHEKHOV_GUN'
        CHECK(thread_type IN ('CHEKHOV_GUN', 'MYSTERY', 'SUBPLOT', 'PROMISE')),
    introduced_in_chapter INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    resolved_in_chapter INTEGER REFERENCES chapters(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'INTRODUCED'
        CHECK(status IN ('INTRODUCED', 'ESCALATED', 'RESOLVED')),
    payoff_requirement TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_modified_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Chapter Segments Table: Primary AST Storage Node
CREATE TABLE chapter_segments (
    segment_id TEXT PRIMARY KEY, -- UUID v4
    chapter_id INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    sequence_order INTEGER NOT NULL,
    segment_type TEXT NOT NULL
        CHECK(segment_type IN ('NARRATION', 'DIALOGUE', 'INTERNAL_MONOLOGUE')),
    speaker_id TEXT NOT NULL REFERENCES character_profiles(character_key) ON UPDATE CASCADE,
    emotion TEXT NOT NULL DEFAULT 'neutral',
    prosody_rate REAL NOT NULL DEFAULT 1.0,
    prosody_pitch REAL NOT NULL DEFAULT 1.0,
    text_content TEXT NOT NULL,
    ambient_cue TEXT,
    sfx_cue TEXT,
    prev_request_id TEXT, -- Acoustic continuity pointer to speaker's prior line
    audio_hash TEXT,      -- SHA-256 chunk cache key
    audio_cache_path TEXT,
    audio_start_ms INTEGER NOT NULL DEFAULT 0,
    audio_duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_modified_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(chapter_id, sequence_order)
);

-- Indexes for Microsecond Queries
CREATE INDEX idx_chapters_book_order ON chapters(book_id, chapter_number);
CREATE INDEX idx_segments_chapter_order ON chapter_segments(chapter_id, sequence_order);
CREATE INDEX idx_segments_audio_hash ON chapter_segments(audio_hash);
CREATE INDEX idx_threads_status ON unresolved_threads(status);

-- FTS5 Full-Text Search Virtual Table
CREATE VIRTUAL TABLE chapter_segments_fts USING fts5(
    segment_id UNINDEXED,
    text_content,
    tokenize = 'porter unicode61'
);

-- Triggers for FTS5 Synchronization
CREATE TRIGGER trg_segments_ai AFTER INSERT ON chapter_segments BEGIN
    INSERT INTO chapter_segments_fts(segment_id, text_content) VALUES (new.segment_id, new.text_content);
END;

CREATE TRIGGER trg_segments_ad AFTER DELETE ON chapter_segments BEGIN
    DELETE FROM chapter_segments_fts WHERE segment_id = old.segment_id;
END;

CREATE TRIGGER trg_segments_au AFTER UPDATE OF text_content ON chapter_segments BEGIN
    DELETE FROM chapter_segments_fts WHERE segment_id = old.segment_id;
    INSERT INTO chapter_segments_fts(segment_id, text_content) VALUES (new.segment_id, new.text_content);
END;
"#;

/// Engine PRAGMAs — applied to every connection (TEST_INFRA §5.1).
pub const PRAGMAS_SQL: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -64000;
"#;

/// Open (or create) `project.db` inside `root` with the full engine setup.
pub fn open_db(root: &std::path::Path) -> Result<Connection, AppError> {
    let db_path = root.join("project.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| AppError::storage(format!("open project.db: {e}")))?;
    conn.execute_batch(PRAGMAS_SQL)
        .map_err(|e| AppError::storage(format!("pragmas: {e}")))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Create the schema if absent (idempotent), then seed the canonical narrator.
pub fn ensure_schema(conn: &Connection) -> Result<(), AppError> {
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='books'",
        [],
        |r| r.get(0),
    )
    .map_err(|e| AppError::storage(e.to_string()))?;
    if tables == 0 {
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| AppError::storage(format!("schema: {e}")))?;
    }
    // Canonical narrator — segments' default speaker (FK target).
    conn.execute(
        "INSERT OR IGNORE INTO character_profiles(character_key, display_name, role, bio)
         VALUES ('NARRATOR', 'Narrator', 'NARRATOR', 'Omniscient narrator voice')",
        [],
    )
    .map_err(|e| AppError::storage(e.to_string()))?;
    Ok(())
}

/// `PRAGMA wal_checkpoint(TRUNCATE)` — used by close/export and the
/// cloud-sync guard (§6.2): the WAL file ends up 0 bytes.
pub fn wal_truncate(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| AppError::storage(format!("wal checkpoint: {e}")))
}

/// Build a JSON object from a row, reading `cols` positionally.
/// Returns `rusqlite::Result` so it composes with `query_row` mappers.
pub fn row_json(row: &rusqlite::Row<'_>, cols: &[&str]) -> rusqlite::Result<Value> {
    let mut m = Map::new();
    for (i, c) in cols.iter().enumerate() {
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(f) => json!(f),
            ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(_) => {
                // Tech-debt T-5: JSON projections must never silently emit a
                // "<N bytes>" placeholder. No §5 projection selects a BLOB;
                // binary columns go through dedicated loaders (e.g.
                // rag::store::all_chunks). Fail loudly instead.
                return Err(rusqlite::Error::InvalidColumnType(
                    i,
                    (*c).to_string(),
                    rusqlite::types::Type::Blob,
                ));
            }
        };
        m.insert((*c).to_string(), v);
    }
    Ok(Value::Object(m))
}

/// Column projections (contract §5 row shapes).
pub const BOOK_COLS: &[&str] = &[
    "id", "title", "author", "genre", "premise", "theme", "narrative_structure",
    "target_word_count", "target_chapters", "persona_id", "cover_image_path",
    "cover_prompt", "marketing_copy", "isbn", "readability_score",
    "created_at", "last_modified_at",
];
pub const CHAPTER_COLS: &[&str] = &[
    "id", "book_id", "act_number", "chapter_number", "title", "synopsis",
    "target_word_count", "actual_word_count", "status", "created_at", "last_modified_at",
];
pub const CHARACTER_COLS: &[&str] = &[
    "character_key", "display_name", "role", "bio", "ipa_phonetic", "voice_engine",
    "voice_id", "base_rate", "base_pitch", "cadence_notes", "created_at",
];
pub const LORE_COLS: &[&str] = &[
    "id", "key", "category", "content", "ipa_phonetic", "created_at", "last_modified_at",
];
pub const THREAD_COLS: &[&str] = &[
    "id", "thread_title", "thread_type", "introduced_in_chapter", "resolved_in_chapter",
    "status", "payoff_requirement", "created_at", "last_modified_at",
];
pub const SEGMENT_COLS: &[&str] = &[
    "segment_id", "chapter_id", "sequence_order", "segment_type", "speaker_id", "emotion",
    "prosody_rate", "prosody_pitch", "text_content", "ambient_cue", "sfx_cue",
    "prev_request_id", "audio_hash", "audio_cache_path", "audio_start_ms",
    "audio_duration_ms", "created_at", "last_modified_at",
];

/// Convert a `serde_json::Value` into a rusqlite parameter.
pub fn json_to_sql(v: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match v {
        Value::Null => Box::new(rusqlite::types::Null),
        Value::Bool(b) => Box::new(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else {
                Box::new(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => Box::new(s.clone()),
        other => Box::new(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-5 regression: a BLOB column in a §5 JSON projection must fail loudly,
    /// never render the old silent "<N bytes>" placeholder.
    #[test]
    fn row_json_rejects_blob_columns_loudly() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE t(a TEXT, b BLOB);
             INSERT INTO t VALUES ('x', x'00ff10');",
        )
        .unwrap();
        let err = conn
            .query_row("SELECT a, b FROM t", [], |r| row_json(r, &["a", "b"]))
            .unwrap_err();
        assert!(matches!(
            err,
            rusqlite::Error::InvalidColumnType(_, _, rusqlite::types::Type::Blob)
        ));
        // Non-BLOB projections keep working.
        let ok = conn
            .query_row("SELECT a FROM t", [], |r| row_json(r, &["a"]))
            .unwrap();
        assert_eq!(ok["a"], "x");
    }
}
