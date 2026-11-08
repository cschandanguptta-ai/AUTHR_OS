# AUTHR_OS — Frozen IPC / CLI Contract (v1.1)

**Status**: v1 sections are FROZEN. v1.1 (Phases 4–5) extends the surface
**additively**: 13 new commands (§4.11–4.12) and 4 new error codes in two new
categories (§3). Every v1 command, code and semantic is untouched. Any further
change requires a contract version bump.

**Applies to**: Phase 1 (Storage Core), Phase 3 (Author's Cockpit UI),
Phase 4 (Audio Studio), Phase 5 (Local-First AI & Vector RAG).

---

## 1. Transport

| Surface  | Form |
|----------|------|
| Tauri IPC  | `#[tauri::command]` wrappers in `src-tauri/src/shell/`, invoked as `invoke(name, args)` |
| CLI (headless / tests) | `authr <command> [--bundle <BUNDLE_DIR>] [--args '<json>']` |
| Browser dev bridge | `POST /api/ipc` with `{command, args, bundle}` (vite middleware spawns the CLI) |

- Command names are **snake_case** and identical on every surface.
- `--args` is a single JSON object; unknown keys are ignored, missing optional keys use defaults.
- `--bundle` is the `.authr` bundle directory. Required by every bundle-scoped command
  (all except `project_init`, `project_import`, `keychain_*`).

## 2. Result Envelope

Exactly **one line** of JSON on stdout. Exit code `0` on success, `1` on failure.

```json
{"ok": true,  "data": <result>}
{"ok": false, "error": {"code": "<CODE>", "message": "<human-readable>", "category": "<storage|bundle|security|audio|ai>"}}
```

## 3. Error Taxonomy

### v1 — 9 canonical codes (frozen, closed set)

| Code | Category | Raised when |
|------|----------|-------------|
| `ENTITY_NOT_FOUND` | storage | Referenced book/chapter/segment/lore/thread/key does not exist |
| `FOREIGN_KEY_VIOLATION` | storage | Insert/update violates an FK (orphan parent, unknown speaker_key) |
| `STORAGE_ERROR` | storage | Any other storage-layer failure (IO, SQL, corrupt db) |
| `LEASE_LOCK_ACTIVE` | bundle | Fresh lease held by a live process on this/another host |
| `READ_ONLY_PROJECT` | bundle | Write attempted while bundle is open read-only (foreign active lease) |
| `BUNDLE_ERROR` | bundle | Missing/corrupt manifest, invalid bundle layout, export/import failure |
| `PATH_TRAVERSAL_VIOLATION` | security | Path escapes the bundle sandbox (`../`, encoded, null-byte, absolute) |
| `ZIP_SLIP_VIOLATION` | security | Archive entry escapes the destination (`../`, absolute, drive letter, symlink) |
| `SECURITY_ERROR` | security | Keychain unavailable/failure, or any other security-layer failure |

### v1.1 — 4 additive codes (Phases 4–5)

| Code | Category | Raised when |
|------|----------|-------------|
| `AUDIO_ERROR` | audio | Invalid audio args (engine name, layer kind, sample rate, wav decode, output name) |
| `AUDIO_ENGINE_UNAVAILABLE` | audio | Network TTS engine unreachable / API key env missing / `AUTHR_TTS_OFFLINE=1` |
| `AI_ERROR` | ai | Invalid RAG/LLM args (unknown source, mode, embedder, voice, empty query) |
| `AI_ENGINE_UNAVAILABLE` | ai | llama-server unreachable, Kokoro/MiniLM model or runtime missing |

## 4. Command Surface (48 commands)

### 4.1 Project lifecycle

| # | Command | Args (JSON) | Result `data` |
|---|---------|-------------|---------------|
| 1 | `project_init` | `{path, title, author?}` | `{path, manifest, book_id, readonly:false}` — creates bundle dir, manifest v1, `project.db` (schema + seeds `NARRATOR`), `.gitignore`, embedded git + initial scaffold commit |
| 2 | `project_open` | `{path}` (or `--bundle`) | `{readonly, manifest, cloud_sync_guard}` — lease acquire w/ stale recovery; foreign fresh lease ⇒ read-only open |
| 3 | `project_close` | — | `{released:true}` — lease release + `wal_checkpoint(TRUNCATE)` |
| 4 | `project_lock_status` | — | `{locked, pid?, hostname?, user?, locked_at?, lease_expires_at?, session_id?, expired?, readonly}` |
| 5 | `project_export` | `{out}` | `{out, bytes, entries}` — checkpoint, stage `.tmp.zip`, atomic rename; excludes `.authr_lock`, `project.db-wal`, `project.db-shm` and the embedded `.git/` revision directory |
| 6 | `project_import` | `{zip, dest}` | `{path, manifest}` — zip-slip guarded extraction + manifest validation |

### 4.2 Books

| # | Command | Args | Result |
|---|---------|------|--------|
| 7 | `book_create` | `{title, author?, genre?, premise?, theme?, narrative_structure?, target_word_count?, target_chapters?}` | book row |
| 8 | `book_list` | `{}` | `{books:[row…]}` |
| 9 | `book_update` | `{id, …fields}` | book row |
| 10 | `book_delete` | `{id}` | `{deleted:id}` |

### 4.3 Chapters

| # | Command | Args | Result |
|---|---------|------|--------|
| 11 | `chapter_create` | `{book_id, chapter_number, title, act_number?, synopsis?, target_word_count?, status?}` | chapter row |
| 12 | `chapter_list` | `{book_id?}` | `{chapters:[row…]}` ordered by `(book_id, chapter_number)` |
| 13 | `chapter_get` | `{id}` | chapter row |
| 14 | `chapter_update` | `{id, …fields}` | chapter row (re-renders shadow files when title/status change) |
| 15 | `chapter_delete` | `{id}` | `{deleted:id}` (cascades segments, threads, shadow files, FTS) |

### 4.4 Voice Cast (character_profiles)

| # | Command | Args | Result |
|---|---------|------|--------|
| 16 | `character_list` | `{}` | `{characters:[row…]}` |
| 17 | `character_upsert` | `{character_key, display_name, role?, bio?, ipa_phonetic?, voice_engine?, voice_id?, base_rate?, base_pitch?, cadence_notes?}` | character row (insert or update) |
| 18 | `character_delete` | `{character_key}` | `{deleted}` — segments referencing it are rejected by FK first (`FOREIGN_KEY_VIOLATION`) |

### 4.5 Lorebook

| # | Command | Args | Result |
|---|---------|------|--------|
| 19 | `lore_list` | `{}` | `{lore:[row…]}` |
| 20 | `lore_upsert` | `{key, content, category?, ipa_phonetic?}` | lore row |
| 21 | `lore_delete` | `{key}` | `{deleted}` |

### 4.6 Chekhov's Gun Ledger (unresolved_threads)

| # | Command | Args | Result |
|---|---------|------|--------|
| 22 | `thread_open` | `{thread_title, introduced_in_chapter, payoff_requirement, thread_type?, status?}` | thread row |
| 23 | `thread_list` | `{status?}` | `{threads:[row…]}` |
| 24 | `thread_resolve` | `{id, resolved_in_chapter, status?="RESOLVED"}` | thread row |

### 4.7 Segments (AST storage + FTS)

| # | Command | Args | Result |
|---|---------|------|--------|
| 25 | `segment_append` | `{chapter_id, segment_type, speaker_id, text_content, emotion?, prosody_rate?, prosody_pitch?, ambient_cue?, sfx_cue?, sequence_order?}` | segment row (new UUID v4; `sequence_order` defaults to next) |
| 26 | `segment_list` | `{chapter_id}` | `{segments:[row…]}` ordered by `sequence_order` |
| 27 | `segment_search` | `{query, limit?}` | `{hits:[{segment_id, chapter_id, text_content, rank}]}` — FTS5 BM25, porter unicode61 |

### 4.8 Editor Bridge

| # | Command | Args | Result |
|---|---------|------|--------|
| 28 | `sync_lexical_ast_mutation` | `{chapter_id, upserts:[NarrativeSegmentAst…], deletes:[segment_id…]}` | `{applied, commit?}` — single `BEGIN IMMEDIATE` transaction: update `text_content` / re-index `sequence_order` / insert new UUIDs / delete removed; invalidate `audio_hash` only on changed rows; re-render `manuscript/chNN.md` + `.ast.json`; git auto-commit `Auto-sync chNN` |

**Split/merge semantics** (§6.4 of build plan): split ⇒ leading segment keeps its UUID, tail gets a new one; merge ⇒ surviving segment keeps the leading UUID, trailing id goes to `deletes`.

### 4.9 Keychain (no bundle required)

| # | Command | Args | Result |
|---|---------|------|--------|
| 29 | `keychain_store` | `{service, key, value}` | `{stored:true}` |
| 30 | `keychain_get` | `{service, key}` | `{value}` or `ENTITY_NOT_FOUND` |
| 31 | `keychain_delete` | `{service, key}` | `{deleted:true}` |

Backend selection: OS keyring by default; **`AUTHR_MOCK_KEYCHAIN=1`** forces the file-backed
mock (`$AUTHR_MOCK_KEYRING_FILE` or `~/.authr/mock_keyring.json`). Headless CI always sets the mock env.

### 4.10 Maintenance + Sandbox Self-Test

| # | Command | Args | Result |
|---|---------|------|--------|
| 32 | `git_snapshot` | `{message?}` | `{commit}` — stage manuscript/ + manifest, commit (default `chore: manual snapshot`) |
| 33 | `clean_sweep` | `{}` | `{purged_segments, purged_threads, fts_rebuilt:true}` — orphan purge + FTS rebuild |

### 4.11 Audio Studio (Phase 4, v1.1)

All file arguments are bundle-relative and sandbox-resolved; traversal attempts
raise `PATH_TRAVERSAL_VIOLATION`. `AUTHR_TTS_OFFLINE=1` deterministically blocks
network engines (`AUDIO_ENGINE_UNAVAILABLE`). `SCRIPTED` is the deterministic
offline synthesis engine used by headless CI.

| # | Command | Args | Result |
|---|---------|------|--------|
| 34 | `audio_cache_stats` | `{}` | `{root, chunks, bytes, hits, misses, hit_rate}` — SHA-256 content-addressable chunk cache in `audio/segments/` |
| 35 | `audio_pronounce` | `{names:[…]}` (≤256) | `{entries:[{name, ipa, source:lexicon\|rules}], count}` — proper-noun IPA phonetics |
| 36 | `audio_plan` | `{chapter_id}` | `{turns, speakers, waves:[[idx…]], wave_count, deps, continuity:[{turn, previous_request_from, previous_request_id}], order, segment_turns}` — topological DAG lookahead schedule (same-speaker turns serialized for acoustic continuity, independent speakers parallel) |
| 37 | `audio_synthesize` | `{text, voice?, engine?, sample_rate?, rate?, pitch?, ipa_names?}` **or** `{chapter_id, engine?, sample_rate?, ipa_names?, dry_run?}` | single: `{cached, hash(sha256), engine, path, duration_ms?, request_id?}`; chapter: `{chapter_id, engine, turns, cache_hits, waves, results:[…]}` — synthesizes every segment with per-cast voices, stores `audio_hash`/`audio_cache_path`/`audio_duration_ms`/`prev_request_id` on each segment row, threads request ids for continuity |
| 38 | `audio_master` | `{layers:[{kind:SPEECH\|SFX\|AMBIENT, wav, gain_db?, offset_ms?}] (1–16), sample_rate?, out?, target_rms_db?=-20.0, ceiling_db?=-3.1, duck?=true, duck_params?{depth_db=18, attack_ms=120, release_ms=750}}` | `{out: exports/<name>.wav, sample_rate, layers, duck:{ducked_samples, ducked_ms}, metrics:{rms_db, peak_db, lufs, noise_floor_db, gain_applied_db, limited}, acx:{…}}` — 3-layer mix → sidechain ducking → two-pass normalize (−20 dB RMS target) → lookahead limiter (−3.1 dBFS) |
| 39 | `audio_validate` | `{wav}` | `{pass, rms_db, peak_db, noise_floor_db, checks:{rms_ok(−23…−18 dB), peak_ok(≤−3 dB), noise_ok(≤−60 dB)}, issues[]}` — ACX compliance report |

Engines (contract §4.4 `voice_engine` spellings): `SCRIPTED`, `KOKORO_LOCAL`
(via `local_tts_generate`), `EDGE_TTS`, `ELEVEN_LABS`, `CARTESIA`, `OPENAI_TTS`
(+`HF_MMS`). Keys come from env (`ELEVENLABS_API_KEY`, `CARTESIA_API_KEY`,
`OPENAI_API_KEY`, `HUGGINGFACE_API_KEY`).

### 4.12 Local-First AI & Vector RAG (Phase 5, v1.1)

| # | Command | Args | Result |
|---|---------|------|--------|
| 40 | `rag_index` | `{sources?=[lorebook,characters,chapters], book_id?, embedder?="hashing"}` | `{indexed:{…}, total_chunks, dim=384, embedder, elapsed_ms, sources_requested}` — one chunk per lore entry / character profile / AST segment; embeddings stored as f32 LE BLOBs in `rag_chunks` + `rag_chunks_fts` (FTS5 porter unicode61) |
| 41 | `rag_search` | `{query, k?=5(1–50), mode?="hybrid"\|"dense"\|"fts", embedder?}` | `{query, mode, k, hits:[{chunk_id, source, ref_id, snippet, score, ranks:{dense, fts}}], returned, corpus, embedder, latency_us, latency_ms}` — hybrid mode fuses BM25 + cosine via Reciprocal Rank Fusion (k=60) |
| 42 | `rag_status` | `{}` | `{chunks, by_source, embedder, dim, modes}` |
| 43 | `local_llm_generate` | `{prompt, max_tokens?=64, temperature?=0.7, seed?=42, grammar?, server_url?}` | `{backend:"OFFLINE_GRAMMAR"\|"LLAMA_SERVER", text, tokens, seed, prompt_cache_pinned, server?, timings?}` — offline engine is deterministic; `server_url`/`AUTHR_LLAMA_SERVER` routes to a llama.cpp `llama-server` `/completion` call with `cache_prompt:true`, `keep_alive:-1` |
| 44 | `local_llm_status` | `{}` | `{default_backend, server_configured, server_url?, capabilities:{grammar_constrained_output, prompt_prefix_kv_cache, offline}}` |
| 45 | `local_tts_generate` | `{text, voice?="af_bella"}` | Kokoro-82M availability-negotiated; model absent/runtime missing ⇒ `AI_ENGINE_UNAVAILABLE` with placement guidance |
| 46 | `local_ai_status` | `{}` | `{embedder, minilm_model_present, kokoro:{model_file, model_present, voices}, llm:…, fallbacks:{tts:"SCRIPTED", llm:"OFFLINE_GRAMMAR", embedder:"hashing-384"}, audio_cache?}` |

Bundle extension (additive): `models/` holds optional ONNX weights
(`all-MiniLM-L6-v2.onnx`, `kokoro-v1.0.onnx`) — see `docs/LOCAL_AI.md`.
`rag_chunks`/`rag_chunks_fts` are created by an idempotent migration; the
frozen §6.1 core DDL is untouched.

## 5. Row Shapes (camel-free — snake_case exactly as SQLite)

`book`: `{id, title, author, genre, premise, theme, narrative_structure, target_word_count, target_chapters, persona_id, cover_image_path, cover_prompt, marketing_copy, isbn, readability_score, created_at, last_modified_at}`

`chapter`: `{id, book_id, act_number, chapter_number, title, synopsis, target_word_count, actual_word_count, status, created_at, last_modified_at}`

`character`: `{character_key, display_name, role, bio, ipa_phonetic, voice_engine, voice_id, base_rate, base_pitch, cadence_notes, created_at}`

`lore`: `{id, key, category, content, ipa_phonetic, created_at, last_modified_at}`

`thread`: `{id, thread_title, thread_type, introduced_in_chapter, resolved_in_chapter, status, payoff_requirement, created_at, last_modified_at}`

`segment`: `{segment_id, chapter_id, sequence_order, segment_type, speaker_id, emotion, prosody_rate, prosody_pitch, text_content, ambient_cue, sfx_cue, prev_request_id, audio_hash, audio_cache_path, audio_start_ms, audio_duration_ms, created_at, last_modified_at}`

## 6. Bundle & Lease Semantics

- Bundle layout: `project.db*`, `.authr_lock`, `manifest.json`, `manuscript/`, `audio/segments/`, `audio/sfx_custom/`, `assets/`, `exports/`, `.git/`, `.gitignore`.
- `manifest.json` v1: `{schema_version:1, project_id:<uuid4>, title, author, created_at, last_modified_at, authr_version:"3.0.0-PROD"}`.
- Lease TTL **120 s**. A lease is *fresh* iff `lease_expires_at` is in the future; *live* iff `pid` is alive on the local host. Foreign-host leases cannot be liveness-checked and are honored while fresh.
- `.authr_lock` v1: `{pid, hostname, user, locked_at, lease_expires_at, session_id}` (ISO-8601 UTC).
- Recovery: dead local PID **or** expired lease **or** corrupt JSON ⇒ lease is reclaimed. Fresh foreign lease ⇒ open read-only; every mutating command then fails with `READ_ONLY_PROJECT`.
- Cloud-sync guard: bundle path containing `OneDrive` / `Dropbox` / `Google Drive` (case-insensitive) triggers `PRAGMA wal_checkpoint(TRUNCATE)` after every write commit (WAL reduced to 0 bytes).

## 7. Shadow Layer

- `manuscript/chNN.md` (zero-padded 2-digit): YAML frontmatter (`chapter_id`, `chapter_number`, `title`, `act_number`, `status`, `book_id`) + `# Chapter NN: <title>` + body:
  - `NARRATION` → verbatim paragraph
  - `DIALOGUE` → `**<display_name>**: "<text>"`
  - `INTERNAL_MONOLOGUE` → `*<text>*`
- `manuscript/chNN.ast.json`: JSON array of `NarrativeSegmentAst` objects (§5.2 of build plan).
- All writes staged via `<name>.tmp` then atomically renamed.

## 8. Git Messages

- Initial scaffold commit: `chore: initial scaffold`
- Chapter auto-commit: `Auto-sync chNN`
- Manual snapshot: `chore: manual snapshot`
- Signature: `AUTHR_OS <authr@local>`
- `.gitignore`: `project.db`, `project.db-wal`, `project.db-shm`, `.authr_lock`, `audio/`, `exports/`
