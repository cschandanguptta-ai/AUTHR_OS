# AUTHR_OS — Frozen IPC / CLI Contract (v1.3)

**Status**: v1 sections are FROZEN. v1.1 (Phases 4–5) extended the surface
**additively**: 13 new commands (§4.11–4.12) and 4 new error codes in two new
categories (§3). v1.2 (Phase 6) extends it further: 8 new commands (§4.13),
**reusing the frozen error taxonomy** — no new codes. v1.3 (Craft & KDP)
extends it additively once more: 6 new commands (§4.14) and **one** new error
code (`CRAFT_ERROR`, category `craft`). Every v1/v1.1/v1.2 command, code and
semantic is untouched.

**Applies to**: Phase 1 (Storage Core), Phase 3 (Author's Cockpit UI),
Phase 4 (Audio Studio), Phase 5 (Local-First AI & Vector RAG),
Phase 6 (Publishing, Packaging & Hardening), Phase 8 (Craft Catalog &
Storefront Packaging).

---

## 1. Transport

| Surface  | Form |
|----------|------|
| Tauri IPC  | single `#[tauri::command]` pass-through `authr_command` in `src-tauri/src/shell/` (workerised, ADR-0008); the frontend bridge routes every contract command through it |
| CLI (headless / tests) | `authr <command> [--bundle <BUNDLE_DIR>] [--args '<json>']` |
| Browser dev bridge | `POST /api/ipc` with `{command, args, bundle}` (vite middleware spawns the CLI) |

- Command names are **snake_case** and identical on every surface.
- `--args` is a single JSON object; unknown keys are ignored, missing optional keys use defaults.
- `--args-stdin` (additive, Phase 7): read the args JSON from stdin instead of argv. For
  payloads near the OS per-argument limit (~128 KB on Linux — e.g. bulk Lexical AST
  mutations) the dev bridge switches to this automatically. Mutually exclusive with `--args`.
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

### v1.3 — 1 additive code (Craft & KDP)

| Code | Category | Raised when |
|------|----------|-------------|
| `CRAFT_ERROR` | craft | Invalid craft/KDP args: unknown format/arc/persona id, >7 keywords, >2 categories, description outside 20–4000 chars, price outside 0.99–200, unknown catalog `kind` |

## 4. Command Surface (62 commands)

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
| 15 | `chapter_delete` | `{id}` | `{deleted:id, commit:oid\|null}` (cascades segments, threads, shadow files, FTS; manifest touch + contract-locked `chore: manual snapshot` commit — T-4) |

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
| 27 | `segment_search` | `{query, limit?}` | `{hits:[{segment_id, chapter_id, text_content, rank}]}` — FTS5 BM25, porter unicode61; the query is sanitized with the shared `rag::store::fts_sanitize` and bound as a parameter (T-2): FTS syntax in user input is searched literally, never a MATCH syntax error |

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

### 4.13 Publishing & Release (Phase 6, v1.2)

All `publish_*` commands are bundle-scoped **writers** (lease enforced,
`exports/` sandbox). Output names derive from the book title via a slugifier
that collapses hostile titles to safe bare names — traversal attempts raise
`PATH_TRAVERSAL_VIOLATION`. `release_*` and `benchmark_run` are bundle-free
(like `keychain_*`). The error taxonomy is unchanged: publishing failures
(missing ffmpeg, unsynthesized chapters, typst compile errors) raise
`BUNDLE_ERROR`.

| # | Command | Args | Result |
|---|---------|------|--------|
| 47 | `publish_epub` | `{book_id}` | `{out: exports/<slug>.epub, format:"EPUB 3", entries, chapters, overlays, duration_ms, dc:{title,creator,identifier,language,publisher}}` — EPUB 3 OCF: stored-first `mimetype`, container.xml, Dublin Core `content.opf` with `dcterms:modified`, `nav.xhtml`, per-chapter XHTML with `id="sN"` anchors, **SMIL media overlays** (`overlay/chNN.smil`, one `<par>` per segment with `clipBegin`/`clipEnd`) and embedded cached WAVs whenever the chapter is synthesized |
| 48 | `publish_docx` | `{book_id}` | `{out, chapters, paragraphs, words}` — WordprocessingML manuscript: Title/Subtitle/Heading1 styles, bold speaker runs, italic monologue, `docProps/core.xml` |
| 49 | `publish_typst` | `{book_id, typ_bin?}` | `{typ, pdf?, compiled, reason?, chapters, words}` — always emits the print-ready `.typ` (A5 novel template); compiles the PDF via `typ_bin` / `AUTHR_TYPST_BIN` / PATH when present, otherwise degrades gracefully (`compiled:false` + guidance); compile failure ⇒ `BUNDLE_ERROR` |
| 50 | `publish_captions` | `{chapter_id, formats?=["lrc","vtt"]}` | `{chapter_id, lrc?, vtt?, cues, duration_ms}` — line-synced `.lrc` + WebVTT `.vtt` built from segment durations (synthesized values, else the scheduler estimator); timeline identical to the EPUB overlays |
| 51 | `publish_m4b` | `{book_id}` or `{chapter_id}`, `ffmpeg_bin?` | `{out, bytes, chapters, duration_ms, codec:"aac-lc 64k mono 44.1kHz", ffmpeg}` — ffmpeg concat → AAC-LC → `-f ipod` M4B with ffmetadata **chapter atoms**; missing binary ⇒ `BUNDLE_ERROR` with install guidance; any unsynthesized segment ⇒ `BUNDLE_ERROR` |
| 52 | `release_manifest` | `{artifact, version?, url?, key_hex?, out?}` | `{manifest, manifest_data, artifact_bytes, artifact_sha256, generated_key_hex?}` — signs the artifact bytes with **ed25519**; key from `key_hex` / `AUTHR_RELEASE_KEY_HEX` / ephemeral (returned, CI-only); emits the Tauri-updater-shaped `latest.json` |
| 53 | `release_verify` | `{manifest, artifact?}` | `{valid, checks:{sha256, signature}, artifact, version}` — independent verification; tampered artifacts report `valid:false` (still `ok:true`) |
| 54 | `benchmark_run` | `{iterations?=7(3–25), cold_start_limit_ms?=1500, rss_limit_mb?=50, binary_limit_mb?=25}` | `{cold_start_ms:{iterations,min,median,p95,max,limit}, rss:{peak_kb,peak_mb,current_kb,limit_mb}, binary:{bytes,mb,limit_mb}, checks, pass}` — Sprint 16 hardening thresholds: median cold start <1.5 s, peak RSS <50 MB, binary <25 MB |

### 4.14 Craft Catalog & Storefront Packaging (v1.3)

The offline craft knowledge base: **25 project formats**, **37 bestseller-
archetype style personas**, **18 plot-arc structures with beat sheets**, and
Amazon KDP royalty constants. `craft_catalog` is bundle-free; the writers
enforce the lease. Storage is purely additive: `craft_state` (per-book
craft preset + plan + storefront payload) and `chapter_beats` (beat stamps)
follow the `rag_chunks` idempotent-migration pattern; the frozen §6.1 DDL and
all §5 row shapes are untouched. `craft_apply` also syncs the legacy visible
columns `persona_id` (always) and `narrative_structure` (only when the arc
maps to one of the four frozen CHECK values).

| # | Command | Args (JSON) | Result `data` |
|---|---------|-------------|---------------|
| 57 | `craft_catalog` | `{kind?="all\|formats\|personas\|arcs\|pricing"}` | full catalog entries + counts + `pricing` (KDP constants, planning approximation) — bundle-free, deterministic, offline |
| 58 | `craft_apply` | `{book_id, format_id?, plot_arc_id?, style_persona_id?, adopt_defaults?=false}` | `{book_id, format, plot_arc, style_persona, legacy_sync, state}` — validates ids (`CRAFT_ERROR`), upserts `craft_state`, syncs `persona_id`/`narrative_structure`, optionally adopts the format's default word/chapter targets |
| 59 | `craft_scaffold` | `{book_id, plot_arc_id?, chapter_count?, start_chapter?=1, per_chapter_words?=3500, overwrite_synopsis?=false}` | `{book_id, plot_arc, arc_label, chapters_total, created, mapped, beats_total, plan[], chapters_created[]}` — deterministic beat mapping: chapter *i* of *n* carries beat `⌊i·b/n⌋` (monotonic, full coverage for any n); missing chapters are created (title = beat label, synopsis = beat guidance, act = 1+⌊3·beat/b⌋ clamped 1–3) |
| 60 | `craft_progress` | `{book_id}` | `{total_beats, covered_beats, coverage_pct, beats:[{beat_id, label, guidance, chapters[], started}], chapters, total_words, target_words, unresolved_threads, plan}` — beat coverage fused with word counts and the Chekhov ledger |
| 61 | `kdp_check` | `{book_id, price_usd?=4.99}` | `{pass, readiness_score(0–100), checks:{manuscript, epub, docx, cover, metadata, print, audio, polish}, issues:[{severity:FAIL\|WARN\|INFO, area, message, fix}], pricing_hint:{ebook 70%/35%, paperback, hardcover, audiobook_acx}, est_pages}` — read-only compliance scan over manuscript/exports/cover/metadata. **Cover is dimension-probed from the image header** (PNG IHDR / JPEG SOF, no codec dependency): `checks.cover` carries `decodable, width, height, warnings[]`; a decodable-but-sub-spec image (below 1000px longest side or off the ~1.6:1 ratio) scores for presence yet raises WARN issues — never a silent pass |
| 62 | `kdp_package` | `{book_id, price_usd?=4.99, keywords?[≤7, ≤50 chars], categories?[≤2], description?(20–4000 chars, falls back to `marketing_copy`), subtitle?, language?="en", isbn?, publisher?, edition?, ai_disclosure?=false, drm?=false}` | `{package_dir: exports/kdp/<slug>/, files:[metadata.json, description.html, copyright-page.xhtml, checklist.md], metadata, royalties, issues, words, est_pages}` — writer; persists the storefront payload in `craft_state.storefront_json` |

KDP packaging guarantees: `description.html` is rebuilt by a **whitelist
sanitizer** (all input escaped; blank-line paragraphs, `- ` bullet lists,
`*bold*`, `_italic_` only — no source tag can survive); the copyright page is
a well-formed XHTML fragment with every value XML-escaped; royalty math is an
offline planning approximation from `catalog::pricing()` — always re-verify
current rates on kdp.amazon.com before filing real prices.

Persona hooks: `local_llm_generate` accepts an additive `style_persona` arg
(§4.12) — the persona's craft directive is prepended to the prompt for every
backend, and the response carries `style_persona`.

Storefront-grade EPUB (additive enrichment of #47 `publish_epub`, same args,
all legacy fields unchanged): when `books.cover_image_path` resolves to a
readable JPG/PNG inside the bundle the EPUB embeds `OEBPS/images/cover.*` +
`OEBPS/cover.xhtml` with `properties="cover-image"` and the cover page opens
the spine; `OEBPS/title.xhtml` + `OEBPS/copyright.xhtml` (© year, all-rights
boilerplate, imprint/edition/ISBN, optional AI-assistance disclosure) are
always present ahead of the chapters; the OPF gains `dc:description`
(storefront → `marketing_copy` → `premise`), `dc:subject` (categories →
`genre`), storefront publisher/language. Result payload adds `cover:
{embedded, path?, media?, width?, height?, ratio?, warnings[], hint?}` and
`front_matter: ["title.xhtml", "copyright.xhtml"]` and extends `dc` with
`description, subject, subtitle`.

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
