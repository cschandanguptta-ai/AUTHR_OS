// AUTHR_OS — Content-addressable audio chunk cache (Phase 5; Sprint 9 layout).
//
// Chunks live at `<bundle>/audio/segments/<sha256>.wav` — the exact directory
// contract the Phase 4 lookahead scheduler and DSP rack consume. Keys mix
// engine + voice profile + text so any change forces a fresh chunk; writes
// are staged through a `.tmp` file and atomically renamed (same pattern as
// the shadow layer).

use crate::ai::tts::VoiceProfile;
use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Directory inside the bundle (gitignored, travels in exports per §6).
pub const CACHE_DIR: &str = "audio/segments";

/// SHA-256 chunk key over everything that can change the render.
pub fn chunk_key(engine: &str, voice: &VoiceProfile, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(engine.as_bytes());
    hasher.update(b"|");
    hasher.update(voice.voice_id.as_bytes());
    hasher.update(b"|");
    hasher.update(voice.engine.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:.4}|{:.4}", voice.rate, voice.pitch).as_bytes());
    hasher.update(b"|");
    for (term, ipa) in &voice.ipa_overrides {
        hasher.update(term.as_bytes());
        hasher.update(b"=");
        hasher.update(ipa.as_bytes());
        hasher.update(b";");
    }
    hasher.update(b"|");
    hasher.update(text.as_bytes());
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Bundle-relative path of a chunk.
pub fn relative_path(hash: &str) -> String {
    format!("{CACHE_DIR}/{hash}.wav")
}

/// Absolute path if the chunk file exists on disk (corrupt-cache recovery:
/// a stale row pointing at a deleted file yields `None`).
pub fn lookup(bundle_root: &Path, hash: &str) -> Result<Option<PathBuf>, AppError> {
    let abs = bundle_root.join(relative_path(hash));
    if abs.is_file() {
        Ok(Some(abs))
    } else {
        Ok(None)
    }
}

/// Atomic store: write `<hash>.wav.tmp` then rename onto the final name.
pub fn store(bundle_root: &Path, hash: &str, wav: &[u8]) -> Result<PathBuf, AppError> {
    let dir = bundle_root.join(CACHE_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| AppError::storage(format!("cache dir: {e}")))?;
    let final_path = dir.join(format!("{hash}.wav"));
    let tmp = dir.join(format!("{hash}.wav.tmp"));
    std::fs::write(&tmp, wav).map_err(|e| AppError::storage(format!("cache write: {e}")))?;
    std::fs::rename(&tmp, &final_path)
        .map_err(|e| AppError::storage(format!("cache rename: {e}")))?;
    Ok(final_path)
}

/// (files, bytes) under the cache dir; `.tmp` staging files excluded.
pub fn stats(bundle_root: &Path) -> Result<(i64, i64), AppError> {
    let dir = bundle_root.join(CACHE_DIR);
    if !dir.is_dir() {
        return Ok((0, 0));
    }
    let mut files = 0i64;
    let mut bytes = 0i64;
    let entries = std::fs::read_dir(&dir).map_err(|e| AppError::storage(format!("cache read: {e}")))?;
    for e in entries {
        let e = e.map_err(|e| AppError::storage(format!("cache read: {e}")))?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".wav") {
            continue;
        }
        let meta = e
            .metadata()
            .map_err(|e| AppError::storage(format!("cache read: {e}")))?;
        if meta.is_file() {
            files += 1;
            bytes += meta.len() as i64;
        }
    }
    Ok((files, bytes))
}
