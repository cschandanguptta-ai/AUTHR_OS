# RUN_ON_WINDOWS.md — AUTHR_OS on Windows 10 / 11, from download to running app

Read this together with `RUN_ON_LAPTOP.md` (the all-OS guide). This file is the
Windows-specific walk-through: exact PowerShell commands, what the zips contain,
and how to produce a native Windows installer.

> **The one thing to know before anything else:** the prebuilt core binary
> inside `AUTHR_OS_v3.0.0-PROD.zip` is **Linux x86_64 only**
> (`src-tauri/target/release/authr`, an ELF executable). On Windows it cannot
> run. Delete `src-tauri\target\` and rebuild — one cargo command produces the
> native `authr.exe`. Everything else (source, docs, frontend) is
> platform-neutral. The E2E conftest fails fast with the exact build command
> if the binary is missing, so a wrong setup announces itself immediately.

## 1. Choose your package

| Zip | Contents | Use it when |
|---|---|---|
| `AUTHR_OS_docs.zip` | full source + complete docs (no build artifacts) | Windows is the natural choice: build everything from scratch |
| `AUTHR_OS_v3.0.0-PROD.zip` | source + docs **+ prebuilt Linux binary + compiled frontend** (`dist/`) | only the `dist/` folder saves you one step on Windows; the Linux binary is dead weight |

On Windows the effective difference between the two zips is small — you will
rebuild the core and re-run the frontend build anyway. Either zip works.

## 2. Prerequisites (one-time install)

| Tool | Version | Needed for | Install on Windows |
|---|---|---|---|
| **MSVC Build Tools** | VS 2022 Build Tools, "Desktop development with C++" workload | compiling any Rust code (linker + Windows SDK) | `winget install Microsoft.VisualStudio.2022.BuildTools` (select the C++ workload) |
| **Rust** | 1.99.1 pinned baseline (any recent stable works) | building the core | download `rustup-init.exe` from rustup.rs → "install for me", default `stable-x86_64-pc-windows-msvc` |
| **Node.js** | ≥ 20 | frontend + dev bridge | `winget install OpenJS.NodeJS.LTS` |
| **Python** | 3.10+ | E2E test suite only | `winget install Python.Python.3.12` (tick "Add to PATH") |
| **WebView2 Runtime** | Evergreen | desktop window (`gui` feature) | **already shipped with Windows 10/11** — nothing to do |
| **ffmpeg** | any recent | M4B audiobook export only | `winget install Gyan.FFmpeg`; or set `AUTHR_FFMPEG_BIN` to the exe path |
| **typst** | any recent | PDF export only (`.typ` is emitted regardless) | `winget install Typst.Typst`; or set `AUTHR_TYPST_BIN` |
| **Git for Windows** | any | convenience (shell, tar/unzip) | `winget install Git.Git` — *not required by the app* (SQLite and libgit2 are compiled in) |

Everything else — SQLite, the hashing RAG embedder, the offline LLM fallback,
scripted TTS, the deterministic image engine — is **compiled into the core**.
No services, no databases, no API keys required.

Rust version check (matches the CI baseline):

```powershell
rustc --version    # expect: rustc 1.99.1 or newer
```

## 3. Quickstart (PowerShell, ~10 minutes)

```powershell
# 0) unpack — keep the zip's folder layout intact (pytest.ini discovery depends on it)
Expand-Archive AUTHR_OS_v3.0.0-PROD.zip -DestinationPath C:\dev
cd C:\dev\AUTHR_OS

# 1) frontend deps + build (node_modules is never shipped)
npm install
npm run build            # tsc --noEmit + vite build  →  dist\

# 2) rebuild the core as a native Windows binary
Remove-Item -Recurse -Force src-tauri\target    # the Linux binary is useless on Windows
cd src-tauri
cargo build --release           # → src-tauri\target\release\authr.exe
cd ..

# 3) verify the frozen contract (74 commands, v1.6)
.\src-tauri\target\release\authr.exe --contract

# 4) E2E suite — expect 229 passed
python -m pip install pytest
$env:AUTHR_MOCK_KEYCHAIN = "1"          # mandatory on every OS for tests (file-backed keyring)
python -m pytest tests -q

