// AUTHR_OS — Embedded Git integration (F13, git2/libgit2 in-process):
// repo init inside the bundle, .gitignore exclusions, scaffold commit and
// the `Auto-sync chNN` chapter snapshotter (§6.4 step 3).

use crate::error::AppError;
use std::path::Path;

pub const GITIGNORE: &str = "project.db\nproject.db-wal\nproject.db-shm\n.authr_lock\naudio/\nexports/\n";
pub const GIT_SIGNATURE_NAME: &str = "AUTHR_OS";
pub const GIT_SIGNATURE_EMAIL: &str = "authr@local";

/// Fixed contract signature (§8): `AUTHR_OS <authr@local>`.
fn signature() -> Result<git2::Signature<'static>, AppError> {
    git2::Signature::now(GIT_SIGNATURE_NAME, GIT_SIGNATURE_EMAIL)
        .map_err(|e| AppError::storage(format!("git signature: {e}")))
}

/// Open or initialize the embedded repository at `root/.git`.
pub fn ensure_repo(root: &Path) -> Result<git2::Repository, AppError> {
    let git_dir = root.join(".git");
    let repo = if git_dir.exists() {
        git2::Repository::open(root)
    } else {
        git2::Repository::init(root)
    }
    .map_err(|e| AppError::storage(format!("git: {e}")))?;

    // .gitignore (idempotent).
    let gi = root.join(".gitignore");
    if !gi.exists() {
        std::fs::write(&gi, GITIGNORE)?;
    }
    Ok(repo)
}

/// Stage everything not ignored and commit with `message`.
/// Returns the commit oid, or None when the workdir is already clean.
pub fn commit_all(root: &Path, message: &str) -> Result<Option<String>, AppError> {
    let repo = ensure_repo(root)?;
    let mut index = repo
        .index()
        .map_err(|e| AppError::storage(format!("git index: {e}")))?;
    index
        .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| AppError::storage(format!("git add: {e}")))?;
    index.write().map_err(|e| AppError::storage(format!("git index write: {e}")))?;
    let tree_id = index
        .write_tree()
        .map_err(|e| AppError::storage(format!("git tree: {e}")))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| AppError::storage(format!("git tree: {e}")))?;

    // Clean workdir? skip the empty commit (untracked files included).
    let dirty = match repo.statuses(None) {
        Ok(statuses) => !statuses.is_empty(),
        Err(_) => true,
    };
    if !dirty {
        return Ok(None);
    }

    let sig = signature()?;
    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = head_commit.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(|e| AppError::storage(format!("git commit: {e}")))?;
    Ok(Some(oid.to_string()))
}

/// Scaffold commit right after `project_init`.
pub fn initial_scaffold_commit(root: &Path) -> Result<String, AppError> {
    commit_all(root, "chore: initial scaffold")?
        .ok_or_else(|| AppError::storage("scaffold commit produced no changes"))
}

/// Chapter auto-commit (§6.4): stages manuscript/ + manifest and commits
/// `Auto-sync chNN`. No-op on a clean workdir or read-only bundles.
pub fn auto_sync_commit(ctx: &crate::ipc::Ctx, chapter_id: i64) -> Result<Option<String>, AppError> {
    if ctx.readonly {
        return Ok(None);
    }
    let nn = format!("{:02}", chapter_id);
    commit_all(&ctx.root, &format!("Auto-sync ch{nn}"))
}

/// Manual snapshot (contract #32).
pub fn manual_snapshot(ctx: &crate::ipc::Ctx, message: Option<&str>) -> Result<String, AppError> {
    let msg = message.unwrap_or("chore: manual snapshot");
    commit_all(&ctx.root, msg)?.ok_or_else(|| AppError::storage("nothing to snapshot — workdir clean"))
}
