# troubleshooting.md — symptom → cause → where to look → fix

Every AUTHR_OS command answers exactly one JSON line and exit 0/1
(`main.rs:86-95`), so "logs" = stdout (the envelope) + stderr. The dev bridge
relays stdout as HTTP 200 `{ok,data}` / 400 `{ok:false,error}` and answers
500 when the binary is missing (`scripts/ipc-proxy.mjs`). There are no
application log files; `git log` inside the bundle is the history of writes.

## 1. Reading an error envelope

```json
{"ok":false,"error":{"code":"READ_ONLY_PROJECT","message":"…","category":"bundle"}}
```

`code` is one of the 13 canonical codes (`error.rs`, contract §3). `category`
helps route: storage → DB/IO; bundle → manifest/lease/export/publish;
security → path/keychain; audio → args/engines; ai → RAG/LLM.

## 2. Auth / credential failures

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `SECURITY_ERROR` on any `keychain_*` | OS keyring unavailable (headless/no session bus) | `security/keychain.rs::backend:131` | set `AUTHR_MOCK_KEYCHAIN=1`; CI always does (`tests/conftest.py:42-48`) |
| `keychain_get` → `ENTITY_NOT_FOUND` | key never stored, or different keyring file | `handlers.rs:18-25`; check `AUTHR_MOCK_KEYRING_FILE` | store first; use same env in all shells |
| `AUDIO_ENGINE_UNAVAILABLE` mentioning a key name | API key env missing (`ELEVENLABS_API_KEY`, `CARTESIA_API_KEY`, `OPENAI_API_KEY`, `HUGGINGFACE_API_KEY`) | `tts.rs::require_env:92`, `build_request:326-373` | export the key, or use `engine:"SCRIPTED"` / `AUTHR_TTS_OFFLINE=1` semantics |
| `AI_ENGINE_UNAVAILABLE` from `local_llm_generate` | llama-server not running / wrong URL | `localai/llm.rs::llama_server_generate:179`; `local_llm_status` shows `server_configured` | start `llama-server -m model.gguf --port 8080` or pass `server_url`; offline fallback is automatic |

