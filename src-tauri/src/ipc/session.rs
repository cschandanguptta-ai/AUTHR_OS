//! Desktop session state machine (Phase 2) — headless and unit-testable.
//!
//! The Tauri desktop shell holds ONE mounted bundle for the lifetime of the
//! editing session, unlike the Phase 1 CLI where every command was a fresh
//! process. This module owns that session: mount (lease acquire), unmount
//! (lease release), a lease heartbeat that keeps the `.authr_lock` expiry
//! fresh across long editing sessions, and the writable-session guard that
//! every command handler passes through.
//!
//! Everything here is plain Rust with no Tauri types, so the full state
//! machine is exercised by `cargo test` in headless CI environments.

use crate::bundle::{lifecycle, lock};
use crate::error::AppError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Lease duration bounds accepted by `desktop_mount` (seconds).
pub const MIN_LEASE_SECS: i64 = 30;
pub const MAX_LEASE_SECS: i64 = 3600;

/// IPC commands that mutate bundle state. The desktop shell emits a
/// `authr://git` event after any of these so the UI can refresh commit
/// history. Read-only commands (`*_get`, `*_list`, `search_segments`,
/// `project_info`, `project_export`, `sandbox_resolve`, `project_git_log`)
/// are deliberately excluded.
pub const MUTATING_COMMANDS: [&str; 21] = [
    "book_create",
    "book_update",
    "book_delete",
    "chapter_create",
    "chapter_update",
    "chapter_delete",
    "character_create",
    "character_update",
    "character_delete",
    "lore_create",
    "lore_update",
    "lore_delete",
    "thread_create",
    "thread_update",
    "thread_delete",
    "segment_create",
    "segment_update",
    "segment_delete",
    "segment_reorder",
    "project_mount",
    "project_unmount",
];

/// One mounted bundle session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Canonicalized bundle root.
    pub root: PathBuf,
    /// Lease session id (stable across heartbeats of the same session).
    pub session_id: String,
    /// Lease duration in seconds used by every heartbeat refresh.
    pub lease_secs: i64,
}

/// The desktop shell's mutable session state (`tauri::State` inner).
#[derive(Debug, Default)]
pub struct SessionState {
    inner: Option<Session>,
}

impl SessionState {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn session(&self) -> Option<&Session> {
        self.inner.as_ref()
    }

