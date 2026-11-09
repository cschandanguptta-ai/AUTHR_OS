# invariants-and-gotchas.md — things that must always stay true

Labels: **FACT** = seen in code at the cited location; **INFERRED** = derived,
needs no human but is not literal code text. Past bugs are at §5.

## 1. Hard invariants (breaking any of these breaks the contract)

1. **One-line JSON envelope, exit 0/1.** `main.rs:32-36,86-95`. Any
   `println!`/`eprintln!` added to a command path corrupts the envelope for
   CLI *and* the dev bridge. FACT.
2. **`after_commit()` before shadow before git.** `ipc/mod.rs:43-49` (comment:
   "manifest touch happens BEFORE the shadow/git sync, never after");
   exercised at `segments.rs:44-46`, `segments.rs:213-215`, `chapters.rs:31-32,96-98`,
   `studio.rs:290,353,396`. Reordering breaks the cloud-sync WAL guarantee and
   commit content. FACT.
3. **Audio cache key formula is load-bearing identity**:
   `sha256(engine|voice|sample_rate|ipa|text)` (`cache.rs:37-50`). Changing the
   formula invalidates every existing cache silently (old chunks orphan, stats
   still count them). It must remain a pure function of those 5 fields — no
   timestamps. FACT.
4. **Synthesis must stamp segment rows in ALL non-dry-run branches, including
   cache hits** (`studio.rs:262-274`). This was a real bug (§5 B-1). Any new
   early-return in `synthesize_chapter` must preserve the stamping. FACT.
5. **`apply_mutation` audio invalidation is decided in Rust, not SQL**
   (`segments.rs:163-172`): SQLite evaluates later SET expressions against
   values already assigned earlier in the same statement, so a `CASE WHEN
   text_content IS ?` inside one UPDATE is unsafe. Keep the two-variant SQL
   string. FACT (comment in code).
6. **`reindex_in_tx` two-phase shift** (`segments.rs:221-235`): first
   `+1000000`, then renumber by rowid order. Collapses to one phase ⇒ trips
   `UNIQUE(chapter_id, sequence_order)` mid-update. FACT.
7. **Every bundle-relative file read/write goes through the sandbox**:
   `resolve_in_root` (`sandbox.rs:119-169`, rejects `..`, absolute/Prefix
   components, symlink components, canonicalize-escape) for reads/inputs;
   `slugify`+`exports_path` (`publish/mod.rs:107,150`) or
   `out_path_in_exports` (`studio.rs:474-491`) for outputs. FACT.
8. **Closed error taxonomy** — 13 codes (9 v1 + 4 v1.1), 5 categories (`error.rs`,
   contract §3). New failure modes reuse an existing code (Phase 6 deliberately
   reused `BUNDLE_ERROR` for publishing; `CLI_CONTRACT.md:197-199`). FACT.
9. **`WRITER_COMMANDS` completeness** (`ipc/mod.rs:53-78`): every command that
   mutates bundle state must be listed, or it will happily write to a
   read-only-leased bundle. FACT.
10. **`lib.rs::COMMANDS` == `shell` == `api.ts` == contract §4.** `test_ip1`
    asserts the surface via `authr --contract`. FACT.
11. **Frozen core DDL** (`db.rs:10-144`, header comment "Do not edit"; also
    `CLI_CONTRACT.md:186-189`). Only `rag_chunks`/`rag_chunks_fts` may be
    created additively (`rag/store.rs:12-35`). FACT.
12. **Git identity and messages are contract-locked** (§8): signature
    `AUTHR_OS <authr@local>` (`git.rs:9-10`), `Auto-sync chNN`,
    `chore: initial scaffold`, `chore: manual snapshot`. FACT.
