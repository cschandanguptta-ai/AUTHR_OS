//! `#[tauri::command]` surface (Phase 2).
//!
//! Each command is a thin adapter: lease-guarded session root in,
//! `ipc::dispatch` call, frozen envelope out. The `ipc_command!` macro keeps
//! the wrapper name equal to the dispatched command name by construction
//! (stringified identifier = command name), which the parity tests
//! (`tests/tier5_desktop_shell`) enforce against `ipc::dispatch`.
//!
//! Frontend invocation shape (see `frontend/src/transport.ts`):
//!
//! ```js
//! const envelope = await invoke('book_create', { payload: { title: '...' } });
//! // envelope: { ok: true, data: ... } | { ok: false, error: { code, message, category } }
//! ```

use super::{keychain_mode, run_with_session, ShellState, EVT_LEASE};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};

/// Define an IPC command wrapper whose Tauri command name is guaranteed to
/// equal the `ipc::dispatch` command name.
macro_rules! ipc_command {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[tauri::command]
        pub fn $name(
            state: State<'_, ShellState>,
            app: AppHandle,
            payload: Value,
        ) -> Value {
            run_with_session(&state, &app, stringify!($name), payload)
        }
    };
}

// ---------------- Desktop session management ----------------

/// Mount a bundle for the desktop session (acquire the lease lock).
#[tauri::command]
pub fn desktop_mount(
    state: State<'_, ShellState>,
    app: AppHandle,
    root: String,
    lease_secs: Option<i64>,
) -> Value {
    let session = state.session.lock().expect("session mutex poisoned");
    let result = session.mount(std::path::Path::new(&root), lease_secs);
    let envelope = match &result {
        Ok(data) => crate::envelope_ok(data),
        Err(e) => crate::envelope_err(e),
    };
    let ok = envelope["ok"].as_bool().unwrap_or(false);
    let _ = app.emit(
        EVT_LEASE,
        json!({
            "state": if ok { "mounted" } else { "mount_failed" },
            "session": session.status(),
        }),
    );
    let _ = app.emit(
        super::EVT_COMMAND,
        json!({
            "command": "desktop_mount",
            "ok": ok,
            "duration_ms": 0,
        }),
    );
    envelope
}

/// Unmount the current session (release the lease). Idempotent.
#[tauri::command]
pub fn desktop_unmount(state: State<'_, ShellState>, app: AppHandle) -> Value {
    let session = state.session.lock().expect("session mutex poisoned");
    let result = session.unmount();
    let envelope = match &result {
        Ok(data) => crate::envelope_ok(data),
        Err(e) => crate::envelope_err(e),
    };
    let _ = app.emit(
        EVT_LEASE,
        json!({ "state": "unmounted", "session": session.status() }),
    );
    envelope
}

/// Session snapshot for the status bar (no dispatch round-trip needed).
#[tauri::command]
pub fn desktop_status(state: State<'_, ShellState>) -> Value {
    state.session.lock().expect("session mutex poisoned").status()
}

/// Shell metadata: version, effective keychain mode, platform.
#[tauri::command]
pub fn desktop_info(state: State<'_, ShellState>) -> Value {
    json!({
        "version": crate::AUTHR_VERSION,
        "keychain": keychain_mode(),
        "platform": std::env::consts::OS,
        "mounted": state.session.lock().expect("session mutex poisoned").session().is_some(),
    })
}

// ---------------- Project lifecycle ----------------

ipc_command!(
    /// Bundle status: manifest, lock state, read-only flag.
    project_info
);
ipc_command!(
    /// Mount via the CLI-style dispatcher (one-shot; desktop sessions should
    /// use `desktop_mount`).
    project_mount
);
ipc_command!(
    /// Unmount a named session via the CLI-style dispatcher.
    project_unmount
);
ipc_command!(
    /// Export the bundle to an atomic ZIP archive.
    project_export
);
ipc_command!(
    /// Commit history (Phase 2 addition, used by the Git panel).
    project_git_log
);

// ---------------- Books ----------------

ipc_command!(book_create);
ipc_command!(book_get);
ipc_command!(book_list);
ipc_command!(book_update);
ipc_command!(book_delete);

// ---------------- Chapters ----------------

ipc_command!(chapter_create);
ipc_command!(chapter_get);
ipc_command!(chapter_list);
ipc_command!(chapter_update);
ipc_command!(chapter_delete);

// ---------------- Characters ----------------

ipc_command!(character_create);
ipc_command!(character_get);
ipc_command!(character_list);
ipc_command!(character_update);
ipc_command!(character_delete);

// ---------------- Lorebook ----------------

ipc_command!(lore_create);
ipc_command!(lore_get);
ipc_command!(lore_list);
ipc_command!(lore_update);
ipc_command!(lore_delete);

// ---------------- Chekhov threads ----------------

ipc_command!(thread_create);
ipc_command!(thread_get);
ipc_command!(thread_list);
ipc_command!(thread_update);
ipc_command!(thread_delete);

// ---------------- Segments (NarrativeSegmentAst) ----------------

ipc_command!(segment_create);
ipc_command!(segment_get);
ipc_command!(segment_list);
ipc_command!(segment_update);
ipc_command!(segment_delete);
/// Phase 3 additive: AST-bridge whole-chapter sequence re-index.
ipc_command!(segment_reorder);

// ---------------- FTS5 / BM25 search ----------------

ipc_command!(
    /// Full-text search across chapter segments (BM25 ranking).
    search_segments
);

// ---------------- Security ----------------

ipc_command!(
    /// Resolve a bundle-relative path through the sandbox guard.
    sandbox_resolve
);
