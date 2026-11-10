# change-impact.md — "If I change X, what else breaks?"

The most important doc. Each row lists the blast radius of a change and the
minimum verification set. `E2E:` names map to
`python3 tests/run_e2e.py --feature=<name>` (`tests/run_e2e.py:22-49`).
Unit = `cargo test --release`. All = full `pytest tests/ -q` (~11 s).

## 1. Change-impact table

| Change area | Blast radius | Files to re-verify | Tests to run |
|---|---|---|---|
| **`storage/db.rs::SCHEMA_SQL`** (tables, columns, CHECKs) | Every command; row projections; shadow renderer; publish loaders; FTS triggers | `db.rs:214-240` col lists, all `storage/*.rs`, `shadow.rs`, `publish/mod.rs:41`, `rag/index.rs` | Unit + E2E: sqlite_schema, indexes, fts5_triggers, schema_constraints, foreign_key_violations, cascade_deletions, ipc_contracts, shadow_markdown, then All |
| **Row projection lists (`BOOK_COLS`…`SEGMENT_COLS`)** | Contract §5 row shapes; frontend `types/ast.ts` typing | `docs/CLI_CONTRACT.md:212-224`, `src/types/ast.ts`, stores | ipc_contracts, sqlite_schema, typecheck |
| **`ipc/mod.rs::WRITER_COMMANDS`** | Lease enforcement; read-only bundles | any new mutating command; `bundle/lock.rs::ensure_writable` | bundle_lifecycle, stale_lease_recovery, publish_boundaries |
| **`ipc/mod.rs::dispatch` (add/rename command)** | 3 transports; `lib.rs::COMMANDS`; contract §4; frontend `api.ts`; `ipc-proxy.mjs` NO_BUNDLE_COMMANDS | `lib.rs:22-92`, `main.rs`, `shell/mod.rs`, `src/ipc/api.ts`, `scripts/ipc-proxy.mjs`, `docs/CLI_CONTRACT.md` | ipc_contracts (asserts exact 62-command surface via `--contract`), then All |
| **`error.rs` (codes, category mapping)** | Envelope §2/§3; every negative test asserts code+category pairing | `docs/CLI_CONTRACT.md:38-62`; `From<rusqlite::Error>` FK mapping `error.rs:104-119` | ipc_contracts, foreign_key_violations, audio_ai_boundaries, publish_boundaries |
| **`security/sandbox.rs::resolve_in_root` / `normalize_raw` / `validate_zip_entry`** | Every file-touching command; import/export; audio master inputs; EPUB/M4B cache reads | `bundle/export.rs`, `ipc/studio.rs` (`out_path_in_exports:474`), `publish/mod.rs::exports_path:150`, `mixer.rs::parse_layer_args_bundle:185` | sandbox, path_traversal, windows_prefixes, zip_slip, publish_boundaries, security rows in ipc_contracts |
| **`bundle/lock.rs` (TTL, classify, liveness)** | open/close, read-only mode, all writers | `bundle/lifecycle.rs:75-109`, `ipc/mod.rs::Ctx::open:25` | bundle_lifecycle, stale_lease_recovery, publish_boundaries (test_pb6) |
| **`bundle/manifest.rs` (`AUTHR_VERSION`, touch, atomic_write)** | init/open/export; epub/docx timestamps | `lifecycle.rs`, `export.rs`, `ipc/mod.rs::after_commit:43` | bundle_lifecycle, ipc_contracts |
| **`bundle/export.rs` (exclusion list, staging)** | project_export/import round-trips | `sandbox.rs::validate_zip_entry` | bundle_lifecycle, zip_slip, ipc_contracts |
| **`audio/cache.rs::key()` formula** (`sha256(engine\|voice\|rate\|ipa\|text)`) | Cache identity across CLI runs; continuity request ids stored in meta | `ipc/studio.rs::synthesize_chapter:238` (key build), `scheduler.rs:133` (continuity preview uses same fn) | audio_surface, tts_engines, audio_rag_scenario; manual: run `audio_synthesize` twice, expect `cache_hits` |
| **`audio/tts.rs` engine set / env keys / `Engine::parse`** | `character_profiles.voice_engine` CHECK (`db.rs:58-59`), contract §4.11 spellings, `synth` routing | `db.rs` CHECK constraint, `localai/tts.rs`, `ipc/studio.rs:106` | tts_engines, audio_surface, audio_ai_boundaries |
| **`audio/scheduler.rs::plan`** (same-speaker chain, Kahn, longest-first) | `audio_plan`, `audio_synthesize` order, EPUB/M4B/caption timeline ordering assumptions | `ipc/studio.rs:196-289`, `publish/mod.rs::cumulative_starts:95` | audio_surface, dsp_mastering, audio_rag_scenario |
| **`audio/mixer.rs` / `dsp.rs` / `master.rs` constants** (duck 18/120/750, target −20.0, ceiling −3.1, LUFS K-weighting) | `audio_master` output; ACX pass/fail; `benchmark` durability claims; contract §4.11 defaults | `ipc/studio.rs::audio_master:302-362`, `docs/BENCHMARKS.md`, contract defaults | dsp_mastering, audio_rag_scenario |
| **Duration estimator** `audio::scheduler::estimate_duration_ms` (single source since Phase 7, T-3) | Captions, EPUB SMIL clip times, M4B chapter marks for unsynthesized segments; `audio_plan` est | `publish/mod.rs::segment_duration_ms`, `ipc/studio.rs` (both call sites) — never re-inline the formula | audio_surface, publish (test pb captions/epub), audio_rag_scenario |
| **`publish/mod.rs::slugify` / `exports_path`** | All 5 publish outputs' filenames; hostile-title safety | `publish/*.rs` out-path calls; `ipc/studio.rs:474` (audio twin) | publish_boundaries (test_pb1 hostile title), publish |
| **`publish/epub.rs` SMIL timeline** | Must stay consistent with `publish_captions` and `publish_m4b` chapter marks (all derive from `cumulative_starts`) | `publish/captions.rs`, `publish/m4b.rs:88`, `publish/mod.rs:95` | publish, publish_boundaries, t4_publish_scenario |
| **`publish/m4b.rs` ffmpeg invocation** | External binary contract (argv only, `abs_escape` quoting) | `SECURITY_AUDIT.md §4` | publish, publish_boundaries (missing-ffmpeg → BUNDLE_ERROR) |
| **`publish/release.rs` (ed25519, `latest.json` shape)** | Updater config `tauri.conf.json:61-68`; PACKAGING docs; key env | `docs/PACKAGING.md`, `tauri.conf.json` pubkey placeholder | release_bench (test_r1–r3) |
| **`publish/bench.rs` thresholds** | `docs/BENCHMARKS.md`, Golden Master gate, `test_b1` | `docs/BENCHMARKS.md` | release_bench |
| **`rag/embed.rs::HashingEmbedder` / `resolve`** | All stored embeddings (BLOB format f32 LE, dim 384); hybrid + dense ranks; `rag_status` | `rag/index.rs`, `rag/search.rs`, `LOCAL_AI.md` | rag, local_ai, audio_rag_scenario |
| **`rag/store.rs` schema / `fts_search` sanitization** | `rag_chunks`/`rag_chunks_fts`; hostile-query safety | `rag/search.rs`, `LOCAL_AI.md §1` | rag, local_ai (fts sanitization tests live in `rag/store.rs` tests) |
| **`localai/llm.rs` offline engine or llama-server protocol** | `local_llm_generate` determinism (seeded SplitMix64); `cache_prompt`/`keep_alive` flags asserted by tests | `docs/LOCAL_AI.md §3` | local_ai, audio_ai_boundaries |
| **`localai/tts.rs` Kokoro negotiation** | `local_tts_generate` error contract (`AI_ENGINE_UNAVAILABLE` + placement guidance) | `docs/LOCAL_AI.md §2` | local_ai |
| **`shadow.rs` markdown/AST format** | Git auto-commit diffs; editor round-trip (`sync_lexical_ast_mutation` consumers); contract §7 | `storage/segments.rs::apply_mutation`, contract §7, `types/ast.ts::NarrativeSegmentAst` | shadow_markdown, git_integration, ipc_contracts |
| **`storage/git.rs` (`GITIGNORE`, signatures, message formats)** | Contract §8; every mutating command's commit behavior; export excludes `.git/` | `bundle/export.rs` (excludes `.git/`), contract §8 | git_integration, shadow_markdown, bundle_lifecycle |
| **`storage/segments.rs::apply_mutation` transaction / reindex two-phase shift** | Editor save path; `UNIQUE(chapter_id, sequence_order)`; audio invalidation | `segments.rs:163-172` (Rust-side invalidation — do not move into SQL), `reindex_in_tx:221` | ipc_contracts, fts5_triggers, shadow_markdown, author_scenarios |
| **`ctx.after_commit()` ordering** (manifest touch → cloud WAL checkpoint → shadow → git) | Cloud-sync correctness (WAL must shrink after every write); manifest timestamps | `ipc/mod.rs:43-49`, every writer handler (`segments.rs:44,213`, `chapters.rs:31,96`, `studio.rs:290,353,396`) | cloud_wal_truncation, ipc_contracts |
| **`security/keychain.rs` backend selection** | `AUTHR_MOCK_KEYCHAIN`/`AUTHR_MOCK_KEYRING_FILE` contract; headless CI. Phase 7 (ADR-0008): the mock is reachable ONLY without the `os-keychain` feature — desktop builds always use the OS keyring | `ipc/handlers.rs`, `tests/conftest.py:42-48` | keychain, All (every fixture depends on the mock) |
| **`src/ipc/bridge.ts` transport logic** | Whole frontend; error surfacing (`IpcError` code/category). Phase 7 (T-1): desktop path routes through `authr_command` + client-side bundle tracking (init/import/open) | `src/ipc/api.ts`, `scripts/ipc-proxy.mjs` (400 shape, stdin switch ≥96 KB, `project_open` tracker), error envelope | typecheck; manual `npm run dev` smoke; dev_bridge verified in test_results.json |
| **`rag/search.rs` parallel scan / `rag/index.rs` parallel embed** (rayon, Phase 7) | Determinism of ranks and insertion order (invariants §6) | `rag/store.rs::all_chunk_blobs/chunks_by_ids`, `embed.rs` (Embedder must stay `Send + Sync` + pure) | rag, local_ai, audio_rag_scenario; unit: `rag::search` tests |
| **`main.rs --args-stdin` / `ipc-proxy.mjs` stdin switch** | CLI transport §1; dev bridge large payloads | `scripts/ipc-proxy.mjs`, contract §1 | ipc_contracts; manual: bulk `sync_lexical_ast_mutation` >96 KB |
| **`scripts/ipc-proxy.mjs`** | Browser dev mode; bundle tracking after init/import | `bridge.ts`, `vite.config.ts` | manual smoke: `npm run dev`, create project, edit, save |
| **`types/ast.ts`** | api.ts, stores, editor nodes | `api.ts`, `stores/manuscriptStore.ts`, Lexical nodes | typecheck + manual smoke |
| **`Cargo.toml` features/deps** | Build matrix (`gui`, `os-keychain`); binary size gate (25 MB) | `bench.rs` gates, `docs/BENCHMARKS.md` | Unit, release_bench (test_b1 binary size) |

