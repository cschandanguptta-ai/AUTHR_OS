# system-map.md — components, data flow, module ownership

All paths relative to repo root. Line numbers verified at contract v1.2 / git `f57f0d0`.

## 1. Big picture

```mermaid
flowchart LR
    subgraph Clients
        UI["React 19 frontend<br/>src/ (Lexical canvas, 4 studios,<br/>8 Zustand stores)"]
        CLI["CLI users / CI scripts"]
    end

    subgraph Transports
        DEV["Browser dev bridge<br/>scripts/ipc-proxy.mjs<br/>POST /api/ipc → spawns CLI"]
        TAURI["Tauri shell (feature gui)<br/>shell/mod.rs authr_command"]
        BIN["authr binary<br/>main.rs (clap)"]
    end

    DISP["FROZEN DISPATCHER<br/>ipc/mod.rs::dispatch<br/>65 commands · WRITER_COMMANDS lease gate"]

    subgraph Core["Rust core (crate authr)"]
        LIFE["bundle/: manifest v1,<br/>.authr_lock lease (120s),<br/>zip export/import"]
        SEC["security/: sandbox paths,<br/>keychain (OS + mock)"]
        ST["storage/: SQLite (WAL)<br/>6 tables + FTS5, git2,<br/>cleaner"]
        SH["shadow.rs: manuscript/<br/>chNN.md + chNN.ast.json"]
        AUD["audio/: cache → tts engines →<br/>scheduler DAG → mixer → dsp →<br/>master → wav → g2p"]
        RAG["rag/: embed → index →<br/>store (rag_chunks) → search RRF"]
        AI["localai/: llm (offline grammar /<br/>llama-server HTTP), tts (Kokoro)"]
        PUB["publish/: epub, docx, typst,<br/>captions, m4b, release (ed25519),<br/>bench"]
    end

    FS[("`.authr` bundle dir:<br/>project.db · manifest.json ·<br/>manuscript/ · audio/segments/<br/>exports/ · .git/ · models/")]

    UI -->|"invoke(name,args)<br/>src/ipc/bridge.ts"| DEV
    UI -->|"__TAURI_INTERNALS__"| TAURI
    CLI --> BIN
    DEV --> BIN
    BIN --> DISP
    TAURI --> DISP
    DISP --> LIFE & SEC & ST & AUD & RAG & AI & PUB
    ST <--> FS
    SH <--> FS
    AUD <--> FS
    PUB <--> FS
    RAG <--> FS
    LIFE <--> FS
    SEC <--> FS
```

External services (optional, never in CI): Edge TTS WebSocket, ElevenLabs /
Cartesia / OpenAI / HuggingFace REST (`audio/tts.rs:326-373`), llama-server
HTTP (`localai/llm.rs:179`), external binaries `ffmpeg` (`publish/m4b.rs:139`)
and `typst` (`publish/typst.rs:29-32`). API keys come from env only.

## 2. Module table

