# RUN_ON_LAPTOP.md — from download to running app in ~10 minutes

You have two packages. Both contain the identical, fully verified source tree
(168/168 E2E + 67/67 unit tests green, contract v1.2, 56 commands).

| Zip | Contents | Use it when |
|---|---|---|
| `AUTHR_OS_source.zip` | clean source only (no build artifacts) | you want to build everything from scratch, or read/modify the code |
| `AUTHR_OS_workspace.zip` | source **+ prebuilt Linux x86_64 core binary** (`src-tauri/target/release/authr`) **+ compiled frontend** (`dist/`) | you want to run tests and the app immediately on a Linux/WSL machine |

## 1. Prerequisites

| Tool | Version | Needed for | Install |
|---|---|---|---|
| Rust | 1.99.1 (pinned baseline) | building the core | `curl https://sh.rustup.rs -sSf -o ri.sh && bash ri.sh -y --default-toolchain 1.99.1 --profile minimal` (Windows: rustup-init.exe) |
| Node.js | ≥ 20 | frontend + dev bridge | nodejs.org or your package manager |
| Python | 3.10+ with `pip install pytest` | E2E test suite only | python.org |
| ffmpeg | any recent | M4B audiobook export only | apt/choco/brew; or `AUTHR_FFMPEG_BIN=/path` |
| typst | any recent | PDF export only | typst.app; or `AUTHR_TYPST_BIN=/path` |

Desktop-shell builds additionally need per-OS system deps (see §4).
Everything else — SQLite, git (vendored libgit2), hashing RAG embedder, offline
LLM fallback — is compiled in; no services to install.

## 2. Quickstart (any OS)

```bash
unzip AUTHR_OS_workspace.zip && cd AUTHR_OS   # or AUTHR_OS_source.zip
npm install                                    # always; node_modules is not shipped
npm run build                                  # tsc --noEmit + vite build → dist/
cd src-tauri && cargo build --release          # → target/release/authr
cd .. && python3 -m pip install pytest         # once
AUTHR_MOCK_KEYCHAIN=1 python3 -m pytest tests/ -q     # expect: 168 passed
./src-tauri/target/release/authr --contract    # expect: 56 commands, version 1.2
npm run dev                                    # open http://localhost:5173
```

`npm run dev` is the full Author's Cockpit in a plain browser: the vite
middleware (`scripts/ipc-proxy.mjs`) spawns the real `authr` binary, so every
edit drives the actual Rust core. On the workspace zip under Linux you can skip
the two build steps — binary and `dist/` are already there.

Windows PowerShell: set env vars with `$env:AUTHR_MOCK_KEYCHAIN = "1"`; the
binary is `src-tauri\target\release\authr.exe`.

## 3. What runs with zero network / zero API keys

Everything: authoring, storage, shadow files, RAG, offline LLM, EPUB/DOCX/
captions/Typst, and scripted audio synthesis. Tests enforce this via
`AUTHR_MOCK_KEYCHAIN=1` + `AUTHR_TTS_OFFLINE=1`. Real network TTS (Edge TTS
needs no key; ElevenLabs/Cartesia/OpenAI need keys) is opt-in — see
`docs/config-and-env.md`. Local AI models (Kokoro ONNX, GGUF) are dropped into
`<bundle>/models/` at runtime — see `docs/LOCAL_AI.md`.

## 4. Desktop window (Tauri shell) — optional

```bash
npm run build                                   # dist/ must exist first
cd src-tauri && cargo build --release --features gui,os-keychain
./target/release/authr                          # opens the 1600×1000 cockpit window
```

System deps for the `gui` feature:
- **Linux (Debian/Ubuntu)**: `sudo apt install libwebkit2gtk-4.1-dev build-essential libgtk-3-0`
- **macOS**: `xcode-select --install`
- **Windows**: MSVC Build Tools; WebView2 ships with Win10/11

Installers (msi/nsis/dmg/appimage/deb, targets configured in
`src-tauri/tauri.conf.json`):

```bash
npm install -D @tauri-apps/cli && npx tauri build
```

## 5. Verify / measure (same numbers we shipped)

```bash
python3 tests/run_e2e.py --json                 # per-tier official runner
python3 scripts/refresh_acceptance.py .         # regenerates test_results.json
./src-tauri/target/release/authr benchmark_run --args '{}' --bundle /tmp/bm
cd src-tauri && cargo test --release            # 67 unit tests
```

Benchmark gates (must print `pass: true`): cold start < 1500 ms, peak RSS
< 50 MB, binary < 25 MB. On the shipped Linux build expect ≈1.8 ms / 6.3 MB /
10.1 MB.

## 6. If something breaks

Open `docs/troubleshooting.md` first — symptom → cause → exact file/line → fix.
`AGENTS.md` is the maintainer entry point (hard rules + doc index). The E2E
suite tells you the exact command that failed; re-run it by hand with
`./src-tauri/target/release/authr <command> --bundle <dir> --args '<json>'`.

Known platform notes:
- The prebuilt binary in the workspace zip is **Linux x86_64 only**. On
  macOS/Windows delete `src-tauri/target/` and rebuild (conftest fails fast
  with the exact build command if the binary is missing).
- `AUTHR_MOCK_KEYCHAIN=1` is mandatory for tests on every OS (file-backed
  keyring; real OS keychain is only used by the `os-keychain` desktop build).
- pytest config is pinned by `pytest.ini` in the repo root — keep the zip's
  directory layout intact so it stays discoverable.
