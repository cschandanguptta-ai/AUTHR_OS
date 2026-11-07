// AUTHR_OS — Bundle-free handlers: keychain (contract §4.9) and the
// sandbox self-test endpoint (§4.10).

use crate::error::IpcResult;
use serde_json::{json, Value};

/// #29 keychain_store
pub fn keychain_store(args: &Value) -> IpcResult {
    let service = crate::ipc::args::req_str(args, "service")?;
    let key = crate::ipc::args::req_str(args, "key")?;
    let value = crate::ipc::args::req_str(args, "value")?;
    let backend = crate::security::keychain::backend()?;
    backend.store(&service, &key, &value)?;
    Ok(json!({ "stored": true }))
}

/// #30 keychain_get
pub fn keychain_get(args: &Value) -> IpcResult {
    let service = crate::ipc::args::req_str(args, "service")?;
    let key = crate::ipc::args::req_str(args, "key")?;
    let backend = crate::security::keychain::backend()?;
    let value = backend.get(&service, &key)?;
    Ok(json!({ "value": value }))
}

/// #31 keychain_delete
pub fn keychain_delete(args: &Value) -> IpcResult {
    let service = crate::ipc::args::req_str(args, "service")?;
    let key = crate::ipc::args::req_str(args, "key")?;
    let backend = crate::security::keychain::backend()?;
    backend.delete(&service, &key)?;
    Ok(json!({ "deleted": true }))
}

/// #34 sandbox_normalize — pure path-normalization / traversal verdict.
/// No filesystem access; the opaque-box suite drives Windows-prefix and
/// traversal boundary vectors through it.
pub fn sandbox_normalize(args: &Value) -> IpcResult {
    let raw = crate::ipc::args::req_str(args, "path")?;
    let normalized = crate::security::sandbox::normalize_raw(&raw)?;
    Ok(json!({
        "normalized": normalized,
        "raw": raw,
        "within_scope": true,
        "cloud_sync_dir": crate::security::sandbox::is_cloud_sync_path(std::path::Path::new(&raw)),
    }))
}
