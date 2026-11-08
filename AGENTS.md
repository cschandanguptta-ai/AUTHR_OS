# AGENTS.md — AI maintainer guide for AUTHR_OS

## What this app is

AUTHR_OS Desktop is a local-first authoring workstation for novelists and audio
dramatists. A Rust core runtime (`src-tauri/`, crate `authr`) owns a portable
`.authr` bundle directory (SQLite DB + embedded git + manuscript shadow files +
audio cache) and exposes **56 IPC commands** through one frozen dispatcher
(`src-tauri/src/ipc/mod.rs::dispatch`). Three transports share that dispatcher:
the `authr` CLI (single-line JSON envelope, exit 0/1), a Tauri v2 pass-through
command (`shell/mod.rs`), and a browser dev bridge (vite middleware spawning the
CLI, `scripts/ipc-proxy.mjs`). A React 19 / Lexical frontend is a pure client of
the same contract. Audio (TTS → DAG schedule → 3-layer mix → EBU R128 master →
ACX check), vector RAG (hashing embedder + FTS5 + RRF), offline LLM, and
publishing (EPUB 3 + SMIL, DOCX, Typst, .lrc/.vtt, M4B) all run in-core.

**Contract is law**: `docs/CLI_CONTRACT.md` (v1.2). Command names, envelope
shape, the 13 error codes, row shapes, lease and shadow semantics are frozen.
Any change to them requires a contract version bump.

## Doc index — read this when…

| Doc | Read this when… |
|-----|-----------------|
| `docs/CLI_CONTRACT.md` | touching any command, arg, envelope, error code, row shape, lease/shadow/git-message rule |
| `docs/system-map.md` | you need to find which module owns a behavior, or how data flows |
| `docs/change-impact.md` | **before any code change** — blast radius per module, what else breaks |
| `docs/invariants-and-gotchas.md` | **before any code change** — ordering rules, hidden coupling, past bugs |
| `docs/troubleshooting.md` | something broke — symptom → cause → where to look |
| `docs/data-model.md` | touching schema, queries, migrations, or row projections |
| `docs/api-contracts.md` | adding/changing commands or transports; frontend `src/ipc/` work |
| `docs/config-and-env.md` | env vars, feature flags, build features, external binaries |
| `docs/testing.md` | running/adding tests; verifying a change safely |
| `docs/tech-debt.md` | planning refactors; before "improving" something odd-looking |
| `docs/open-questions.md` | you can answer an UNKNOWN for a human, or hit one |
| `docs/decisions/ADR-*.md` | before changing an architectural decision listed there |
| `docs/LOCAL_AI.md` | RAG/LLM/Kokoro backends, ONNX seams, llama-server protocol |
| `docs/PACKAGING.md` | installers, release signing, updater, Golden Master checklist |
| `docs/SECURITY_AUDIT.md` | credential handling, sandbox guarantees, threat model evidence |
| `docs/BENCHMARKS.md` | perf thresholds enforced by `benchmark_run` (cold start / RSS / binary) |

## Hard rules

1. **Never edit `SCHEMA_SQL` in `src-tauri/src/storage/db.rs:10-144`** (frozen
   build-plan §6.1). Additive migrations only — see `docs/data-model.md`.
2. **Never rename/remove a command, error code, or envelope field** without a
   contract version bump in `lib.rs:18` + `docs/CLI_CONTRACT.md`.
3. **Never write bundle files without going through
   `security/sandbox.rs::resolve_in_root` / `slugify` + `exports_path`**
   (`publish/mod.rs:107,150`). No `std::fs` paths built from raw user input.
4. **Never add a mutating command without adding it to `WRITER_COMMANDS`**
   (`src-tauri/src/ipc/mod.rs:53-78`) — otherwise it silently bypasses the
   read-only lease guard.
5. Keep the write pipeline order: `after_commit()` (manifest touch + cloud WAL
   checkpoint) **before** shadow render **before** git commit
   (`ipc/mod.rs:43-49`, `storage/segments.rs:42-46`).
6. Never contact network TTS/LLM engines in tests: `AUTHR_TTS_OFFLINE=1` +
   `AUTHR_MOCK_KEYCHAIN=1` are mandatory in headless runs (`tests/conftest.py:42-48`).
7. One JSON line on stdout, exit 0/1 — no eprintln!/println! debugging in
   command paths (breaks the envelope; `main.rs:32-36`).
8. Frontend goes through `src/ipc/api.ts` only — never call `invoke()` from
   components/stores directly.

## Commands

```bash
# Rust core (toolchain: see docs/testing.md §Toolchain)
cd src-tauri && cargo build --release          # → target/release/authr
cd src-tauri && cargo test --release           # 67 unit tests
cargo build --release --features gui,os-keychain   # desktop shell

# Frontend
npm install
npm run build        # tsc --noEmit + vite build → dist/
npm run typecheck    # tsc only
npm run dev          # http://localhost:5173 (browser dev bridge, no Tauri needed)

# E2E suite (runs the compiled binary; no cargo needed)
AUTHR_MOCK_KEYCHAIN=1 python3 -m pytest tests/ -q
python3 tests/run_e2e.py --json                # official runner (4 tiers)
python3 tests/run_e2e.py --tier=2 --bail
python3 tests/run_e2e.py --feature=fts5_triggers

# Smoke a command by hand
./src-tauri/target/release/authr --contract
./src-tauri/target/release/authr <command> --bundle <dir> --args '<json>'

# Perf gates (must stay pass:true)
./src-tauri/target/release/authr benchmark_run --args '{}'
```

There is no linter config in-repo; `cargo build --release` with zero warnings
is the Rust bar, `npm run typecheck` the TypeScript bar. Schema check:
`python3 tests/run_e2e.py --feature=sqlite_schema` (there is no separate
migrate step — schema is created/migrated idempotently on every `open_db`).

## Workflow (mandatory)

**Before changing code**: read `docs/change-impact.md` for the module you touch
and `docs/invariants-and-gotchas.md`. If the change touches the command
surface, re-read `docs/CLI_CONTRACT.md` §2–§5 first.

**After changing code**:
1. Re-run the impacted rows of the test matrix from `docs/change-impact.md`
   (minimum: `cargo test --release` + full `pytest tests/ -q`).
2. Update the affected docs in the same change: `change-impact.md` rows,
   `invariants-and-gotchas.md` (new invariants), `troubleshooting.md` (if you
   debugged something non-obvious, add the symptom+fix), `CLI_CONTRACT.md`
   (only with version bump), `testing.md` (new test modules).
3. If you answered an entry in `docs/open-questions.md`, delete it there and
   write the answer into the relevant doc.
4. Keep `README.md` counts (commands, tests, contract version) truthful.
