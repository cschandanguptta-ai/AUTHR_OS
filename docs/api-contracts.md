# api-contracts.md — the 56-command IPC surface in practice

Authoritative spec: `docs/CLI_CONTRACT.md` (v1.2, FROZEN sections). This file
maps the contract onto code and consumers; it does not restate every arg.
Command list constant: `src-tauri/src/lib.rs:22-92` (`COMMANDS`, 56).
Dispatcher: `src-tauri/src/ipc/mod.rs::dispatch:81-210`.

## 1. Transports

| Transport | Form | Code | Consumers |
|---|---|---|---|
| CLI | `authr <command> [--bundle D] [--args JSON]`; one JSON line, exit 0/1 | `main.rs:38-96` | tests (`conftest.py::Cli`), dev bridge, scripts |
| Tauri IPC | single generic command `authr_command(command, bundle, args)` returning `{ok,data}` or throwing the error object | `shell/mod.rs:9-20` | frontend when `window.__TAURI_INTERNALS__` exists |
| Browser dev bridge | `POST /api/ipc {command, args, bundle}` → 200 `{ok:true,data}` / 400 flat error / 500 if binary missing | `scripts/ipc-proxy.mjs` | frontend in plain-browser dev |

Frontend selection logic: `src/ipc/bridge.ts:48-79`. Known gap on the Tauri
path: `bridge.ts` invokes the contract name directly (e.g. `project_init`)
while the shell registers only `authr_command` — see `tech-debt.md T-1` and
`open-questions.md Q-2`.

**Bundle-free commands** (no `--bundle`): `project_init`, `project_import`,
`keychain_*`, `sandbox_normalize`, `release_manifest`, `release_verify`,
`benchmark_run` (`ipc/mod.rs:83-98`). `project_open` accepts `args.path` or
`--bundle` (`lifecycle.rs:75-82`). Everything else is bundle-scoped.

## 2. Envelope & errors

- Success: `{"ok":true,"data":…}`; failure: `{"ok":false,"error":{code,message,category}}`
  (`main.rs:86-95`, `error.rs::to_json:91-93`).
- 13 codes / 5 categories (`error.rs`; 9 v1 + 4 v1.1): storage `ENTITY_NOT_FOUND`,
  `FOREIGN_KEY_VIOLATION`, `STORAGE_ERROR`; bundle `LEASE_LOCK_ACTIVE`,
  `READ_ONLY_PROJECT`, `BUNDLE_ERROR`; security `PATH_TRAVERSAL_VIOLATION`,
  `ZIP_SLIP_VIOLATION`, `SECURITY_ERROR`; audio `AUDIO_ERROR`,
  `AUDIO_ENGINE_UNAVAILABLE`; ai `AI_ERROR`, `AI_ENGINE_UNAVAILABLE`.
- rusqlite FK failures auto-map to `FOREIGN_KEY_VIOLATION`
  (`error.rs:104-119`).
- Unknown command → `BUNDLE_ERROR` "unknown command … see docs/CLI_CONTRACT.md §4"
  (`ipc/mod.rs:206-208`).

## 3. Command groups → handlers (quick map)

| Group | Commands | Handler module | Writer? |
|---|---|---|---|
| Project lifecycle #1–6 | project_init/open/close/lock_status/export/import | `bundle/lifecycle.rs`, `bundle/export.rs` | open(implicit)/close; export stages zip |
| Books #7–11 | book_create/get/list/update/delete | `storage/books.rs` | create/update/delete |
| Chapters #12–16 | chapter_create/get/list/update/delete | `storage/chapters.rs` | create/update/delete |
| Voice cast #17–19 | character_list/upsert/delete | `storage/characters.rs` | upsert/delete |
| Lorebook #20–22 | lore_list/upsert/delete | `storage/lorebook.rs` | upsert/delete |
| Threads #23–25 | thread_open/list/resolve | `storage/threads.rs` | open/resolve |
| Segments #26–28 | segment_append/list/search | `storage/segments.rs` | append |
| Editor bridge #29 | sync_lexical_ast_mutation | `storage/segments.rs::apply_mutation:98` | yes |
| Keychain #30–32 | keychain_store/get/delete | `ipc/handlers.rs` | n/a (no bundle) |
| Maintenance #33–35 | git_snapshot, clean_sweep, sandbox_normalize | `storage/git.rs:89`, `storage/cleaner.rs:7`, `ipc/handlers.rs:38` | snapshot/sweep |
| Audio studio #36–41 | audio_cache_stats/pronounce/plan/synthesize/master/validate | `ipc/studio.rs:17-373` | synthesize, master |
| RAG + local AI #42–48 | rag_index/search/status, local_llm_generate/status, local_tts_generate, local_ai_status | `ipc/studio.rs:380-460` | rag_index |
| Publishing #49–53 | publish_epub/docx/typst/captions/m4b | `publish/*.rs` | all 5 |
| Release & bench #54–56 | release_manifest, release_verify, benchmark_run | `publish/release.rs:45,107`, `publish/bench.rs:15` | n/a (no bundle) |

