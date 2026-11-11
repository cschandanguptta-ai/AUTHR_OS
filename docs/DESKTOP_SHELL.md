# AUTHR_OS Desktop Shell — Tauri v2 (Phase 2)

Phase 2 wraps the Phase 1 storage core in a native desktop shell. The
storage engine, the frozen IPC envelope contract (`docs/CLI_CONTRACT.md`)
and the entire 101-test Phase 1 suite are untouched and still green; Phase 2
adds the GUI adapter, a persistent session model, live shell events, and a
production frontend — plus a fifth test tier that pins all of it.

```
┌────────────────────────────────────────────────────────────────────┐
│  frontend/ (React 18 + TypeScript + Vite)                          │
│    console UI: resource panels · FTS search · git · security       │
│    transport.ts ── invoke(cmd, {payload}) ─▶ envelope back         │
│        │ (browser preview falls back to mock.ts fixture bundle)    │
├────────────────────────────────────────────────────────────────────┤
│  tauri_shell (src-tauri, feature = "desktop")                      │
│    #[tauri::command] ×41  ──▶  ipc::dispatch  ──▶  ipc::handlers   │
│    ShellState ─ Mutex<ipc::session::SessionState>                  │
│    events: authr://command · authr://git · authr://lease           │
├────────────────────────────────────────────────────────────────────┤
│  storage core (Phase 1, unchanged)                                 │
│    SQLite WAL + FTS5 · lease lock · git2 · shadow MD/AST · sandbox │
└────────────────────────────────────────────────────────────────────┘
```

## Single source of truth

Every command handler lives in **`src-tauri/src/ipc/handlers.rs`**. The CLI
dispatcher (`ipc::dispatch`) and the Tauri command layer both delegate there,
so a behavioral change can never land in one surface and miss the other.
`dispatch` keeps its exact Phase 1 semantics (canonicalization, DTO
validation, `unknown IPC command` envelope); the Phase 1 suite froze this.

The `ipc_command!` macro in `tauri_shell/commands.rs` stringifies its
identifier into the dispatched command name, making wrapper/command drift
impossible by construction; `tests/tier5_desktop_shell` enforces parity
across Rust, the frontend catalog (`frontend/src/api.ts`), and the mock
transport.

## Desktop session model

Phase 1 CLI processes were one-shot: mount → command → exit. A desktop shell
holds one mounted bundle for the whole editing session:

| Concern | Mechanism |
| --- | --- |
| Session state | `ipc::session::SessionState` — headless, plain Rust, 10 unit tests |
| Mutual exclusion | `.authr_lock` lease acquired at `desktop_mount` |
| Long sessions | **lease heartbeat** — every command refreshes the lease via same-session `lock::acquire` (30–3600 s clamp) |
| Writability | `ensure_writable` still rejects foreign leases with `READ_ONLY_PROJECT` |
| Exit hygiene | `RunEvent::Exit` releases the lease — never leave a bundle locked behind a dead shell |

**Phase 2 semantics fix:** a lease owned by the *current process* is never
"foreign" (`bundle/lock.rs::foreign_active`). In the CLI each command ran in
a fresh process, so the case never arose; in the shell the live PID is ours,
and treating it as foreign would have made every post-mount write fail.
Frozen tests are unaffected — they simulate foreign leases with real
subprocess or ghost PIDs.

## Wire format & events

Commands return the frozen envelope:

```json
{ "ok": true,  "data": ... }
{ "ok": false, "error": { "code": "...", "message": "...", "category": "storage|bundle|security" } }
```

Frontend invocation:

```ts
const envelope = await invoke('book_create', { payload: { title: 'Signal Fires' } });
```

Shell events (Tauri `emit` → `listen`):

| Event | When | Payload |
| --- | --- | --- |
| `authr://command` | after every invocation | `{ command, ok, duration_ms }` |
| `authr://git` | after a mutating command lands | `{ latest: { id, message, timestamp } }` |
| `authr://lease` | mount / unmount / heartbeat failure | `{ state, session }` |

`MUTATING_COMMANDS` (in `ipc/session.rs`) drives the `authr://git` trigger:
19 entity mutations (incl. `segment_reorder`) + `project_mount`/`project_unmount`.

### Additive commands (Phase 2)

| Command | Payload | Purpose |
| --- | --- | --- |
| `project_git_log` | `{ limit?: usize }` → `{ commits: [{ id, message, timestamp }] }` | commit history for the desktop Git panel |

### Additive commands (Phase 3)

| Command | Payload | Purpose |
| --- | --- | --- |
| `segment_reorder` | `{ chapter_id, order: [row ids] }` → `{ chapter_id, reordered }` | whole-chapter `sequence_order` re-index (dense 1..N) after Lexical AST splits / merges / deletes. The payload must be a permutation of every segment row id of the chapter; duplicates, foreign ids and partial lists are rejected with `VALIDATION_ERROR`. Participates in the touch / shadow / git snapshot machinery (`MUTATING_COMMANDS`). Backed by `storage::segments::reorder` (single transaction). |

