// AUTHR_OS — Media read access (contract v1.6, §4.19).
//
// Read-only accessor for bundle-local media assets (scene frames, character
// reference sheets, title cards). The Scene Studio storyboard paints real
// pixels without exposing the filesystem:
//
//   * relative path only — absolute paths and `..` traversal are rejected
//     before resolution, and the resolved target must stay inside
//     `<bundle root>/media/` (canonicalized containment check),
//   * image containers only (BMP | PNG | JPEG), 32 MiB cap,
//   * response carries base64 + mime + sha256 so both transports (Tauri IPC,
//     dev-bridge JSON) can carry it; the UI builds a `data:` URL.
//
// No listing, no writes, no derived state — a pure file fact under the same
// sandbox posture as every other bundle-scoped surface.

use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAX_MEDIA_BYTES: u64 = 32 * 1024 * 1024;

const IMAGE_EXTS: [&str; 4] = ["bmp", "png", "jpg", "jpeg"];

const MIME_BY_EXT: [(&str, &str); 4] = [
    ("bmp", "image/bmp"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
];

fn ext_of(p: &str) -> Option<&'static str> {
    let lower = p.to_ascii_lowercase();
    let dot = lower.rfind('.')?;
    let ext = &lower[dot + 1..];
    if !IMAGE_EXTS.contains(&ext) {
        return None;
    }
    Some(MIME_BY_EXT.iter().find(|(e, _)| *e == ext)?.0)
}

fn mime_for(ext: &str) -> &'static str {
    MIME_BY_EXT
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, m)| *m)
        .unwrap_or("application/octet-stream")
}

/// §4.19 `media_read` — one media asset as {mime, bytes, sha256, data_b64}.
pub fn media_read(ctx: &Ctx, a: &Value) -> Result<Value, AppError> {
    let raw = args::req_str(a, "path")?;

    // Security checks FIRST (shape leaks nothing to a hostile path):
    // absolute paths and `..` traversal are rejected before any resolution.
    if raw.starts_with('/') || raw.starts_with('\\') || raw.contains(":\\") {
        return Err(AppError::traversal(
            "media_read: absolute paths are rejected — pass a bundle-relative media path",
        ));
    }
    if raw.split(['/', '\\']).any(|seg| seg == "..") {
        return Err(AppError::traversal(
            "media_read: path traversal (`..`) is rejected",
        ));
    }

    let ext = ext_of(&raw).ok_or_else(|| {
        AppError::not_found(format!(
            "media_read: unsupported image container for `{raw}` (expected .bmp|.png|.jpg|.jpeg)"
        ))
    })?;

    // Registry paths are bundle-relative and already start with `media/` —
    // enforce the prefix instead of re-joining it (which would double it).
    if !(raw.starts_with("media/") || raw.starts_with("media\\")) {
        return Err(AppError::traversal(
            "media_read: only bundle `media/` assets are served",
        ));
    }
    let target = ctx.root.join(&raw);
    // Canonicalized containment: the resolved target must stay inside the
    // bundle root AND inside media/ (the prefix check + canonicalize together
    // defeat symlink escapes too).
    let canonical_root = ctx
        .root
        .canonicalize()
        .map_err(|e| AppError::storage(format!("bundle root: {e}")))?;
    let media_root = canonical_root.join("media");
    let canonical = target
        .canonicalize()
        .map_err(|_| AppError::not_found(format!("media asset not found: {raw}")))?;
    if !canonical.starts_with(&canonical_root) || !canonical.starts_with(&media_root) {
        return Err(AppError::traversal(
            "media_read: resolved path escapes the bundle media tree",
        ));
    }

    let bytes = std::fs::read(&canonical)
        .map_err(|e| AppError::storage(format!("media_read: {e}")))?;
    if bytes.len() as u64 > MAX_MEDIA_BYTES {
        return Err(AppError::security(format!(
            "media_read: asset exceeds the {MAX_MEDIA_BYTES}-byte cap"
        )));
    }
    let sha = hex(&Sha256::digest(&bytes));
    Ok(json!({
        "path": raw,
        "mime": mime_for(ext),
        "bytes": bytes.len(),
        "sha256": sha,
        "data_b64": base64_encode(&bytes),
    }))
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

// ── minimal base64 encoder (std-only; mirrors providers::decode_b64) ─────────

pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_whitelist_and_mime_map() {
        assert_eq!(ext_of("media/video/b1/ch1/scene_0.bmp"), Some("bmp"));
        assert_eq!(ext_of("media/x.PNG"), Some("png"));
        assert_eq!(ext_of("media/x.jpeg"), Some("jpeg"));
        assert_eq!(ext_of("media/x.exe"), None);
        assert_eq!(ext_of("media/noext"), None);
        assert_eq!(mime_for("bmp"), "image/bmp");
        assert_eq!(mime_for("jpg"), "image/jpeg");
        assert_eq!(mime_for("jpeg"), "image/jpeg");
    }

    #[test]
    fn base64_known_vectors_and_padding() {
        assert_eq!(base64_encode(b"AUTHR"), "QVVUSFI=");
        assert_eq!(base64_encode(b"A"), "QQ==");
        assert_eq!(base64_encode(b"AB"), "QUI=");
        assert_eq!(base64_encode(b"ABC"), "QUJD");
        // Roundtrip against the provider decoder.
        let raw: Vec<u8> = (0..255u8).collect();
        assert_eq!(crate::providers::decode_b64(&base64_encode(&raw)).unwrap(), raw);
    }
}
