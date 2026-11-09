# AUTHR_OS Desktop

**The Author's Cockpit** — an industrial, commercial-grade desktop workstation for
professional authors, novelists, scriptwriters, and audio dramatists.

Architecture: **Tauri v2 + Rust Core Runtime + React 19 / TypeScript / Vite /
Tailwind CSS / Meta's Lexical AST** under a Zero-Double-Work paradigm.

> **Run this on a new machine?** Start with **`RUN_ON_LAPTOP.md`** —
> prerequisites, per-OS build/run/installer steps, and what works offline.

| Component | Status |
|-----------|--------|
| Phase 1 — Foundation & Storage Core | ✅ complete (schema §6.1, bundle lifecycle, security, CLI) |
| Phase 2 — Desktop Shell plumbing | ✅ `#[tauri::command]` pass-through wired to the frozen dispatcher (`gui` feature) |
| Phase 3 — Author's Cockpit & Lexical AST Canvas | ✅ 3-pane cockpit, triple-view canvas, 4 restored studios |
| Phase 4 — Lookahead TTS & 3-Layer DSP Rack | ✅ content-addressable audio cache, Edge/ElevenLabs/Cartesia/OpenAI/HF-MMS connectors, DAG lookahead scheduler, IPA phonetics, sidechain ducking, EBU R128/BS.1770 mastering, ACX validation |
| Phase 5 — Local-First AI & Vector RAG | ✅ hashing-384 embedder (+ONNX seam), dense/FTS5/hybrid-RRF retrieval, `rag_chunks` store, offline grammar LLM + llama-server client (`cache_prompt`, `keep_alive:-1`), Kokoro-82M availability path |
| Phase 6 — Publishing & Hardening | ✅ EPUB 3 compiler with SMIL overlays, DOCX manuscript, Typst print engine (.typ + optional PDF), synced .lrc/.vtt captions, ACX M4B audiobook (ffmpeg AAC + chapter atoms), ed25519 release signing/verification, Tauri bundler + updater config, benchmark gates (cold start 1.7 ms / RSS 6.2 MB / binary 10.1 MB) |
| v1.3 — Craft Catalog & Storefront Packaging | ✅ 25 project formats · 37 bestseller-archetype style personas (LLM prompt directives) · 18 plot arcs with beat sheets + deterministic scaffolding · beat progress fused with the Chekhov ledger · KDP compliance scoring (`kdp_check`) and storefront packages (`kdp_package`: metadata.json, KDP-safe description.html, copyright page, launch checklist, royalty projection) — see `docs/GO_TO_MARKET.md` |

## Layout

```
AUTHR_OS/
├── docs/CLI_CONTRACT.md      # FROZEN 62-command IPC surface v1.3 (all transports obey)
├── docs/GO_TO_MARKET.md      # one-person authoring/publishing house playbook (KDP etc.)
├── docs/LOCAL_AI.md          # Phase-5 extension points (ort / llama.cpp / models/)
├── docs/PACKAGING.md         # Phase-6 installer matrix, signing, auto-updater
├── docs/SECURITY_AUDIT.md    # Phase-6 hardening report (credentials, sandbox, IPC)
├── docs/BENCHMARKS.md        # Phase-6 measured thresholds (cold start / RSS / size)
├── src-tauri/                # Rust core runtime
│   └── src/
│       ├── error.rs          # 14-code taxonomy (storage/bundle/security/audio/ai/craft)
│       ├── security/         # OS keychain (+ mock), path sandbox, zip-slip
│       ├── storage/          # SQLite schema, entity CRUD, git2, cleaner
│       ├── bundle/           # manifest v1, .authr_lock lease, atomic zip export/import
│       ├── shadow.rs         # manuscript/chNN.md + chNN.ast.json renderer
│       ├── audio/            # Phase 4: cache, TTS engines, DAG scheduler, G2P,
│       │                     #          3-layer mixer, ducking, BS.1770 master, ACX
│       ├── rag/              # Phase 5: embedder, chunk store, hybrid search
│       ├── localai/          # Phase 5: offline LLM + llama-server client, Kokoro path
│       ├── publish/          # Phase 6: EPUB 3/SMIL, DOCX, Typst, captions, M4B,
│       │                     #          ed25519 release signing, benchmark gates
│       ├── craft/            # v1.3/v1.4: formats/personas/arcs catalogs, beat engine,
│       │                     #          retention kit (SFB-2.0), KDP check + package builder
│       ├── ipc/              # dispatcher — CLI, Tauri and dev bridge share it
│       ├── shell/            # Tauri v2 window builder (feature = "gui")
│       └── main.rs           # `authr` CLI (single-line JSON envelope)
├── src/                      # React 19 frontend (Author's Cockpit)
│   ├── components/layout/    # 3-pane cockpit, Project Binder, Dynamic Inspector
│   ├── components/editor/    # Lexical canvas, custom AST nodes, view modes
│   ├── components/studios/   # Audio / Video / Localization / Marketing / Craft
│   ├── stores/               # Zustand domain stores
│   └── ipc/                  # typed contract surface + transport bridge
├── tests/                    # 34 modules · 195 E2E tests (pytest + run_e2e.py)
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

### 3. E2E test suite (195 tests · 34 modules)

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
(v1.4: 65 commands · `{ok,data}` / `{ok:false,error{code,message,category}}` envelopes ·
14 canonical error codes across 6 categories · 120 s lease TTL · shadow-format &
git-message rules). The 9 v1 codes and 35 v1 commands are untouched; v1.1 adds
the Phase 4/5 studio surface, v1.2 the Phase 6 publishing surface, v1.3 the
craft/KDP surface and v1.4 the retention/serialization surface — all
additively. Changes require a contract version bump.