Everything else is the frozen Phase 1 surface (36 commands), unchanged.
`docs/CLI_CONTRACT.md` remains the frozen Phase 1 reference; the desktop
contract lives here.

### Desktop-only session commands

`desktop_mount { root, lease_secs? }`, `desktop_unmount`,
`desktop_status`, `desktop_info` — managed by the shell (not `dispatch`)
because they own the long-lived session instead of a one-shot root.

## Security posture

* **CSP** (`tauri.conf.json`): `default-src 'self'`; no `unsafe-eval`;
  `connect-src ipc: http://ipc.localhost` for the Tauri IPC bridge;
  `style-src 'unsafe-inline'` (Vite-injected styles only).
* **Capabilities** (`capabilities/default.json`): scoped to the `main`
  window only; `core:default` + dialog open/save. No shell, fs, or http
  plugin permissions are granted.
* **Path sandboxing** is unchanged: every filesystem path still flows
  through `security::sandbox` (verbatim-prefix normalization, traversal
  rejection, symlink/Zip-Slip guards).
* **Keychain**: OS keychain on Windows (Credential Manager) and macOS
  (Keychain); mock file fallback on Linux/headless (`AUTHR_MOCK_KEYCHAIN=1`
  forces mock anywhere). The status bar shows the effective mode via
  `desktop_info`.

## Frontend

React 18 + TypeScript + Vite, no UI framework. Layout: top bar (mount /
unmount, keychain + mode badges), sidebar (manuscript + introspection
panels), schema-driven resource tables for all six entity domains, FTS5
search, git history, security/sandbox inspector, and a right-hand inspector
showing the raw envelope of the last command plus the live event log.

**Demo mode:** `window.__TAURI_INTERNALS__` detection switches the transport
(`frontend/src/transport.ts`). Outside the webview, `mock.ts` serves an
in-memory fixture bundle implementing the entire 37-command surface with
frozen envelope semantics — so `npm run dev` in any browser, UI tests, and
screenshots work without the Rust shell. Native mode uses
`@tauri-apps/api` + `@tauri-apps/plugin-dialog` for the bundle picker.

## Build matrix

Prerequisites: Rust (stable) + Node ≥ 20. WebView runtime: WebView2
(preinstalled on Windows 10/11), WKWebView (macOS), WebKitGTK (Linux).

### Windows

```powershell
npm --prefix frontend install
npm --prefix frontend run build
cargo build --release --features desktop        # or: cargo tauri build
# installer: src-tauri/target/release/bundle/nsis/AUTHR_OS_3.1.0_x64-setup.exe
```

`cargo tauri build` (via `cargo install tauri-cli --version "^2"`) also
produces MSI (WiX) and NSIS setups; signing hooks live in `tauri.conf.json`.

### macOS

```bash
npm --prefix frontend install && npm --prefix frontend run build
cargo build --release --features desktop        # or: cargo tauri build
# .app / .dmg under src-tauri/target/release/bundle/
```

### Linux

WebKitGTK 4.1 is required (the `desktop` feature will not compile without
it — this is why headless CI builds only the CLI):

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
npm --prefix frontend install && npm --prefix frontend run build
cargo build --release --features desktop        # or: cargo tauri build
# .deb / .rpm / AppImage under src-tauri/target/release/bundle/
```

### Headless / CLI-only environments

```bash
cargo build --release            # CLI only, no GUI deps needed
cargo test --lib                 # session + sandbox unit tests
AUTHR_BIN=src-tauri/target/release/authr python3 tests/run_e2e.py   # 117 tests
```

## Development loop

```bash
cargo tauri dev        # hot-reloads the Vite frontend + rebuilds the shell
npm --prefix frontend run dev   # browser demo (mock bundle, no Rust needed)
```

## Test layout (Tier 5)

| File | Scope |
| --- | --- |
| `test_t5_tauri_config.py` | conf schema, CSP, capabilities, icons, feature gating, headless session module |
| `test_t5_command_parity.py` | dispatch ↔ Tauri wrappers ↔ frontend catalog ↔ mock coverage |
| `test_t5_frontend_artifacts.py` | dist build integrity, no dev URLs/secrets, docs presence |

Phase 3 adds Tier 6 (`tests/tier6_cockpit/`): dist markers for the cockpit /
studios, the dependency surface, a live `segment_reorder` CLI smoke, the
Vitest unit run and documentation binding — see `docs/COCKPIT.md`.

Full suite: **125 E2E tests** (101 frozen Phase 1 + 16 Phase 2 + 8 Phase 3)
plus 48 Vitest unit tests and 14 Rust unit tests (`cargo test --lib`).