13. **Export/import exclusion list** (`bundle/export.rs`, contract #5):
    `.authr_lock`, `project.db-wal`, `project.db-shm`, embedded `.git/` are
    never exported. Consequence (INFERRED): export/import round-trip loses
    revision history by design.
14. **Frontend colors go through the semantic palette — never hardcoded
    Tailwind spectrum classes** (`tailwind.config.js`, `index.css` theme
    blocks). `text-emerald-300`-style constants are bright-on-dark and vanish
    on the sepia light theme; use `text-ok` / `text-warn` / `text-bad` /
    `text-accent2` / `text-mut` / `text-on-accent` (all var-driven). The
    semantic colors are alpha-capable via the per-theme `--X-rgb` channel
    triples — a *plain* `var(--x)` Tailwind color makes Tailwind silently
    DROP alpha modifiers (`bg-accent/10` emitted no CSS at all before the
    triples existed; 37 usages were silently broken). Emotion-chip hues use
    the `.chip-*` classes (per-theme). FACT (probed: tailwind emits
    `rgb(var(--accent-rgb) / 0.1)`).
15. **App-level shortcuts avoid plain Ctrl/Cmd+B/I** (`useGlobalShortcuts.ts`):
    Lexical owns those (bold/italic). Binder/inspector toggles are
    Ctrl/Cmd+Alt+B/I, studios 1–6, palette Ctrl/Cmd+K. Focus mode must always
    have an exit (floating pill + Escape) — an earlier build stranded users
    in typewriter mode with no visible way out. FACT.

## 2. Ordering & race-condition notes

- **Lease is advisory, not exclusive** (INFERRED from design): SQLite runs in
  WAL with `busy_timeout=5000` (`db.rs:147-154`), so two processes that ignore
  `.authr_lock` can both write; correctness relies on `ensure_writable`
  (`lock.rs:259-271`) refusing foreign fresh leases.
- **PID liveness is `/proc`-based and Linux-only** (`lock.rs:124-141`): on
  non-Linux, a same-host lease is always considered alive while fresh. Porting
  the check requires a platform backend. FACT.
- **`project_export` `out` resolves against the CLI's CWD when relative** —
  bundle exports can therefore land anywhere the process can write (Phase 7
  lesson: a profiler dumped an ~90 MB export into the repo). Scripts must
  pass absolute paths; E2E always does. FACT.
- **Cloud-sync guard** (`is_cloud_sync_path`, `sandbox.rs:211-218`) is a
  substring match on the bundle path (`onedrive`, `dropbox`,
  `google drive`/`_drive`/`googledrive`). A project *named* "MyDropboxNotes"
  false-positives into checkpoint-after-every-write mode. FACT (behavior),
  INFERRED (acceptable).
- **`Manifest::touch` + WAL checkpoint run per write, not per transaction
  batch** — heavy batch imports pay it per command invocation (one process per
  command, so amortized fine). INFERRED.
- **`commit_all` skips clean trees** (`git.rs:54-61`): `auto_sync_commit` and
  `manual_snapshot` return `None`/"nothing to snapshot" when git status is
  empty — callers must treat `commit:null` as success (`segments.rs:215`,
  `ipc/mod.rs:175-179`). FACT.
- **FTS triggers only fire on `UPDATE OF text_content`** (`db.rs:140-143`):
  updates that don't change text leave FTS alone (fine); but a direct SQL
  write outside these triggers would desync — `clean_sweep` rebuilds FTS as
  the recovery path (`cleaner.rs:20-31`). FACT.

## 3. Hidden coupling

- **Duration estimator is single-sourced**: `chars×55+400ms` lives only in
  `audio::scheduler::estimate_duration_ms` (Phase 7, T-3); `audio_plan`, the
  synthesis scheduler, captions, EPUB SMIL and M4B chapter marks all call it.
  Do not re-inline the formula anywhere. FACT.
- **`segment_duration_ms` prefers `audio_duration_ms>0`, else estimator** —
  EPUB overlays, captions and M4B chapter marks therefore *change meaning*
  after a later `audio_synthesize` run (real durations replace estimates).
  Regenerate captions/EPUB after synthesis. FACT (code), INFERRED (ops note).
- **Cache hits cannot replay continuity**: the cache key excludes
  `previous_request_id` (`cache.rs:37`), so a repeated line synthesizes once
  and reuses the stored audio for every future occurrence; the *stored*
  `request_id` is returned on hit (`studio.rs:242`) and re-threaded into
  `prev_request_id` stamps. Acoustic continuity is therefore
  best-effort under caching. FACT (code), INFERRED (semantic).
- **`row_json` renders BLOBs as `"<N bytes>"`** (`db.rs:206`) — selecting
  `rag_chunks.embedding` through any row projection yields that placeholder,
  never data. Use `rag/store.rs::all_chunks` (pack/unpack in `embed.rs:139-153`).
  FACT.
- **`segment_search` FTS input is sanitized** (Phase 7, T-2): both
  `segment_search` and the rag FTS route through
  `rag::store::fts_sanitize` with a bound `MATCH ?1` — hostile input is
  searched literally, never a MATCH syntax error (regression test `test_ft7`).
  FACT.
- **`character_delete` is blocked by FK while segments reference the key**
  (contract #18, `db.rs:101` `ON UPDATE CASCADE` only). Renaming a
  `character_key` cascades into `chapter_segments.speaker_id` and any
  `rag_chunks` with `source=characters` become stale-keyed. FACT/INFERRED.
- **`chapter_delete` participates in full bookkeeping** (Phase 7, T-4):
  after_commit → shadow removal → `chore: manual snapshot` commit; the
  workdir is clean and HEAD records the removal immediately. FACT.
- **Engine spellings are constrained twice**: `Engine::parse` (`tts.rs:54-80`)
  and the `voice_engine` CHECK constraint (`db.rs:58-59`) must accept the same
  set (contract §4.11 note). `SCRIPTED`/`HF_MMS` are valid engine args but NOT
  valid cast `voice_engine` values. FACT.
- **`benchmark_run` measures itself**: spawns `current_exe()` per iteration
  (`bench.rs:27`) — a wrapper script or debug build skews the numbers.
  FACT.
- **`sandbox_normalize` (#34) is bundle-free** — it is a pure self-test of
  `normalize_raw` (`handlers.rs:35-38`); don't wire it to filesystem effects.
  FACT.
- **The keychain mock cannot leak into desktop builds** (Phase 7):
  `security/keychain.rs::backend` selects the OS keyring whenever the
  `os-keychain` feature is compiled; `AUTHR_MOCK_KEYCHAIN=1` only takes
  effect on builds without that feature (headless CI / sandbox). FACT.

## 4. Fragile areas — "do not touch without reading X"

| Area | File | Read first |
|------|------|-----------|
| AST mutation transaction | `storage/segments.rs:95-235` | contract §4.8 split/merge semantics; `change-impact.md` row |
| Lease state machine | `bundle/lock.rs:143-271` | contract §6; `test_t2_stale_lease_recovery.py` |
| Path normalization | `security/sandbox.rs:12-118` | `test_t2_windows_prefixes.py` vectors (verbatim prefixes, percent-encoding) |
| Sec-MS-GEC token math | `audio/tts.rs:172-191` | 5-minute-bucket SHA-256 over Windows file time + `EDGE_TRUSTED_CLIENT_TOKEN`; unit test `gec_token_is_deterministic_and_buckets_5min` |
| Lookahead limiter / LUFS | `audio/dsp.rs:68-253` | BS.1770 K-weighting biquads; `dsp_mastering` tier |
| EPUB SMIL assembly | `publish/epub.rs:64-335` | stored-first `mimetype` ordering, `id="sN"` anchors, `fmt_clock` |
| Shell envelope pass-through | `shell/mod.rs:9-20` | `Result<Value,Value>` maps Err→JSON error object (Tauri rejects non-String errors) |
| Dev-bridge error shape | `src/ipc/bridge.ts:35-46` | HTTP 400 flat `{code,message,category}` vs nested envelope tolerance |

## 5. Past bugs and their fixes (git history)

- **B-1 — Cache-hit segments lost their audio stamps** (fixed in `2ab4eb8`):
  duplicate text as a cache hit returned early without writing
  `audio_hash`/`audio_cache_path`/`audio_duration_ms`/`prev_request_id` to the
  segment row. Fix: stamp in every non-dry-run branch + regression test
  (`studio.rs:262-274` comment documents it). FACT.
- **B-2 — FTS MATCH aliasing** (Phase 1 lesson): `MATCH` must target the real
  table name, no aliases — `segments.rs:72` comment. FACT.
- **B-3 — Manifest touch ordering** (Phase 1 lesson): touching the manifest
  after shadow render made the shadow commit contain a newer manifest than the
  DB write it describes; fixed by `after_commit()` ordering (see §1.2). FACT.
- **B-4 — SQL-side CASE invalidation** (Phase 3 lesson): audio-hash
  invalidation inside a single UPDATE mis-evaluated because of SQLite SET
  evaluation order; fixed Rust-side (`segments.rs:163-172`). FACT.

## 6. Determinism guarantees (CI relies on all of these)

- `SCRIPTED` engine is deterministic per (voice,text) (`tts.rs:108-170`).
- `HashingEmbedder` (FNV-1a feature hashing, 384-dim, L2) is pure
  (`rag/embed.rs:22-102`).
- `OFFLINE_GRAMMAR` LLM is seeded SplitMix64 → same prompt+seed = same output
  (`localai/llm.rs:26-103`).
- Dense RAG scan iterates in pack order; RRF is rank-based (`rag/search.rs`).
- `edge_gec_token` is a pure function of unix seconds (5-min buckets).
  FACT for all — unit tests assert each (`tts.rs:608`, `embed.rs:155+`,
  `llm.rs:278`).