# 5) run the Author's Cockpit in a plain browser — the full app, no Tauri needed
npm run dev              # open http://localhost:5173
```

`npm run dev` is **the real application**, not a mock: the Vite middleware
(`scripts/ipc-proxy.mjs`) spawns the actual `authr.exe` and relays the frozen
JSON IPC contract, so Lexical edits, binder trees and all six studios drive the
Rust core. It resolves the binary as `src-tauri\target\release\authr.exe`
(override with `$env:AUTHR_CLI`); the default bundle lives at `<repo>\sandbox.authr`
(override with `$env:AUTHR_BUNDLE`).

`authr.exe` also works as a plain CLI — every one of the 74 contract commands
is callable directly:

```powershell
.\src-tauri\target\release\authr.exe bootstrap --bundle C:\dev\mybook.authr --args '{}'
.\src-tauri\target\release\authr.exe chapter_create --bundle C:\dev\mybook.authr --args '{"title":"Chapter 1"}'
```

## 4. Native desktop window (Tauri shell)

```powershell
npm run build                                   # dist\ must exist first
cd src-tauri
cargo build --release --features gui,os-keychain
.\target\release\authr.exe                      # opens the 1600×1000 cockpit window
```

- `gui` links Tauri v2 against the **WebView2** runtime that ships with
  Windows 10/11 — no extra system packages needed on Windows (unlike Linux).
- `os-keychain` stores API keys in the **Windows Credential Manager** (via the
  `keyring` crate). Without it the file-backed mock keyring is used.

## 5. Windows installers: MSI + NSIS setup exe

Both Windows targets are pre-configured in `src-tauri/tauri.conf.json`
(`bundle.targets` includes `msi` and `nsis`; WiX and NSIS toolchains are pulled
by the Tauri bundler automatically):

```powershell
npm install -D @tauri-apps/cli
npx tauri build
# → src-tauri\target\release\bundle\msi\AUTHR_OS_3.0.0_x64_en-US.msi
# → src-tauri\target\release\bundle\nsis\AUTHR_OS_3.0.0_x64-setup.exe
```

What the installer gives the end user: Start-menu shortcut, WebView2
bootstrapper check, the core + frontend bundled into one signed unit.

Code signing (optional, release-machine task): put the Authenticode
`certificateThumbprint` of your code-signing certificate into
`tauri.conf.json → bundle.windows`; the SHA-256 digest + timestamp server are
already configured. The updater manifest is produced entirely offline by the
core itself:

```powershell
.\src-tauri\target\release\authr.exe release_manifest --args '{
  "artifact": "src-tauri/target/release/bundle/nsis/AUTHR_OS_3.0.0_x64-setup.exe",
  "version": "3.0.0-PROD",
  "url": "https://example.com/releases/AUTHR_OS_3.0.0_x64-setup.exe"
}'
.\src-tauri\target\release\authr.exe release_verify --args '{"manifest":"latest.json","artifact":"AUTHR_OS_3.0.0_x64-setup.exe"}'
```

`release_manifest` needs the ed25519 seed via `AUTHR_RELEASE_KEY_HEX` (exported
from the OS keychain — see `docs/PACKAGING.md §Key management`); without it a
clearly-marked ephemeral key is generated (CI-only mode). Paths passed to
`release_manifest` / `release_verify` should be absolute.

## 6. PowerShell environment-variable reference

PowerShell syntax differs from bash — set with `$env:NAME = "value"`, read with
`$env:NAME`, delete with `Remove-Item Env:\NAME`. The variables you are most
likely to touch:

| Variable | Purpose | Default behaviour without it |
|---|---|---|
| `AUTHR_MOCK_KEYCHAIN` | file-backed keyring for tests | **set to `"1"` for pytest, always** |
| `AUTHR_TTS_OFFLINE` | deterministically blocks network TTS engines (exact value `"1"`) | network TTS allowed |
| `AUTHR_FFMPEG_BIN` | ffmpeg path for M4B export | looks up `ffmpeg` on PATH |
| `AUTHR_TYPST_BIN` | typst path for PDF compile | looks up `typst` on PATH; missing ⇒ `.typ` emitted, `compiled:false` |
| `AUTHR_BIN` | binary path the test suite drives | `src-tauri\target\release\authr.exe` (note: conftest uses the `.exe`-suffixed name automatically on Windows) |
| `AUTHR_CLI` | binary the dev bridge spawns | same as above |
| `AUTHR_BUNDLE` | default bundle for the dev bridge | `<repo>\sandbox.authr` |
| `AUTHR_LLAMA_SERVER` | local llama-server URL for `local_llm_generate` | offline grammar LLM answers instead |
| `AUTHR_IMG_ENDPOINT` / `AUTHR_IMG_MODEL` / `AUTHR_IMG_API_KEY` | image-provider ladder (env tier) | fail-closed R2I tier; SCRIPTED image engine unaffected |

Stored settings always outrank env: anything configured in the cockpit under
**Settings → AI providers** beats the `AUTHR_IMG_*` / `AUTHR_LLAMA_SERVER`
environment tier (ADR-0012 ladder).

## 7. What works with zero network / zero keys

Everything: authoring, storage, shadow files, RAG retrieval, the offline LLM
fallback, EPUB 3 / DOCX / captions / Typst export, scripted audio and the
deterministic image engine. Optional network tiers (Edge TTS needs no key;
ElevenLabs / Cartesia / OpenAI need keys; image endpoints need an endpoint) are
opt-in and fail closed. Local models (Kokoro ONNX, GGUF) are dropped into
`<bundle>\models\` at runtime — see `docs/LOCAL_AI.md`.

## 8. Verify / measure (same gates as CI)

```powershell
python -m pytest tests -q                       # 229 E2E across 41 modules
python tests\run_e2e.py --json                  # per-tier official runner
.\src-tauri\target\release\authr.exe benchmark_run --args '{}' --bundle $env:TEMP\bm
cd src-tauri; cargo test --release              # 135 unit tests
```

Benchmark gates must print `pass: true` (cold start < 1500 ms, peak RSS < 50 MB,
binary < 25 MB). The shipped Linux build measures ≈1.7 ms / 6.4 MB / 11.3 MB —
your Windows numbers will differ slightly but must clear the same gates.

## 9. If something breaks

1. `docs/troubleshooting.md` — symptom → cause → exact file/line → fix.
2. The E2E suite prints the exact failing command; re-run it by hand:
   `.\src-tauri\target\release\authr.exe <command> --bundle <dir> --args '<json>'`.
3. `Permission denied` / "binary missing" on tests ⇒ the core wasn't rebuilt
   (`cargo build --release` in `src-tauri\`) or `AUTHR_BIN` points elsewhere.
4. `cargo` not found in a fresh terminal ⇒ open the **x64 Native Tools
   Command Prompt for VS** or ensure `%USERPROFILE%\.cargo\bin` is on PATH.
5. `npm run dev` returns HTTP 500 on `/api/ipc` ⇒ `AUTHR_CLI` unset and the
   exe missing — rebuild step 3 of the quickstart.
6. `AGENTS.md` is the maintainer entry point (hard rules + doc index).
