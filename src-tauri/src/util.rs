//! Small shared filesystem helpers.

use crate::error::AppError;
use std::path::{Path, PathBuf};

/// Append a textual suffix to a path, preserving the full file name.
/// e.g. `("project.db", "-wal") -> "project.db-wal"`, `("ch01.md", ".tmp") -> "ch01.md.tmp"`.
pub fn suffix_path(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

/// Atomic file write: stage into a sibling `.tmp` file, then commit via
/// filesystem rename (as required for the shadow Markdown/AST layer).
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), AppError> {
    let tmp = suffix_path(path, ".tmp");
    std::fs::write(&tmp, data)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}
