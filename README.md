# AUTHR_OS Desktop

**The Author's Cockpit** — an industrial, commercial-grade desktop workstation for
professional authors, novelists, scriptwriters, and audio dramatists.

Architecture: **Tauri v2 + Rust Core Runtime + React 19 / TypeScript / Vite /
Tailwind CSS / Meta's Lexical AST** under a Zero-Double-Work paradigm.

| Component | Status |
|-----------|--------|
| Phase 1 — Foundation & Storage Core | ✅ complete (schema §6.1, bundle lifecycle, security, CLI) |
| Phase 2 — Desktop Shell plumbing | ✅ `#[tauri::command]` pass-through wired to the frozen dispatcher (`gui` feature) |
| Phase 3 — Author's Cockpit & Lexical AST Canvas | ✅ 3-pane cockpit, triple-view canvas, 4 restored studios |
| Phase 4 — Lookahead TTS & 3-Layer DSP Rack | ✅ content-addressable audio cache, Edge/ElevenLabs/Cartesia/OpenAI/HF-MMS connectors, DAG lookahead scheduler, IPA phonetics, sidechain ducking, EBU R128/BS.1770 mastering, ACX validation |
| Phase 5 — Local-First AI & Vector RAG | ✅ hashing-384 embedder (+ONNX seam), dense/FTS5/hybrid-RRF retrieval, `rag_chunks` store, offline grammar LLM + llama-server client (`cache_prompt`, `keep_alive:-1`), Kokoro-82M availability path |
| Phase 6 — Publishing & Hardening | ⏳ next phase (per build plan §8) |

## Layout

```
AUTHR_OS/
├── docs/CLI_CONTRACT.md      # FROZEN 48-command IPC surface v1.1 (all transports obey)
├── docs/LOCAL_AI.md          # Phase-5 extension points (ort / llama.cpp / models/)
├── src-tauri/                # Rust core runtime
│   └── src/
│       ├── error.rs          # 11-code taxonomy (storage/bundle/security/audio/ai)
│       ├── security/         # OS keychain (+ mock), path sandbox, zip-slip
│       ├── storage/          # SQLite schema, entity CRUD, git2, cleaner
│       ├── bundle/           # manifest v1, .authr_lock lease, atomic zip export/import
│       ├── shadow.rs         # manuscript/chNN.md + chNN.ast.json renderer
│       ├── audio/            # Phase 4: cache, TTS engines, DAG scheduler, G2P,
│       │                     #          3-layer mixer, ducking, BS.1770 master, ACX
│       ├── rag/              # Phase 5: embedder, chunk store, hybrid search
│       ├── localai/          # Phase 5: offline LLM + llama-server client, Kokoro path
│       ├── ipc/              # dispatcher — CLI, Tauri and dev bridge share it
│       ├── shell/            # Tauri v2 window builder (feature = "gui")
│       └── main.rs           # `authr` CLI (single-line JSON envelope)
├── src/                      # React 19 frontend (Author's Cockpit)
│   ├── components/layout/    # 3-pane cockpit, Project Binder, Dynamic Inspector
│   ├── components/editor/    # Lexical canvas, custom AST nodes, view modes
│   ├── components/studios/   # Audio / Video / Localization / Marketing
│   ├── stores/               # Zustand domain stores
│   └── ipc/                  # typed contract surface + transport bridge
├── tests/                    # 24 modules · 149 E2E tests (pytest + run_e2e.py)
└── scripts/ipc-proxy.mjs     # browser dev bridge: POST /api/ipc → CLI
```

## Build & Run

### 1. Core runtime (Rust)

```bash
cd src-tauri
cargo build --release          # CLI: target/release/authr
cargo test --release           # unit tests
```

Desktop shell (needs webkit2gtk / Xcode CLT / MSVC):

```bash
cargo build --release --features gui,os-keychain
```

### 2. Frontend (Author's Cockpit)

```bash
npm install
npm run build                  # tsc + vite build → dist/
npm run dev                    # http://localhost:5173
```

`npm run dev` is **fully functional in a plain browser**: the vite middleware
(`scripts/ipc-proxy.mjs`) spawns the real `authr` CLI and relays the JSON
envelope, so Lexical edits, binder trees and studio tables drive the actual
Rust core. No Tauri required until packaging.

### 3. E2E test suite (149 tests)

```bash
python tests/run_e2e.py                 # all 4 tiers
python tests/run_e2e.py --json          # machine-readable summary
python tests/run_e2e.py --tier=2        # boundary vectors only
python tests/run_e2e.py --feature=fts5_triggers
python -m pytest tests -v               # pytest directly
```

Headless environments: the suite sets `AUTHR_MOCK_KEYCHAIN=1`
(file-backed credential store) automatically.

## Contract

Every transport — `#[tauri::command]`, the CLI, and the browser dev bridge —
dispatches through one frozen surface: **`docs/CLI_CONTRACT.md`**
(v1.1: 48 commands · `{ok,data}` / `{ok:false,error{code,message,category}}` envelopes ·
11 canonical error codes across 5 categories · 120 s lease TTL · shadow-format &
git-message rules). The 9 v1 codes and 35 v1 commands are untouched; v1.1 adds
the Phase 4/5 studio surface additively. Changes require a contract version bump.