## 3. DB / storage errors

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `FOREIGN_KEY_VIOLATION` on `segment_append` | `speaker_id` not in `character_profiles` | `db.rs:101` FK; `error.rs:104-119` mapping | `character_upsert` first (`NARRATOR` is pre-seeded, `db.rs:180`) |
| `FOREIGN_KEY_VIOLATION` on `character_delete` | segments still reference the cast key (by design, contract #18) | — | delete/reassign segments first |
| `ENTITY_NOT_FOUND` from `audio_plan`/`audio_synthesize` | chapter has no segments, or wrong id | `studio.rs:56-57,79-85` | `segment_append` at least one segment |
| `STORAGE_ERROR: --args parse error` | `--args` not a JSON *object* | `main.rs:65-84` | quote JSON: `--args '{"k":v}'` |
| `STORAGE_ERROR: malformed CLI output` (tests) | something printed to stdout mid-command | any new `println!` — violates AGENTS.md rule 7 | remove; envelope must be the only stdout line |
| DB locked / busy errors | WAL + `busy_timeout=5000` exceeded (long-held tx) | `db.rs:147-154` | keep transactions short; `apply_mutation` is the only BEGIN IMMEDIATE (`segments.rs:113-116`) |
| Stale-looking data after direct SQL | FTS triggers only cover the 3 write paths | `db.rs:131-143` | `clean_sweep` rebuilds FTS (`cleaner.rs:20-31`) |

## 4. Bundle / lease errors

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `LEASE_LOCK_ACTIVE` on `project_open` | fresh lease held by live local PID or foreign host | `lock.rs::classify:153-172`; `project_lock_status` shows holder | wait ≤120 s (TTL, `lock.rs:8`), or close the other session; stale recovery is automatic on open |
| Every write → `READ_ONLY_PROJECT` | foreign fresh lease at open ⇒ read-only mount | `lifecycle.rs:101`, `lock.rs::ensure_writable:259` | release the other session; note `publish_*` are writers too (`ipc/mod.rs:53-78`) |
| `BUNDLE_ERROR: existing .authr bundle` | `project_init` onto an existing bundle | `lifecycle.rs:28-33` | new path, or `project_import` instead |
| `BUNDLE_ERROR: corrupt lease` | `.authr_lock` empty/invalid JSON | `lock.rs:74-78,161` | auto-reclaimed on next open (stale path) — no manual fix needed |
| `BUNDLE_ERROR: book N has no chapters to publish` | publish before writing chapters | `publish/mod.rs:47-51` | create chapters + segments |
| Export missing `.git/` or lock | by design — exclusion list | `bundle/export.rs` (contract #5) | history is intentionally not portable |

## 5. Security errors

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `PATH_TRAVERSAL_VIOLATION` | `..`, absolute path, Windows prefix, null byte, or symlink component in a bundle-relative arg | `sandbox.rs::normalize_raw:12` / `resolve_in_root:119` | pass bundle-relative paths; outputs must be bare file names (`studio.rs:474`, `publish/mod.rs:150`) |
| `ZIP_SLIP_VIOLATION` on `project_import` | malicious/absolute/backslashed archive entry | `sandbox.rs::validate_zip_entry:174` | re-zip with plain relative entries |
| Publish output of hostile title is `etc-passwd.epub` etc. | `slugify` collapses traversal — expected | `publish/mod.rs:107-130`; test_pb1 | not a bug |

## 6. Audio / AI engine failures

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `AUDIO_ENGINE_UNAVAILABLE` with "blocked by AUTHR_TTS_OFFLINE=1" | offline guard (exact string compare `"1"`) | `tts.rs::offline_guard:82-90` | unset the env for real synthesis |
| `AUDIO_ERROR: sample_rate N outside 8000–96000` | arg validation | `studio.rs:110-112,325-327` | use 16000/22050/24000/44100 |
| `AUDIO_ERROR: layers must contain 1–16 entries` | `audio_master` bounds | `studio.rs:307-309` | trim layers |
| `AUDIO_ERROR: output name must end with .wav` | `out` arg is a bare name check | `studio.rs:474-491` | `"out":"mastered.wav"` |
| `AI_ERROR: unknown mode` / `query must not be empty` | rag_search validation | `rag/search.rs:83-89` | mode ∈ dense\|fts\|hybrid |
| `AI_ENGINE_UNAVAILABLE` from `local_tts_generate` | `models/kokoro-v1.0.onnx` missing + no runtime seam | `localai/tts.rs::generate:46-67`; `local_ai_status` shows `kokoro.model_present` | follow the placement guidance in the error; fallback engine for pipeline work is `audio_synthesize engine:SCRIPTED` |
| Ducking "didn't happen" in `audio_master` | no SPEECH layer, or duck=false | `mixer.rs::mix:91`, `dsp.rs::sidechain_duck:147` | include a `SPEECH` layer; `duck:false` disables by design |
| Mastered RMS not exactly −20 | target is two-pass normalize; LUFS ≠ RMS | `master.rs:11-46` | metrics report both (`rms_db`, `lufs`); ACX band is −23…−18 dB RMS |

## 7. Publishing failures

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `BUNDLE_ERROR: ffmpeg not found` | binary absent | `m4b.rs::ffmpeg_bin:139-163` | install ffmpeg or `AUTHR_FFMPEG_BIN=/path` or arg `ffmpeg_bin` |
| `BUNDLE_ERROR` "unsynthesized" segment in M4B | every segment needs a cached WAV | `m4b.rs` checks; contract #51 | `audio_synthesize {chapter_id}` first |
| `publish_typst` → `compiled:false` | no typst binary found | `typst.rs:29-41` (arg → `AUTHR_TYPST_BIN` → PATH) | install typst; `.typ` is always emitted regardless |
| `BUNDLE_ERROR` from typst compile | typst exited non-zero | `typst.rs:47-60` | run `typst compile <out.typ>` manually to see the real error |
| Captions/EPUB timings ≠ audio | segments were re-synthesized after publish | `publish/mod.rs::segment_duration_ms:84` (estimator vs real) | re-run `publish_captions`/`publish_epub` after synthesis |

## 8. Build / test failures

| Symptom | Likely cause | Where to look | Fix |
|---|---|---|---|
| `cargo build` fails on `tauri`/`keyring` | optional features pulled system deps | `Cargo.toml:15-20` | build without features (default is hermetic) |
| tests fail: `core binary missing` | binary not built / stale | `tests/conftest.py:24-39` (`AUTHR_BIN` override) | `cd src-tauri && cargo build --release` |
| Test count differs on your machine (195) | parametrized test (`test_t1_audio_surface.py:75`) counts ×2 | `docs/testing.md` | compare against `run_e2e.py --json` totals, not file counts |
| `test_ip1` (contract surface) fails | `lib.rs::COMMANDS` out of sync with dispatcher/docs | `change-impact.md §5` | reconcile all four lists; bump version if surface changed |
| Frontend build fails | tsc errors in `src/` | `npm run typecheck` | fix types; row shapes must match contract §5 exactly |
| `npm run dev` → HTTP 500 on /api/ipc | CLI binary missing | `scripts/ipc-proxy.mjs::resolveBinary` (`AUTHR_CLI` override) | build the binary or set `AUTHR_CLI` |
| `benchmark_run pass:false` | thresholds (cold 1500 ms / RSS 50 MB / binary 25 MB) exceeded | `bench.rs`, `docs/BENCHMARKS.md` | usually a debug binary or heavy dep added — check `Cargo.toml` diff |

## 9. Reproducing locally & rolling back

- **Reproduce any command**: `authr <command> --bundle <dir> --args '<json>'` —
  headless env: `AUTHR_MOCK_KEYCHAIN=1 AUTHR_TTS_OFFLINE=1`.
- **Reproduce a failing test**: `python3 tests/run_e2e.py --feature=<name>`
  (feature map at `tests/run_e2e.py:22-49`), or
  `python3 -m pytest tests/tier1_features/test_t1_x.py -v`.
- **Roll back bundle content**: every write is a git commit in the embedded
  repo (`manuscript/`, manifest; DB is gitignored) —
  `git -C <bundle> log --oneline`, then restore `manuscript/` from a commit and
  re-run the affected command. Audio cache and DB are **not** versioned
  (`.gitignore`, `git.rs:8`).
- **Roll back code**: repo is a single git history; contract changes must be
  additive (see `change-impact.md §5`).
- **Wipe a corrupt cache**: delete `<bundle>/audio/segments/*` (hashes are
  content-addressed; next `audio_synthesize` rebuilds). Counters live in
  `stats.json` (`cache.rs:52-75`) and may be reset the same way.
