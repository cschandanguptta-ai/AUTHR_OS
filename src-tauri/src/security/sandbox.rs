// AUTHR_OS — Path sandboxing, Windows verbatim-prefix normalization,
// percent-decoding, Zip-Slip guard and cloud-sync directory detection.
// (Build plan §7.3; TEST_INFRA features F4 + tier2 boundary vectors.)

use crate::error::AppError;
use std::path::{Path, PathBuf};

/// Normalize a raw path string the way the Windows-native core does before any
/// filesystem access: strip `\\?\` verbatim / `\\?\UNC\` / `\??\` prefixes,
/// percent-decode (rejecting encoded traversal), normalize slashes and drive
/// letter casing. Pure string work — no filesystem access.
pub fn normalize_raw(raw: &str) -> Result<String, AppError> {
    if raw.contains('\0') {
        return Err(AppError::traversal("null byte in path"));
    }

    let mut s = raw.to_string();

    // Strip Windows verbatim prefixes before anything else.
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("\\\\?\\unc\\") {
        s = format!(r"\\{}", &s[8..]);
    } else if lower.starts_with("\\\\?\\") || lower.starts_with("\\??\\") {
        s = s[4..].to_string();
    }

    let decoded = percent_decode(&s)?;

    // Windows-style = contains backslashes or a drive letter prefix.
    let chars: Vec<char> = decoded.chars().collect();
    let has_drive = chars.len() >= 2 && chars[1] == ':' && chars[0].is_ascii_alphabetic();
    let windows_style = decoded.contains('\\') || has_drive;

    if !windows_style {
        // Unix path: forward slashes preserved; reject traversal segments.
        for seg in decoded.split('/').filter(|s| !s.is_empty()) {
            if seg == ".." {
                return Err(AppError::traversal(format!(
                    "path traversal segment `{seg}` in `{raw}`"
                )));
            }
        }
        return Ok(decoded);
    }

    // ── Windows normalization ────────────────────────────────────────────────
    // Normalize separators (Windows accepts both), collapsing duplicate
    // backslashes while preserving a leading UNC `\\`.
    let unified = decoded.replace('/', "\\");
    let mut out = String::new();
    for c in unified.chars() {
        if c == '\\' {
            if out.ends_with('\\') {
                // Allow exactly one leading double backslash (UNC root).
                if out.len() == 1 {
                    out.push('\\');
                }
                continue;
            }
            out.push('\\');
        } else {
            out.push(c);
        }
    }

    // Drive letter casing: `c:` → `C:`.
    let mut result = out;
    let wchars: Vec<char> = result.chars().collect();
    if wchars.len() >= 2 && wchars[1] == ':' && wchars[0].is_ascii_alphabetic() {
        let up = wchars[0].to_ascii_uppercase();
        result = format!("{}{}", up, &result[1..]);
    }

    // Reject traversal segments outright.
    for seg in result.split('\\').filter(|s| !s.is_empty()) {
        if seg == ".." {
            return Err(AppError::traversal(format!(
                "path traversal segment `{seg}` in `{raw}`"
            )));
        }
    }
    Ok(result)
}

fn percent_decode(s: &str) -> Result<String, AppError> {
    if !s.contains('%') {
        return Ok(s.to_string());
    }
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                // Literal `%` — keep verbatim.
                out.push(b'%');
                i += 1;
                continue;
            }
            let hi = (bytes[i + 1] as char).to_digit(16).unwrap() as u16;
            let lo = (bytes[i + 2] as char).to_digit(16).unwrap() as u16;
            out.push(((hi << 4) | lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| AppError::traversal("percent-decode produced invalid UTF-8"))
}

/// Resolve `relative` strictly inside `root`:
/// symlinks rejected, canonical path must stay under root.
pub fn resolve_in_root(root: &Path, relative: &Path) -> Result<PathBuf, AppError> {
    if relative.as_os_str().is_empty() {
        return Ok(root.to_path_buf());
    }
    for comp in relative.components() {
        use std::path::Component::*;
        match comp {
            Normal(_) | RootDir | CurDir => {}
            ParentDir => {
                return Err(AppError::traversal(format!(
                    "`..` escapes the bundle sandbox: {}",
                    relative.display()
                )))
            }
            Prefix(_) => {
                return Err(AppError::traversal(format!(
                    "absolute component in bundle-relative path: {}",
                    relative.display()
                )))
            }
        }
    }
    let joined = root.join(relative);
    // Reject symlinked components (no resolution tricks).
    let mut walked = PathBuf::from(root);
    for comp in relative.components() {
        walked.push(comp);
        if let Ok(meta) = std::fs::symlink_metadata(&walked) {
            if meta.file_type().is_symlink() {
                return Err(AppError::traversal(format!(
                    "symlink component rejected: {}",
                    walked.display()
                )));
            }
        }
    }
    let canon_root = root
        .canonicalize()
        .map_err(|e| AppError::bundle(format!("bundle root missing: {e}")))?;
    let canon = joined
        .canonicalize()
        .map_err(|e| AppError::not_found(format!("path not found: {e}")))?;
    if !canon.starts_with(&canon_root) {
        return Err(AppError::traversal(format!(
            "resolved path {} escapes bundle root {}",
            canon.display(),
            canon_root.display()
        )));
    }
    Ok(canon)
}

/// Zip-Slip guard (build plan §7.3): every archive entry name must be a safe
/// relative path. Rejects absolute paths, drive letters, `..`, backslashes,
/// null bytes and Windows reserved separators.
pub fn validate_zip_entry(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::zip_slip("empty archive entry name"));
    }
    if name.contains('\0') {
        return Err(AppError::zip_slip(format!("null byte in entry `{name}`")));
    }
    if name.starts_with('/') || name.starts_with('\\') {
        return Err(AppError::zip_slip(format!(
            "absolute archive entry `{name}`"
        )));
    }
    if name.contains('\\') {
        return Err(AppError::zip_slip(format!(
            "backslash separator in entry `{name}`"
        )));
    }
    let upper = name.to_ascii_uppercase();
    if upper.len() >= 2 && upper.as_bytes()[1] == b':' {
        return Err(AppError::zip_slip(format!(
            "drive letter in entry `{name}`"
        )));
    }
    for seg in name.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err(AppError::zip_slip(format!(
                "traversal entry `{name}` escapes destination"
            )));
        }
    }
    Ok(())
}

/// Cloud-sync directory heuristics (§6.2 step 2): OneDrive / Dropbox / Google Drive.
pub fn is_cloud_sync_path(p: &Path) -> bool {
    let hay = p.to_string_lossy().to_lowercase();
    hay.contains("onedrive")
        || hay.contains("dropbox")
        || hay.contains("google drive")
        || hay.contains("google_drive")
        || hay.contains("googledrive")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefixes() {
        assert_eq!(normalize_raw(r"\\?\C:\tmp\x").unwrap(), r"C:\tmp\x");
        assert_eq!(normalize_raw(r"\\?\UNC\srv\share").unwrap(), r"\\srv\share");
        assert_eq!(normalize_raw(r"\??\c:\x").unwrap(), r"C:\x");
    }

    #[test]
    fn traversal_rejected() {
        assert!(normalize_raw("a/../../etc").is_err());
        assert!(normalize_raw("%2e%2e/x").is_err());
        assert!(normalize_raw("a\0b").is_err());
    }
}
