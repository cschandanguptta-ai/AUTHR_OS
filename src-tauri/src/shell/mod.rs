// AUTHR_OS — Tauri v2 desktop shell (Phase 2 plumbing, compiled with
// feature `gui` on real desktop targets). Every `#[tauri::command]` delegates
// to the same frozen dispatcher, so GUI, CLI and tests share one contract.

use serde_json::Value;

/// Generic pass-through command: the frontend calls
/// `invoke('authr_command', { command, bundle, args })`.
#[tauri::command]
pub fn authr_command(
    command: String,
    bundle: Option<String>,
    args: Option<Value>,
) -> Result<Value, Value> {
    let root = bundle.map(std::path::PathBuf::from);
    match crate::ipc::dispatch(root.as_deref(), &command, &args.unwrap_or(Value::Null)) {
        Ok(data) => Ok(serde_json::json!({ "ok": true, "data": data })),
        Err(err) => Err(err.to_json()),
    }
}

/// Run the Tauri app (requires `gui` feature and a desktop environment).
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![authr_command])
        .run(tauri::generate_context!())
        .expect("AUTHR_OS desktop shell failed to start");
}
