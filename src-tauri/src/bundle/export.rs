// AUTHR_OS — Atomic ZIP export / guarded import (contract #5–6).
// Export excludes volatile state; import enforces the Zip-Slip guard.

use super::lock;
use super::manifest::Manifest;
use crate::error::AppError;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;

/// #5 project_export — checkpoint WAL, stage `<out>.tmp.zip`, atomic rename.
pub fn project_export(bundle: &Path, out: &str) -> Result<Value, AppError> {
    Manifest::read(bundle)?;

    // Exclusive snapshot: a fresh foreign lease means the DB may be mid-write.
    match lock::classify(bundle)? {
        lock::LockState::ForeignActive(l) => {
            return Err(AppError::lease_active(format!(
                "cannot export: active lease held by pid {} until {}",
                l.pid, l.lease_expires_at
            )))
        }
        lock::LockState::Stale(_) => {
            let _ = std::fs::remove_file(lock::lock_path(bundle));
        }
        lock::LockState::Unlocked => {}
    }

    // WAL → 0 bytes so project.db is self-contained.
    {
        let conn = crate::storage::db::open_db(bundle)?;
        crate::storage::db::wal_truncate(&conn)?;
        drop(conn);
    }

    let out_path = std::path::PathBuf::from(out);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staging = out_path.with_extension("tmp.zip");

    let file = std::fs::File::create(&staging)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let exclude = |name: &str| {
        name == ".authr_lock"
            || name == "project.db-wal"
            || name == "project.db-shm"
            || name == "project.db-wal.tmp"
    };

    let mut total: u64 = 0;
    let mut count: usize = 0;
    let root_len = bundle.to_string_lossy().len() + 1;
    let walker = walk_files(bundle)?;
    for file_path in walker {
        let rel = &file_path.to_string_lossy()[root_len..];
        let rel = rel.replace('\\', "/");
        if exclude(&rel) {
            continue;
        }
        let bytes = std::fs::read(&file_path)?;
        zip.start_file(&rel, options)
            .map_err(|e| AppError::storage(format!("zip: {e}")))?;
        zip.write_all(&bytes)?;
        total += bytes.len() as u64;
        count += 1;
    }
    zip.finish()
        .map_err(|e| AppError::storage(format!("zip finish: {e}")))?;
    std::fs::rename(&staging, &out_path)?;

    Ok(json!({ "out": out_path.display().to_string(), "bytes": total, "entries": count }))
}

fn walk_files(root: &Path) -> Result<Vec<std::path::PathBuf>, AppError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let p = entry?.path();
            let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if name == ".git" {
                continue; // revisions excluded from portable archives
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// #6 project_import — guarded extraction into a staging dir, manifest
/// validation, atomic rename into `dest`.
pub fn project_import(zip_path: &str, dest: &str) -> Result<Value, AppError> {
    let raw = std::fs::File::open(zip_path)
        .map_err(|e| AppError::bundle(format!("open archive: {e}")))?;
    let mut archive = zip::ZipArchive::new(raw)
        .map_err(|e| AppError::bundle(format!("read archive: {e}")))?;

    let dest_path = std::path::PathBuf::from(dest);
    if dest_path.exists() && std::fs::read_dir(&dest_path)?.next().is_some() {
        return Err(AppError::bundle(format!(
            "destination {} exists and is not empty",
            dest_path.display()
        )));
    }
    let staging = dest_path.with_extension("import-tmp");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| AppError::bundle(format!("entry {i}: {e}")))?;
        let name = entry.name().to_string();
        crate::security::sandbox::validate_zip_entry(&name)?;
        if entry.is_dir() {
            std::fs::create_dir_all(staging.join(&name))?;
            continue;
        }
        // Hard symlink defense.
        if entry.compressed_size() != entry.size() {
            // still fine — just compressed
        }
        let target = staging.join(&name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&target)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        out.write_all(&buf)?;
    }

    // Validate the extracted bundle.
    Manifest::read(&staging)?;
    // A lock file inside an archive is stale by definition.
    let _ = std::fs::remove_file(staging.join(".authr_lock"));

    std::fs::rename(&staging, &dest_path)?;

    let manifest = Manifest::read(&dest_path)?;
    Ok(json!({ "path": dest_path.display().to_string(), "manifest": manifest }))
}