## 2. Shared code / shared types hotspots

- **`Ctx`** (`ipc/mod.rs:16-50`) — every bundle-scoped handler receives it.
  Changing `open()` (lease, pragmas, schema ensure) affects ALL commands.
- **`error.rs` From impls** — a new rusqlite error string pattern must be added
  to the FK mapping or it will surface as `STORAGE_ERROR` instead of
  `FOREIGN_KEY_VIOLATION` (negative tests pair code+category strictly).
- **`db::row_json`** renders BLOB columns as `"<N bytes>"` (`db.rs:206`) —
  never select `embedding` through it.
- **Duration estimator** and **slug machinery** are duplicated between
  `publish/mod.rs` and `ipc/studio.rs` — change both or timelines diverge.
- **`lib.rs::COMMANDS`** ↔ `docs/CLI_CONTRACT.md` §4 ↔ `shell/mod.rs` ↔
  `src/ipc/api.ts` must list the same surface; `test_ip1` fails otherwise.
- **Frontend row types** (`src/types/ast.ts`) mirror contract §5 snake_case —
  no camelCase anywhere in envelopes.

## 3. Env vars / feature flags coupling

See `docs/config-and-env.md` for the full table. Coupled pairs:
- `AUTHR_TTS_OFFLINE=1` ↔ every network engine short-circuit (`tts.rs:82`).
- `AUTHR_MOCK_KEYCHAIN=1` ↔ `AUTHR_MOCK_KEYRING_FILE` (mock store path).
- `AUTHR_RELEASE_KEY_HEX` ↔ `release_manifest` ephemeral-key fallback.
- `AUTHR_FFMPEG_BIN` / `AUTHR_TYPST_BIN` ↔ PATH fallbacks in m4b/typst.
- Cargo features `gui` ↔ `tauri`/`tauri-build` deps; `os-keychain` ↔ `keyring`
  dep (`Cargo.toml:15-20`). Default build has neither — keep CI hermetic.

