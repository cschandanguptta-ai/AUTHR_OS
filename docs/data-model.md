# data-model.md — entities, relationships, constraints, migration rules

Authoritative source: `src-tauri/src/storage/db.rs:10-144` (`SCHEMA_SQL`,
frozen — header comment "Do not edit") and `src-tauri/src/rag/store.rs:12-33`
(`RAG_SCHEMA_SQL`, additive). Row shapes exposed by the contract live at
`db.rs:214-240` and `docs/CLI_CONTRACT.md §5`.

## 1. ER diagram

```mermaid
erDiagram
    BOOKS ||--o{ CHAPTERS : "chapters.book_id CASCADE"
    CHAPTERS ||--o{ CHAPTER_SEGMENTS : "chapter_id CASCADE"
    CHAPTERS ||--o{ UNRESOLVED_THREADS : "introduced_in_chapter CASCADE"
    CHAPTERS ||--o{ UNRESOLVED_THREADS : "resolved_in_chapter SET NULL"
    CHARACTER_PROFILES ||--o{ CHAPTER_SEGMENTS : "speaker_id, ON UPDATE CASCADE, delete blocked"
    CHAPTER_SEGMENTS ||--|| CHAPTER_SEGMENTS_FTS : "triggers ai/ad/au"
    LOREBOOK ||..o{ RAG_CHUNKS : "snapshot, source='lorebook', no FK"
    CHARACTER_PROFILES ||..o{ RAG_CHUNKS : "snapshot, source='characters', no FK"
    CHAPTER_SEGMENTS ||..o{ RAG_CHUNKS : "snapshot, source='chapters', no FK"
    BOOKS {
        integer id PK
        text title
        text narrative_structure "CHECK 4 values"
        integer target_word_count
        integer last_modified_at
    }
    CHAPTERS {
        integer id PK
        integer book_id FK
        integer chapter_number "UNIQUE(book_id,chapter_number)"
        text status "CHECK 5 values"
    }
    CHAPTER_SEGMENTS {
        text segment_id PK "UUID v4"
        integer chapter_id FK
        integer sequence_order "UNIQUE(chapter_id,sequence_order)"
        text speaker_id FK
        text segment_type "CHECK 3 values"
        text prev_request_id "continuity pointer"
        text audio_hash "SHA-256 cache key"
        integer audio_duration_ms
    }
    CHARACTER_PROFILES {
        text character_key PK
        text voice_engine "CHECK 5 values"
        text voice_id
    }
    UNRESOLVED_THREADS {
        integer id PK
        integer introduced_in_chapter FK
        integer resolved_in_chapter FKnullable
        text status "CHECK 3 values"
    }
    LOREBOOK {
        integer id PK
        text key UK
        text category "CHECK 5 values"
    }
    CHAPTER_SEGMENTS_FTS {
        text segment_id "UNINDEXED"
        text text_content "porter unicode61"
    }
    RAG_CHUNKS {
        text chunk_id PK "source:ref_id"
        integer book_id
        text source
        text ref_id
        blob embedding "f32 LE, dim 384"
        integer dim
    }
    RAG_CHUNKS_FTS {
        text chunk_id "UNINDEXED"
        text content "porter unicode61"
    }
```

## 2. Tables (all FACT from `db.rs` unless noted)

