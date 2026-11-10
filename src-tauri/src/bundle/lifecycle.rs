// AUTHR_OS — Project lifecycle (contract #1–4): init / open / close / status.
// Includes the cloud-sync guard (§6.2 step 2).

use super::lock;
use super::manifest::Manifest;
use crate::error::AppError;
use serde_json::{json, Value};
use std::path::Path;

const SUBDIRS: &[&str] = &[
    "manuscript",
    "audio/segments",
    "audio/sfx_custom",
    "assets",
    "exports",
];

/// #1 project_init — create the directory bundle, manifest v1, schema,
/// embedded git scaffold and book #1.
pub fn project_init(a: &Value) -> Result<Value, AppError> {
    let raw_path = crate::ipc::args::req_str(a, "path")?;
    let title = crate::ipc::args::req_str(a, "title")?;
    let author = crate::ipc::args::get_str(a, "author").unwrap_or_else(|| "Anonymous".into());

    let normalized = crate::security::sandbox::normalize_raw(&raw_path)?;
    let root = std::path::PathBuf::from(&normalized);

    if root.exists() && root.join("manifest.json").exists() {
        return Err(AppError::bundle(format!(
            "existing .authr bundle at {}",
            root.display()
        )));
    }
    std::fs::create_dir_all(&root)?;

    for sub in SUBDIRS {
        std::fs::create_dir_all(root.join(sub))?;
    }

    let manifest = Manifest::new(&title, &author);
    let manifest_json = manifest.write(&root)?;

    // Schema + narrator seed.
    {
        let conn = crate::storage::db::open_db(&root)?;
        drop(conn);
    }

    // Book #1 carries the project title.
    let book_id: i64 = {
        let conn = crate::storage::db::open_db(&root)?;
        crate::storage::books::create(
            &conn,
            &json!({"title": title, "author": author}),
        )?["id"]
            .as_i64()
            .unwrap_or(1)
    };

    // Embedded git + scaffold commit.
    let commit = crate::storage::git::ensure_repo(&root)
        .and_then(|_| crate::storage::git::initial_scaffold_commit(&root))
        .ok();

    Ok(json!({
        "path": root.display().to_string(),
        "manifest": manifest_json,
        "book_id": book_id,
        "readonly": false,
        "git_commit": commit,
    }))
}

/// #2 project_open — lease acquire (with stale recovery) or read-only mount.
pub fn project_open(a: &Value, bundle: Option<&Path>) -> Result<Value, AppError> {
    let path = match a.get("path").and_then(|v| v.as_str()) {
        Some(p) => std::path::PathBuf::from(p),
        None => match bundle {
            Some(b) => b.to_path_buf(),
            None => return Err(AppError::bundle("project_open requires `path`")),
        },
    };
    let root = path;
    let manifest = Manifest::read(&root)?;

    let cloud_guard = crate::security::sandbox::is_cloud_sync_path(&root);

    // Stale leases are reclaimed on open (dead pid / expired / corrupt).
    let readonly = match lock::classify(&root)? {
        lock::LockState::Unlocked => {
            let lease = lock::Lease::acquire();
            lease.write(&root)?;
            false
        }
        lock::LockState::Stale(_) => {
            lock::recover_stale(&root)?;
            let lease = lock::Lease::acquire();
            lease.write(&root)?;
            false
        }
        lock::LockState::ForeignActive(_) => true, // inspection mode
    };

    Ok(json!({
        "readonly": readonly,
        "manifest": manifest,
        "cloud_sync_guard": cloud_guard,
    }))
}

/// #3 project_close — release lease + WAL truncation checkpoint.
pub fn project_close(bundle: &Path, conn: &rusqlite::Connection) -> Result<Value, AppError> {
    let released = lock::release(bundle)?;
    if conn
        .query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
        .unwrap_or_default()
        == "wal"
    {
        crate::storage::db::wal_truncate(conn)?;
    }
    Ok(json!({ "released": released }))
}

/// #4 project_lock_status.
pub fn project_lock_status(bundle: &Path) -> Result<Value, AppError> {
    lock::status_json(bundle)
}
