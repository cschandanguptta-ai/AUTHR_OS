# ADR-0001 — One frozen dispatcher shared by all transports

**Status**: Accepted (implemented since Phase 1; frozen by contract v1/v1.1/v1.2)

**Context**: The product needs the same 56 operations from a headless CLI
(CI, scripting), a Tauri desktop shell, and a plain-browser dev server —
without three implementations drifting.

**Decision**: Every transport funnels into a single table-driven dispatcher,
`src-tauri/src/ipc/mod.rs::dispatch:81-210`. The CLI (`main.rs`), the Tauri
pass-through `authr_command` (`shell/mod.rs:9-20`), and the vite dev bridge
(`scripts/ipc-proxy.mjs` spawning the CLI) are thin adapters. The surface is
listed once in `lib.rs::COMMANDS:22-92` and specified in
`docs/CLI_CONTRACT.md`; `test_t1_ipc_contracts.py::test_ip1` asserts the
shipped binary matches the doc via `authr --contract`.

**Alternatives**: (a) per-transport native APIs (e.g. 56 `#[tauri::command]`
fns) — rejected by the evidence of one shared `Ctx`/lease/sandbox path being
unit-testable headlessly; no code for (a) exists in-repo. (b) HTTP server core
— not present in-repo.

**Consequences**: adding a command touches `lib.rs` + dispatcher + contract +
ipc_contracts test; behaviors are identical across surfaces by construction;
headless CI exercises the exact production code path (minus the webview).

**Do not change unless**: you are prepared to bump the contract version,
re-verify `test_ip1`, and re-document all three transports
(`change-impact.md §5`).

**Evidence**: `src-tauri/src/ipc/mod.rs`, `src-tauri/src/main.rs:86`,
`src-tauri/src/shell/mod.rs`, `scripts/ipc-proxy.mjs`,
`tests/tier1_features/test_t1_ipc_contracts.py`.