| Table | PK | Key columns & constraints | Notes |
|---|---|---|---|
| `books` | `id` INTEGER AUTOINCREMENT | `narrative_structure` CHECK ∈ SAVE_THE_CAT/THREE_ACT/FICHTEAN_CURVE/HEROS_JOURNEY (`db.rs:19-20`); defaults: author Anonymous, genre Sci-Fi, 80000 words, 25 chapters | timestamps = unix secs (`strftime('%s','now')`) |
| `chapters` | `id` | FK `books` CASCADE; `UNIQUE(book_id, chapter_number)` (`db.rs:47`); `status` CHECK ∈ PLANNED/DRAFTING/POLISHED/AUDIO_READY/PUBLISHED; default 3500 target words | ordered by `(book_id, chapter_number)`, index `idx_chapters_book_order` |
| `character_profiles` | `character_key` TEXT | `role` CHECK ∈ PROTAGONIST/ANTAGONIST/SUPPORTING/NARRATOR; `voice_engine` CHECK ∈ KOKORO_LOCAL/EDGE_TTS/ELEVEN_LABS/CARTESIA/OPENAI_TTS (`db.rs:58-59`) | `NARRATOR` seeded `INSERT OR IGNORE` on every connection (`db.rs:179-186`) — segments' default speaker/FK target |
| `lorebook` | `id` | `key` UNIQUE; `category` CHECK ∈ GENERAL/LOCATION/FACTION/MAGIC_TECH/HISTORICAL | hybrid-RAG corpus source |
| `unresolved_threads` | `id` | FK introduced CASCADE, resolved SET NULL (`db.rs:85-86`); `thread_type` CHECK ∈ CHEKHOV_GUN/MYSTERY/SUBPLOT/PROMISE; `status` CHECK ∈ INTRODUCED/ESCALATED/RESOLVED | Chekhov ledger |
| `chapter_segments` | `segment_id` TEXT UUIDv4 | FK chapter CASCADE; FK speaker → character_profiles **ON UPDATE CASCADE** (delete of referenced key blocked ⇒ `FOREIGN_KEY_VIOLATION`, contract #18); `UNIQUE(chapter_id, sequence_order)`; audio columns `prev_request_id`, `audio_hash`, `audio_cache_path`, `audio_start_ms` (default 0), `audio_duration_ms` | primary AST storage; index `idx_segments_audio_hash` |
| `chapter_segments_fts` | (FTS5) | `segment_id UNINDEXED`, `text_content`, `tokenize='porter unicode61'` | synced by 3 triggers `trg_segments_ai/ad/au` (`db.rs:131-143`); `au` fires only on `UPDATE OF text_content` |
| `rag_chunks` | `chunk_id` TEXT = `"<source>:<ref_id>"` | `book_id`, `source`, `ref_id`, `content`, `dim`, `embedding` BLOB (little-endian f32, 384) | **no FK** — snapshot rows; created by idempotent migration (`rag/store.rs:35`); upsert keyed on chunk_id (`store.rs:41`) |
| `craft_state` | `book_id` FK books CASCADE | `format_id` (25 ids), `plot_arc_id` (18 ids), `style_persona_id` (37 ids), `plan_json` TEXT, `storefront_json` TEXT | **v1.3 additive** (`craft/engine.rs::ensure_craft_schema`, executed eagerly from `db::open_db`); idempotent `CREATE TABLE IF NOT EXISTS` — frozen DDL untouched |
| `chapter_beats` | `chapter_id` FK chapters CASCADE | `beat_id` TEXT, `beat_index` INTEGER, `beat_label` TEXT | **v1.3 additive**; one beat stamp per chapter; `idx_chapter_beats_beat` |
| `chapter_retention` | PK `(book_id, chapter_number)` | `ending_type` TEXT (6 SFB-2.0 ids or ''), `scores_json` TEXT (7×0–2), `total` INTEGER 0–14, `band` TEXT, `notes` TEXT ≤2000, `updated_at` | **v1.4 additive** (`craft/retention.rs::ensure_retention_schema`); QC verdicts per chapter; `idx_chapter_retention_book` |
| `rag_chunks_fts` | (FTS5) | `chunk_id UNINDEXED`, `content`, porter unicode61 | kept in sync by `upsert_chunk`/`clear_source` (`store.rs:41-70,126-133`) |

Indexes: `idx_chapters_book_order`, `idx_segments_chapter_order`,
`idx_segments_audio_hash`, `idx_threads_status` (`db.rs:119-122`) plus the
additive `idx_chapter_beats_beat` (craft v1.3) and
`idx_chapter_retention_book` (retention v1.4). Index
guarantees are asserted by `tests/tier1_features/test_t1_indexes.py`.

## 3. Engine PRAGMAs (every connection, `db.rs:147-154`)

`foreign_keys=ON` · `journal_mode=WAL` · `synchronous=NORMAL` ·
`temp_store=MEMORY` · `busy_timeout=5000` · `cache_size=-64000`.
`PRAGMA wal_checkpoint(TRUNCATE)` runs on close/export and after every write
commit when the bundle sits in a cloud-sync folder (`ipc/mod.rs:43-49`,
`lifecycle.rs:112-122`).

## 4. Derived/maintained columns (who writes what)

| Column | Writer | Trigger |
|---|---|---|
| `chapters.actual_word_count` | `chapters.rs::refresh_word_count:111-125` (space-count heuristic) | `segment_append`, `apply_mutation` |
| `chapters.last_modified_at`, `books.last_modified_at` | SQL `strftime` in each UPDATE; `books` bumped by `chapter_create` (`chapters.rs:26-29`) | — |
| `manifest.last_modified_at` | `Manifest::touch` via `Ctx::after_commit` (`ipc/mod.rs:43-44`) | every writer except `chapter_delete` (gotcha: `chapters.rs:103-108` skips it) |
| `chapter_segments.audio_*` | `studio.rs::synthesize_chapter:265-274` | every non-dry-run turn incl. cache hits |
| `segment.text change` ⇒ `audio_hash=NULL` | `apply_mutation` Rust-side (`segments.rs:163-172`) | text-diff upserts only |

## 5. Migration rules (how to change the schema safely)

1. **Never edit `SCHEMA_SQL`** (`db.rs:9` "Do not edit"; contract §4.12 note).
   It is applied only when the `books` table is absent (`db.rs:168-187`), so
   edits would never reach existing bundles and would fork fresh ones.
2. **Additive migrations**: follow the `rag_chunks` / `craft_state` pattern —
   an idempotent `CREATE TABLE IF NOT EXISTS`/`CREATE VIRTUAL TABLE IF NOT EXISTS`
   batch executed per connection (like `ensure_rag_schema`, `rag/store.rs:35-39`,
   or `ensure_craft_schema`, now called eagerly from `db::open_db`),
   called from every command that needs it (see `studio.rs:381,403,414`).
3. **Column adds to frozen tables**: if ever required, add a numbered
   `ALTER TABLE` guarded by a `PRAGMA table_info` check inside a new
   `ensure_*` function; bump the contract version because row shapes (§5)
   widen. UNKNOWN: no precedent exists in-repo yet — needs a human-approved
   ADR before the first core-table migration.
4. **Never widen a CHECK's accepted set silently** — `voice_engine`/`status`/
   `segment_type` values are contract-visible (`CLI_CONTRACT.md §4.4/4.11`,
   row shapes §5) and asserted by `test_t2_schema_constraints.py`.
5. **FK changes cascade into behavior tests**: `cascade_deletions`,
   `foreign_key_violations` tier-2 modules encode the current ON DELETE
   matrix.
6. **RAG store is rebuildable**: `rag_index` re-chunks and upserts
   (`rag/index.rs:100`); `clear_source` wipes a source (`store.rs:126`).
   Embedding format change (dim, dtype) ⇒ must bump `dim` handling everywhere
   (`embed.rs:13`, `store.rs` schema, `search.rs`) or cosine panics/mismatches.

## 6. Data lifecycle

- `project_init` → schema + NARRATOR + book #1 (`lifecycle.rs:43-58`).
- Segment deletes cascade FTS rows (trigger `ad`); chapter deletes cascade
  segments/threads + remove shadow files (`shadow.rs::remove_chapter:144`).
- `clean_sweep` purges orphaned segments/threads (defensive — FKs should make
  this a no-op) and rebuilds FTS (`cleaner.rs:7-40`).
- Audio WAVs are content-addressed and **never garbage-collected** by the DB;
  orphans (stale hashes) are harmless but counted in `audio_cache_stats`.
  INFERRED (no GC code exists).
- Git stores `manuscript/` + manifest only; DB/audio/exports/lock are
  gitignored (`git.rs:8`).