## 4. DB schema coupling

- `chapter_segments.speaker_id` → `character_profiles.character_key`
  (`ON UPDATE CASCADE`, delete RESTRICTED by FK → `FOREIGN_KEY_VIOLATION`,
  contract #18). Renaming a cast key rewrites segment rows silently.
- `unresolved_threads.resolved_in_chapter` → `chapters` `ON DELETE SET NULL`;
  `introduced_in_chapter` `ON DELETE CASCADE`.
- `chapter_segments.chapter_id` / `chapters.book_id` → CASCADE deletes (chapter
  delete wipes segments + threads + FTS rows via triggers + shadow files via
  `chapters.rs::delete:103`).
- `rag_chunks` has **no FK** — keyed `"<source>:<ref_id>"` snapshots; stale
  rows survive source deletion until the next `rag_index` / `clear_source`.

## 5. API contract coupling

Any change in §2 envelope, §3 codes, §4 args/results, §5 row shapes, §6 lease
semantics, §7 shadow format, §8 git messages ⇒ bump `CONTRACT_VERSION`
(`lib.rs:18`), update `docs/CLI_CONTRACT.md`, re-run `ipc_contracts`
(`test_ip1` asserts the exact frozen surface through `authr --contract`).

## 6. Minimum safe-verification recipe

```bash
cd src-tauri && cargo test --release          # 70 unit tests, 0.02 s
AUTHR_MOCK_KEYCHAIN=1 python3 -m pytest tests/ -q   # 170 tests, ~11 s
./target/release/authr benchmark_run --args '{}'    # pass:true
npm run typecheck                              # frontend changes only
python3 scripts/latency_profile.py             # Phase 7: per-command latency baseline
```

## 7. Phase 7 behavior changes (Forensic Integrity Audit, 2026-11-09)

Surface-compatible changes shipped with ADR-0008 (details in
`docs/FORENSIC_AUDIT.md`):

- `chapter_delete` response gains `commit: oid|null` (additive; contract §4.4
  updated) and now performs the standard after_commit + snapshot bookkeeping.
- `segment_search` sanitizes its query (T-2) — previously-erroring FTS syntax
  now returns literal-text results; contract §4.7 note updated.
- `slugify` appends an 8-hex hash suffix ONLY for titles whose slug exceeds
  64 chars (T-11) — short titles and republishes are byte-identical to before.
- `row_json` errors on BLOB columns instead of emitting `"<N bytes>"` (T-5).
- Additive CLI transport `--args-stdin` (contract §1); dev bridge auto-switches
  above 96 KB.
- Keychain mock gating (K-1), bridge `authr_command` routing (T-1), CSP set
  (T-6), `run_e2e.py --feature` aliases (T-8) — see the table rows above and
  the ADR.