| Path | Responsibility | Depends on | Depended on by |
|------|----------------|------------|----------------|
| `src-tauri/src/main.rs` | CLI entry: clap parse, `--contract`, envelope emit, exit codes | `ipc`, `lib.rs` consts | (binary entry) |
| `src-tauri/src/lib.rs` | Crate root; `CONTRACT_VERSION="1.2"`, `COMMANDS` (56) list | all modules | `main.rs`, `shell/`, tests |
| `src-tauri/src/ipc/mod.rs` | `Ctx` (open bundle, lease, cloud guard), `WRITER_COMMANDS`, `dispatch` (56) | `bundle`, `storage`, all handlers | `main.rs`, `shell/`, every handler |
| `src-tauri/src/ipc/args.rs` | typed arg extraction (`req_str`, `req_i64`, `get_f64`…) | — | every handler |
| `src-tauri/src/ipc/handlers.rs` | keychain_* + `sandbox_normalize` | `security/keychain`, `security/sandbox` | `ipc/mod.rs` |
| `src-tauri/src/ipc/studio.rs` | Phase 4/5 handlers: audio_* (6), rag_* (3), local_* (4); synthesis pipeline incl. segment stamping | `audio/*`, `rag/*`, `localai/*`, `Ctx` | `ipc/mod.rs` |
| `src-tauri/src/error.rs` | `AppError` — 13 codes / 5 categories; `From<rusqlite/io/json>` mapping | — | everything |
| `src-tauri/src/bundle/manifest.rs` | manifest.json v1 read/write/touch, `atomic_write`, `AUTHR_VERSION` | `error` | lifecycle, `Ctx::after_commit`, epub/docx |
| `src-tauri/src/bundle/lifecycle.rs` | project_init/open/close/lock_status; cloud-sync guard wiring | `manifest`, `lock`, `db`, `git`, `sandbox` | `ipc/mod.rs` |
| `src-tauri/src/bundle/lock.rs` | `.authr_lock` lease: 120s TTL, PID liveness, stale recovery, read-only classify | `error`, `chrono`, `whoami` | lifecycle, `Ctx::open`, `ensure_writable` |
| `src-tauri/src/bundle/export.rs` | zip export (excludes lock/WAL/git) + guarded import | `manifest`, `sandbox` | `ipc/mod.rs` |
| `src-tauri/src/security/sandbox.rs` | path normalize/percent-decode, `resolve_in_root`, zip-slip, cloud-dir heuristic | `error` | studio, export, lifecycle, publish/*, tests |
| `src-tauri/src/security/keychain.rs` | `KeyringBackend` trait; OS keyring (`os-keychain` feature) vs mock file store | `error`, `keyring` (optional) | `ipc/handlers.rs` |
| `src-tauri/src/storage/db.rs` | frozen `SCHEMA_SQL` (6 tables + FTS5 + triggers), PRAGMAs, `open_db`, narrator seed, `row_json`, col projections | `error`, `rusqlite` | all storage modules, `Ctx::open` |
| `src-tauri/src/storage/books.rs` / `chapters.rs` / `segments.rs` / `characters.rs` / `lorebook.rs` / `threads.rs` | entity CRUD + row→JSON | `db`, `error`; chapters→`shadow`; segments→`shadow`,`git` | `ipc/mod.rs`, `publish/mod.rs`, `rag/index.rs` |
| `src-tauri/src/storage/segments.rs` | segment CRUD, FTS BM25 `segment_search`, `apply_mutation` (BEGIN IMMEDIATE AST bridge) | `db`, `chapters`, `shadow`, `git` | `ipc/mod.rs`, `publish` |
| `src-tauri/src/storage/git.rs` | embedded repo init, `.gitignore`, `commit_all` (skips clean trees), `Auto-sync chNN` | `git2`, `Ctx` | lifecycle, segments, chapters(delete side), `ipc/mod.rs` |
| `src-tauri/src/storage/cleaner.rs` | `clean_sweep`: orphan purge + FTS rebuild | `db` | `ipc/mod.rs` |
| `src-tauri/src/shadow.rs` | renders `manuscript/chNN.md` + `chNN.ast.json` (atomic tmp+rename); delete removal | `db`, `Ctx` | chapters, segments, cleaner-adjacent |
| `src-tauri/src/audio/cache.rs` | SHA-256 content-addressable WAV cache + hit/miss stats (`audio/segments/`) | `wav`, `error` | studio (synthesize), scheduler (key) |
| `src-tauri/src/audio/tts.rs` | 7 engines: SCRIPTED + EDGE_TTS (WebSocket, Sec-MS-GEC token) + 4 REST + Kokoro hook; injectable WS/HTTP transports; mp3/wav decode | `error`, `sha2`, `ureq`, `tungstenite`, `symphonia` | studio, localai/tts |
| `src-tauri/src/audio/scheduler.rs` | DAG lookahead: same-speaker chaining, Kahn waves, longest-first order | `cache` (key preview), `error` | studio |
| `src-tauri/src/audio/mixer.rs` | layer parse (sandbox-resolved), resample, 3-layer mix + sidechain duck | `dsp`, `sandbox`, `wav` | studio |
| `src-tauri/src/audio/dsp.rs` | biquads, K-weighted LUFS, RMS/peak/noise, `sidechain_duck`, `lookahead_limiter` | — | mixer, master |
| `src-tauri/src/audio/master.rs` | two-pass normalize to −20 dB RMS, limiter −3.1 dBFS, ACX report | `dsp` | studio |
| `src-tauri/src/audio/g2p.rs` | proper-noun IPA (lexicon + rules) | — | tts, studio, localai/tts |
| `src-tauri/src/audio/wav.rs` | 16-bit PCM WAV read/write | `hound` | cache, mixer, studio, m4b |
| `src-tauri/src/rag/embed.rs` | `Embedder` trait; `HashingEmbedder` (FNV-1a, 384-dim, L2); `resolve("minilm-onnx")` seam; cosine; pack/unpack | `error` | index, search, studio |
| `src-tauri/src/rag/store.rs` | idempotent `rag_chunks` + `rag_chunks_fts` migration; upsert/count/clear; sanitized FTS | `error` | index, search |
| `src-tauri/src/rag/index.rs` | SOURCES = lorebook/characters/chapters; chunking + embedding write | `store`, `embed` | studio |
| `src-tauri/src/rag/search.rs` | dense cosine scan, FTS BM25, hybrid RRF (k=60) | `store`, `embed` | studio |
| `src-tauri/src/localai/llm.rs` | `OFFLINE_GRAMMAR` (seeded SplitMix64) + llama-server HTTP client (`cache_prompt`, `keep_alive:-1`) | `error`, `ureq` | studio |
| `src-tauri/src/localai/tts.rs` | Kokoro-82M availability negotiation (`models/kokoro-v1.0.onnx`), phonemize | `g2p`, `error` | studio |
| `src-tauri/src/publish/mod.rs` | shared: `load_book`, `cast_names`, duration estimator (chars×55+400ms), `cumulative_starts`, `slugify`, `xml_escape`, `exports_path` | storage, `sandbox` | all publish/* |
| `src-tauri/src/publish/epub.rs` | EPUB 3 OCF + SMIL overlays + cached audio | `mod`, `zip` | `ipc/mod.rs` |
| `src-tauri/src/publish/docx.rs` | WordprocessingML manuscript | `mod`, `zip` | `ipc/mod.rs` |
| `src-tauri/src/publish/typst.rs` | `.typ` A5 source + optional PDF via typst binary | `mod` | `ipc/mod.rs` |
| `src-tauri/src/publish/captions.rs` | `.lrc` / `.vtt` from segment durations | `mod` | `ipc/mod.rs` |
| `src-tauri/src/publish/m4b.rs` | ffmpeg concat → AAC → M4B + chapter atoms | `mod`, `wav`, `sandbox` | `ipc/mod.rs` |
| `src-tauri/src/publish/release.rs` | ed25519 sign/verify, Tauri-updater `latest.json` | `error`, `ed25519-dalek` | `ipc/mod.rs` |
| `src-tauri/src/publish/bench.rs` | cold-start (self-spawn) / RSS (VmHWM) / binary size gates | `error` | `ipc/mod.rs` |
| `src-tauri/src/shell/mod.rs` | Tauri v2 window + ONE pass-through command `authr_command` | `ipc` | (gui feature only) |
| `src/ipc/bridge.ts` | transport selection: Tauri (`__TAURI_INTERNALS__`) vs `POST /api/ipc`; `IpcError` | `@tauri-apps/api` (dynamic) | `api.ts` |
| `src/ipc/api.ts` | 33 typed wrapper functions = the only sanctioned call surface | `bridge`, `types/ast` | all stores/components |
| `src/stores/*.ts` (8) | Zustand domain state (project, manuscript, threads, lore, voiceCast, settings, ui, workspace) | `api.ts` | components |
| `scripts/ipc-proxy.mjs` | vite middleware: spawns `authr` CLI, relays envelope as HTTP 200/400; tracks current bundle | child_process | `vite.config.ts` |
| `tests/` | 30 pytest modules, 4 tiers + `run_e2e.py` + 8 helper harnesses | compiled `authr` binary | CI/manual |

## 3. Entry points

| Entry | File | Notes |
|-------|------|-------|
| CLI binary | `src-tauri/src/main.rs:38` `fn main` | `authr <command> [--bundle D] [--args JSON]`, `--contract` prints surface |
| Desktop shell | `src-tauri/src/shell/mod.rs:23` `run()` | compiled only with `--features gui` |
| Library | `src-tauri/src/lib.rs` | `pub mod` everything so tests/shell reuse the dispatcher |
| Dev server | `npm run dev` → `vite.config.ts` → `scripts/ipc-proxy.mjs` | default bundle `<repo>/sandbox.authr`; `AUTHR_CLI` overrides binary |
| Test runner | `tests/run_e2e.py`, `tests/conftest.py` | binary path from `AUTHR_BIN` or `src-tauri/target/release/authr` |

## 4. Background jobs / external services

- **No daemons, no background threads** — every command is a synchronous
  one-shot process; "jobs" are just commands (`benchmark_run` self-spawns the
  binary n times, `publish/bench.rs:27`).
- External services are opt-in per call: network TTS engines, llama-server.
  Missing keys/network map to `AUDIO_ENGINE_UNAVAILABLE` / `AI_ENGINE_UNAVAILABLE`
  (`audio/tts.rs:82-106`, `localai/llm.rs:322` test evidence).
- External binaries: `ffmpeg` (M4B only), `typst` (PDF only). Both degrade to
  `BUNDLE_ERROR` / `compiled:false` with guidance when absent.