    /// Mount a bundle for the desktop session.
    ///
    /// * Mounting the same root again refreshes the lease (same session id).
    /// * Switching to a different root releases our own lease on the
    ///   previous bundle first, then mounts the new one with a fresh id.
    pub fn mount(&mut self, root: &Path, lease_secs: Option<i64>) -> Result<Value, AppError> {
        let lease_secs = lease_secs
            .unwrap_or(lock::DEFAULT_LEASE_SECS)
            .clamp(MIN_LEASE_SECS, MAX_LEASE_SECS);
        let root: PathBuf = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf());
        if let Some(prev) = &self.inner {
            if prev.root != root {
                let _ = lock::release(&prev.root, &prev.session_id);
                self.inner = None;
            }
        }
        let session_id = match &self.inner {
            Some(s) => s.session_id.clone(),
            None => uuid::Uuid::new_v4().to_string(),
        };
        let data = lifecycle::mount(&root, &session_id, lease_secs)?;
        self.inner = Some(Session { root, session_id, lease_secs });
        Ok(data)
    }

    /// Unmount the current session (release the lease). Idempotent.
    pub fn unmount(&mut self) -> Result<Value, AppError> {
        match self.inner.take() {
            None => Ok(json!({ "unmounted": false, "reason": "not mounted" })),
            Some(s) => lifecycle::unmount(&s.root, &s.session_id),
        }
    }

    /// Lease heartbeat: re-acquire with the same session id, which rewrites
    /// `.authr_lock` with a fresh expiry (see `lock::acquire`).
    pub fn heartbeat(&self) -> Result<Option<lock::LeaseLock>, AppError> {
        match &self.inner {
            None => Ok(None),
            Some(s) => lock::acquire(&s.root, &s.session_id, s.lease_secs).map(Some),
        }
    }

    /// Session guard for command handlers: the closure receives the
    /// canonical bundle root. Errors with `BUNDLE_ERROR` when no session is
    /// mounted (the UI treats this as "mount a project first").
    pub fn with_root<R>(
        &self,
        f: impl FnOnce(&Path) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        match &self.inner {
            None => Err(AppError::bundle(
                "desktop session not mounted; call desktop_mount first",
            )),
            Some(s) => f(&s.root),
        }
    }

    /// Snapshot for the UI status bar / `desktop_status` command.
    pub fn status(&self) -> Value {
        match &self.inner {
            None => json!({ "mounted": false }),
            Some(s) => {
                let lease = match lock::read(&s.root) {
                    lock::LockState::Held(l) if l.session_id == s.session_id => json!({
                        "locked_at": l.locked_at,
                        "expires_at": l.lease_expires_at,
                    }),
                    _ => Value::Null,
                };
                json!({
                    "mounted": true,
                    "root": s.root.display().to_string(),
                    "session_id": s.session_id,
                    "lease_secs": s.lease_secs,
                    "lease": lease,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "authr-session-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed_bundle(tag: &str) -> PathBuf {
        let root = temp_root(tag);
        lifecycle::create(&root, "Session Test Book", "Tester").unwrap();
        root
    }

    fn lock_expiry(root: &Path) -> String {
        match lock::read(root) {
            lock::LockState::Held(l) => l.lease_expires_at,
            other => panic!("expected held lock, got {other:?}"),
        }
    }

    #[test]
    fn mount_requires_an_authr_bundle() {
        let mut state = SessionState::new();
        let empty = temp_root("empty");
        let err = state.mount(&empty, None).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::BundleError);
        assert!(err.message.contains("not an .authr bundle"));
    }

    #[test]
    fn mount_then_guard_ok_and_writable() {
        let root = seed_bundle("ok");
        let mut state = SessionState::new();
        state.mount(&root, None).unwrap();
        // Guard passes and hands the canonical root to the handler.
        let seen = state.with_root(|r| Ok(r.display().to_string())).unwrap();
        assert!(Path::new(&seen).is_absolute());
        // Regression (Phase 2): our own live lease must NOT make the
        // project read-only.
        lifecycle::ensure_writable(&root).unwrap();
        assert!(lock::path(&root).exists());
    }

    #[test]
    fn commands_without_mount_error_with_bundle_code() {
        let state = SessionState::new();
        let err = state
            .with_root(|_r: &Path| Ok::<_, AppError>(()))
            .unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::BundleError);
        assert!(err.message.contains("desktop session not mounted"));
    }

    #[test]
    fn unmount_releases_lease_and_is_idempotent() {
        let root = seed_bundle("unmount");
        let mut state = SessionState::new();
        state.mount(&root, None).unwrap();
        assert!(lock::path(&root).exists());

        let data = state.unmount().unwrap();
        assert_eq!(data["unmounted"], json!(true));
        assert!(!lock::path(&root).exists(), "lease file must be removed");

        // Second unmount: idempotent no-op.
        let again = state.unmount().unwrap();
        assert_eq!(again["unmounted"], json!(false));
        assert_eq!(again["reason"], json!("not mounted"));
    }

    #[test]
    fn heartbeat_refreshes_expiry_with_same_session() {
        let root = seed_bundle("heartbeat");
        let mut state = SessionState::new();
        state.mount(&root, Some(60)).unwrap();
        let session_id = state.session().unwrap().session_id.clone();
        let before = lock_expiry(&root);

        let renewed = state.heartbeat().unwrap().unwrap();
        assert_eq!(renewed.session_id, session_id, "session id must be stable");
        let after = lock_expiry(&root);
        let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap();
        assert!(parse(&after) >= parse(&before), "expiry must not move backwards");
        assert_eq!(renewed.pid, std::process::id() as i32);
    }

    #[test]
    fn remount_different_root_releases_previous_bundle() {
        let a = seed_bundle("remount-a");
        let b = seed_bundle("remount-b");
        let mut state = SessionState::new();
        state.mount(&a, None).unwrap();
        let old_id = state.session().unwrap().session_id.clone();

        state.mount(&b, None).unwrap();
        assert!(!lock::path(&a).exists(), "old bundle lease must be released");
        assert!(lock::path(&b).exists());
        let new_id = state.session().unwrap().session_id.clone();
        assert_ne!(old_id, new_id, "new bundle gets a fresh session id");
        assert_eq!(state.session().unwrap().root, b);
    }

    #[test]
    fn same_root_remount_keeps_session_and_refreshes() {
        let root = seed_bundle("refresh");
        let mut state = SessionState::new();
        state.mount(&root, None).unwrap();
        let id1 = state.session().unwrap().session_id.clone();
        let before = lock_expiry(&root);

        state.mount(&root, Some(120)).unwrap();
        assert_eq!(state.session().unwrap().session_id, id1);
        let after = lock_expiry(&root);
        let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap();
        assert!(parse(&after) >= parse(&before));
    }

    #[test]
    fn lease_secs_clamped_to_supported_range() {
        let root = seed_bundle("clamp");
        let mut state = SessionState::new();
        state.mount(&root, Some(1)).unwrap();
        assert_eq!(state.session().unwrap().lease_secs, MIN_LEASE_SECS);
        state.mount(&root, Some(999_999)).unwrap();
        assert_eq!(state.session().unwrap().lease_secs, MAX_LEASE_SECS);
    }

    #[test]
    fn status_reflects_mount_state() {
        let root = seed_bundle("status");
        let mut state = SessionState::new();
        let st = state.status();
        assert_eq!(st["mounted"], json!(false));

        state.mount(&root, None).unwrap();
        let st = state.status();
        assert_eq!(st["mounted"], json!(true));
        assert!(st["session_id"].as_str().is_some());
        assert!(st["lease"]["expires_at"].as_str().is_some());
    }

    #[test]
    fn mutating_commands_shape_is_stable() {
        assert_eq!(MUTATING_COMMANDS.len(), 21);
        for cmd in MUTATING_COMMANDS {
            let is_entity_mutation = cmd.ends_with("_create")
                || cmd.ends_with("_update")
                || cmd.ends_with("_delete")
                // Phase 3 additive AST-bridge re-index command.
                || cmd == "segment_reorder";
            let is_lifecycle = cmd == "project_mount" || cmd == "project_unmount";
            assert!(is_entity_mutation || is_lifecycle, "unexpected member: {cmd}");
        }
        for read_only in [
            "book_list", "book_get", "search_segments", "project_info",
            "project_export", "sandbox_resolve", "project_git_log",
        ] {
            assert!(
                !MUTATING_COMMANDS.contains(&read_only),
                "{read_only} must not be mutating"
            );
        }
    }
}
