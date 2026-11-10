//! Tauri v2 desktop shell (Phase 2).
//!
//! Thin GUI adapter over the storage core:
//! * `ShellState` wraps the headless [`crate::ipc::session::SessionState`]
//!   in a mutex managed by Tauri.
//! * Every `#[tauri::command]` in `commands.rs` delegates to
//!   [`crate::ipc::dispatch`] — the exact same dispatcher the CLI uses — so
//!   the desktop wire format is byte-identical to the frozen IPC envelope
//!   contract (`docs/CLI_CONTRACT.md`).
//! * A best-effort lease heartbeat runs before every command, keeping the
//!   `.authr_lock` expiry fresh across long editing sessions.
//! * Three event channels keep the UI live: `authr://command` (every
//!   invocation), `authr://git` (after a mutating command lands), and
//!   `authr://lease` (mount / unmount / renewal failure).
//!
//! The window exit path releases the lease (`RunEvent::Exit`), so the bundle
//! is never left locked behind a dead shell.

pub mod commands;

use crate::ipc::session::SessionState;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

/// Managed shell state.
pub struct ShellState {
    pub session: Mutex<SessionState>,
}

impl ShellState {
    pub fn new() -> Self {
        Self { session: Mutex::new(SessionState::new()) }
    }
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

/// Event emitted after every command invocation.
pub const EVT_COMMAND: &str = "authr://command";
/// Event emitted after a mutating command changed bundle state.
pub const EVT_GIT: &str = "authr://git";
/// Event emitted on mount / unmount / heartbeat failure.
pub const EVT_LEASE: &str = "authr://lease";

/// Effective keychain mode for the status bar (mirrors
/// `security::keychain::select` without touching the frozen module).
pub fn keychain_mode() -> &'static str {
    if std::env::var("AUTHR_MOCK_KEYCHAIN").ok().as_deref() == Some("1") {
        "mock"
    } else if cfg!(any(target_os = "windows", target_os = "macos")) {
        "os"
    } else {
        "mock"
    }
}

/// Latest commit (id + message) or `None` when the repo has no commits.
fn latest_commit(root: &std::path::Path) -> Option<Value> {
    crate::storage::git::log(root, 1).ok()?.into_iter().next()
}

/// Shared body of every IPC command wrapper: heartbeat, dispatch, envelope,
/// events. Runs with the session mutex held (commands are synchronous).
pub(crate) fn run_with_session(
    state: &State<ShellState>,
    app: &AppHandle,
    command: &str,
    payload: Value,
) -> Value {
    let started = std::time::Instant::now();
    let session = state.session.lock().expect("session mutex poisoned");

    // Best-effort lease heartbeat: same-session acquire refreshes the lock
    // expiry. A failure here means the lease was taken over (e.g. expired
    // while the machine slept); surface it, but let read paths continue.
    if let Err(e) = session.heartbeat() {
        let _ = app.emit(
            EVT_LEASE,
            json!({ "state": "renew_failed", "error": e.message }),
        );
    }

    let result = session.with_root(|root| crate::ipc::dispatch(root, command, payload));
    let envelope = match &result {
        Ok(data) => crate::envelope_ok(data),
        Err(e) => crate::envelope_err(e),
    };

    let ok = envelope["ok"].as_bool().unwrap_or(false);
    let _ = app.emit(
        EVT_COMMAND,
        json!({
            "command": command,
            "ok": ok,
            "duration_ms": started.elapsed().as_millis() as u64,
        }),
    );
    if ok && crate::ipc::session::MUTATING_COMMANDS.contains(&command) {
        if let Some(commit) = session.with_root(latest_commit).ok().flatten() {
            let _ = app.emit(EVT_GIT, json!({ "latest": commit }));
        }
    }
    envelope
}

/// Entry point of the `authr-desktop` binary.
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ShellState::new())
        .invoke_handler(tauri::generate_handler![
            // Desktop session management
            commands::desktop_mount,
            commands::desktop_unmount,
            commands::desktop_status,
            commands::desktop_info,
            // Project lifecycle
            commands::project_info,
            commands::project_mount,
            commands::project_unmount,
            commands::project_export,
            commands::project_git_log,
            // Books
            commands::book_create,
            commands::book_get,
            commands::book_list,
            commands::book_update,
            commands::book_delete,
            // Chapters
            commands::chapter_create,
            commands::chapter_get,
            commands::chapter_list,
            commands::chapter_update,
            commands::chapter_delete,
            // Characters
            commands::character_create,
            commands::character_get,
            commands::character_list,
            commands::character_update,
            commands::character_delete,
            // Lorebook
            commands::lore_create,
            commands::lore_get,
            commands::lore_list,
            commands::lore_update,
            commands::lore_delete,
            // Chekhov threads
            commands::thread_create,
            commands::thread_get,
            commands::thread_list,
            commands::thread_update,
            commands::thread_delete,
            // Segments
            commands::segment_create,
            commands::segment_get,
            commands::segment_list,
            commands::segment_update,
            commands::segment_delete,
            commands::segment_reorder,
            // Search & security
            commands::search_segments,
            commands::sandbox_resolve,
        ])
        .build(tauri::generate_context!())
        .expect("AUTHR_OS desktop shell failed to initialize");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            // Release the lease so the bundle is never left locked behind a
            // dead shell process.
            let state: State<ShellState> = app_handle.state();
            if let Ok(mut session) = state.session.lock() {
                let _ = session.unmount();
            }
        }
    });
}