`WRITER_COMMANDS` (lease-gated): `ipc/mod.rs:53-78` — 24 entries; every
publish_* is included. Numbering here follows the contract's §4 table.

## 4. Auth model

There is **no user auth** — a local desktop app. Authorization == bundle
ownership by filesystem access, guarded by:
- lease (read-only mount / writer refusal): `bundle/lock.rs`;
- path sandbox: `security/sandbox.rs`;
- keychain for *external* secrets only (`keychain_*`).

The Tauri surface inherits Tauri v2's capability model (none configured in
`tauri.conf.json` — the app exposes one pass-through command).

## 5. Request/response conventions

- Args are a single JSON object; unknown keys ignored; missing optionals use
  defaults (`CLI_CONTRACT.md §1`). Arg extraction helpers: `ipc/args.rs`.
- Row shapes are snake_case exactly as SQLite (§5 of contract; projections at
  `db.rs:214-240`). BLOBs render as `"<N bytes>"` via `row_json` (`db.rs:206`).
- Lists wrap in `{"<name>s":[…]}` (`{"books":[…]}`, `{"segments":[…]}` …).
- Deterministic ordering: chapters by `(book_id, chapter_number)`
  (`chapters.rs:52`), segments by `sequence_order` (`segments.rs:61`), RAG
  hits by fused score with rank maps (`rag/search.rs:104-137`).
- File outputs are always bundle-relative strings (`exports/<name>.wav`,
  `exports/<slug>.epub` …) — resolved through the sandbox before any write.

## 6. Consumers and compatibility

| Consumer | Depends on | Guarded by |
|---|---|---|
| React frontend | `src/ipc/api.ts` (33 typed wrappers) + `types/ast.ts` row types | `npm run typecheck`; manual dev-bridge smoke |
| pytest suite | exact envelope, exit codes, every command's negative codes | `tier1_features/test_t1_ipc_contracts.py` (asserts 56-command surface via `--contract`, `test_ip1`) |
| Tauri updater | `latest.json` shape from `release_manifest` | `release_bench` tests r1–r3 |
| llama.cpp `llama-server` | `/completion` with `cache_prompt:true`, `keep_alive:-1`, optional `grammar` | `localai/llm.rs` unit tests (stub server) |
| Edge TTS service | WebSocket protocol incl. Sec-MS-GEC token (`tts.rs:172-191`) | unit tests with injected transport (no live network in CI) |
| ffmpeg / typst binaries | argv-only invocation, no shell | `m4b.rs::abs_escape:165`, `publish_boundaries` tests |

## 7. Versioning rule

Surface change ⇒ bump `CONTRACT_VERSION` (`lib.rs:18`), append a §4.x block in
`docs/CLI_CONTRACT.md`, add the command to `lib.rs::COMMANDS` + dispatcher +
(if bundle-free) `ipc-proxy.mjs::NO_BUNDLE_COMMANDS`, then extend
`test_t1_ipc_contracts.py`. v1.1 (Phase 4/5) and v1.2 (Phase 6) both extended
additively — no frozen item ever changed. Reuse existing error codes before
inventing new ones (Phase 6 precedent, `CLI_CONTRACT.md:5-7`).
