# AUTHR_OS Desktop

**The Author's Cockpit** — an industrial, commercial-grade desktop workstation for
professional authors, novelists, scriptwriters, and audio dramatists.

Architecture: **Tauri v2 + Rust Core Runtime + React 19 / TypeScript / Vite /
Tailwind CSS / Meta's Lexical AST** under a Zero-Double-Work paradigm.

> **Run this on a new machine?** Start with **`RUN_ON_LAPTOP.md`** (all-OS:
> prerequisites, build/run/installer steps, what works offline) — or
> **`RUN_ON_WINDOWS.md`** for the dedicated Windows 10/11 walk-through
> (PowerShell commands, WebView2/MSVC notes, MSI + NSIS installer build).

| Component | Status |
|-----------|--------|
| Phase 1 — Foundation & Storage Core | ✅ complete (schema §6.1, bundle lifecycle, security, CLI) |
| Phase 2 — Desktop Shell plumbing | ✅ `#[tauri::command]` pass-through wired to the frozen dispatcher (`gui` feature) |
| Phase 3 — Author's Cockpit & Lexical AST Canvas | ✅ 3-pane cockpit, triple-view canvas, 4 restored studios |
| Phase 4 — Lookahead TTS & 3-Layer DSP Rack | ✅ content-addressable audio cache, Edge/ElevenLabs/Cartesia/OpenAI/HF-MMS connectors, DAG lookahead scheduler, IPA phonetics, sidechain ducking, EBU R128/BS.1770 mastering, ACX validation |
| Phase 5 — Local-First AI & Vector RAG | ✅ hashing-384 embedder (+ONNX seam), dense/FTS5/hybrid-RRF retrieval, `rag_chunks` store, offline grammar LLM + llama-server client (`cache_prompt`, `keep_alive:-1`), Kokoro-82M availability path |
| Phase 6 — Publishing & Hardening | ✅ EPUB 3 compiler with SMIL overlays, DOCX manuscript, Typst print engine (.typ + optional PDF), synced .lrc/.vtt captions, ACX M4B audiobook (ffmpeg AAC + chapter atoms), ed25519 release signing/verification, Tauri bundler + updater config, benchmark gates (cold start 1.7 ms / RSS 6.2 MB / binary 10.1 MB) |
| v1.3 — Craft Catalog & Storefront Packaging | ✅ 25 project formats · 37 bestseller-archetype style personas (LLM prompt directives) · 18 plot arcs with beat sheets + deterministic scaffolding · beat progress fused with the Chekhov ledger · KDP compliance scoring (`kdp_check`) and storefront packages (`kdp_package`: metadata.json, KDP-safe description.html, copyright page, launch checklist, royalty projection) — see `docs/GO_TO_MARKET.md` |
| v1.4 — Retention & Serialization Kit | ✅ SFB-2.0 as executable craft machinery (`craft_catalog kind=retention`): seven-beat chapter model, six ending types with last-3 rotation blocking, 10–15 chapter micro-arc cycle, 7×0–2 QC rubric with zero-flag rule, A–E drafting worksheet, cadence/buffer arithmetic from the real chapter pipeline — see `docs/GO_TO_MARKET.md` §6 |
| v1.5 — Video Novel Studio | ✅ draft-time media derivation (audio/video/image state computed ONCE on the write path — the no-double-work contract) · deterministic offline SCRIPTED image engine (BMP + embedded 5×7 font + SHA-256-seeded palettes, byte-identical re-renders) · R2I character reference sheets with the consistency ladder (novelcast ADR-0004) · `R2I_ENDPOINT` fail-closed seam (`AUTHR_IMG_ENDPOINT`) · one-pass ffmpeg MP4 (1280×720@30, Ken Burns with persisted focal anchors, narration auto-reused from the audio pipeline, SRT soft-mux) · `video_validate` gate (plan vs registry vs encoded file) — see `docs/decisions/ADR-0011-video-novel-studio.md` |
| v1.6 — Provider Settings Ladder & Scene Studio parity | ✅ one resolution ladder for every live AI endpoint (per-call args > `provider_settings` table > env > fail-closed, ADR-0012) · settings UI: Settings → AI providers (image endpoint/model/timeout/retries, API key → OS keychain `authr_img`, local LLM server URL, one-real-generation probe with latency) · hardened R2I fetch (bounded retries + backoff, 64 MiB cap, PNG/JPEG sniffing, fail-closed `AI_ENGINE_UNAVAILABLE`) · scene frames honor the R2I tier (segment text + cast bios verbatim + style fragment) · shared 10-style art catalog (`video_style`, novelcast parity, style-seeded deterministic repaints) · registry-driven ffmpeg assembly (`missing_only` ensure-render never clobbers live endpoint art) · `media_read` sandbox-scoped storyboard pixels · frontend workerisation (chapter-stats Web Worker + code-split studios) — see `docs/decisions/ADR-0012-provider-settings-ladder.md` |

## Layout

```
AUTHR_OS/
├── docs/CLI_CONTRACT.md      # FROZEN 74-command IPC surface v1.6 (all transports obey)
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
│       ├── video/            # v1.5: draft-time media derivation, SCRIPTED BMP engine
│       │                     #          (5×7 font, seeded palettes), R2I sheets + ladder,
│       │                     #          ffmpeg MP4 assembly (Ken Burns + narration reuse)
│       ├── ipc/              # dispatcher — CLI, Tauri and dev bridge share it
│       ├── shell/            # Tauri v2 window builder (feature = "gui")
│       └── main.rs           # `authr` CLI (single-line JSON envelope)
├── src/                      # React 19 frontend (Author's Cockpit)
│   ├── components/layout/    # 3-pane cockpit, Project Binder, Dynamic Inspector
│   ├── components/editor/    # Lexical canvas, custom AST nodes, view modes
│   ├── components/studios/   # Audio / Video / Localization / Marketing / Craft
│   ├── stores/               # Zustand domain stores
│   └── ipc/                  # typed contract surface + transport bridge
├── tests/                    # 41 modules · 229 E2E tests (pytest + run_e2e.py)
└── scripts/                  # ipc-proxy.mjs (browser dev bridge) ·
                              # restore_env.sh (one-shot recycle recovery) ·
                              # slim_workspace.sh (drop build-time state) ·
                              # build_deliverables.py (release zips)
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

**Cockpit chrome** (v3.0 UI/UX delta): command palette (**Ctrl/Cmd+K** —
studios, canvas views, project actions, themes, all filterable), reader
settings popover (dark/midnight/sepia themes, editor type size 14–22px,
focus mode), studio shortcuts **Ctrl/Cmd+Alt+1–6**, binder/inspector toggles
**Ctrl/Cmd+Alt+B / I**, Escape always exits focus mode. Modals trap focus and
restore it on close; all motion collapses under `prefers-reduced-motion`.
Theming rule: components use the semantic utilities (`text-ok`, `bg-accent/10`,
…) — never hardcoded spectrum classes; see `docs/invariants-and-gotchas.md`
§1.14–1.15.

### 3. E2E test suite (229 tests · 41 modules)

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
(v1.6: 74 commands · `{ok,data}` / `{ok:false,error{code,message,category}}` envelopes ·
14 canonical error codes across 6 categories · 120 s lease TTL · shadow-format &
git-message rules). The 9 v1 codes and 35 v1 commands are untouched; v1.1 adds
the Phase 4/5 studio surface, v1.2 the Phase 6 publishing surface, v1.3 the
craft/KDP surface, v1.4 the retention/serialization surface and v1.5 the video
novel studio — all additively. Changes require a contract version bump.
