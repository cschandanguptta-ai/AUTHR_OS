// AUTHR_OS — Provider settings ladder (contract v1.6, ADR-0012).
//
// One resolution ladder for every live AI endpoint the runtime talks to:
//
//   per-call args  >  project settings (SQLite `provider_settings`)  >  env  >  fail-closed
//
// `provider_get` reports the resolved configuration (keys redacted — only a
// `key_source` badge), `provider_set` validates and persists, `provider_test`
// runs one real generation against the resolved endpoint (never writes
// assets). The hardened fetch used by the R2I render tier and the probe
// shares one path: bounded retries with backoff on transport faults and
// 429/5xx, per-attempt timeout, 64 MiB response cap, PNG/JPEG magic-byte
// sniffing. Fails CLOSED with `AI_ENGINE_UNAVAILABLE` — never fakes output.
//
// API keys NEVER live in the settings table (bundles travel: export/import,
// git shadow layer) — keychain (service `authr_img`) or env only.

use crate::error::AppError;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::Read as _;
use std::time::Duration;

pub const IMG_ENV_ENDPOINT: &str = "AUTHR_IMG_ENDPOINT";
pub const IMG_ENV_MODEL: &str = "AUTHR_IMG_MODEL";
pub const IMG_ENV_API_KEY: &str = "AUTHR_IMG_API_KEY";
pub const IMG_KEYCHAIN_SERVICE: &str = "authr_img";
pub const IMG_KEYCHAIN_KEY: &str = "api_key";
pub const LLM_ENV_SERVER: &str = "AUTHR_LLAMA_SERVER";
pub const IMG_DEFAULT_SIZE: &str = "1024x1024";

const IMG_DEFAULT_TIMEOUT_S: i64 = 120;
const IMG_DEFAULT_RETRIES: i64 = 1;
const IMG_TIMEOUT_MIN_S: i64 = 5;
const IMG_TIMEOUT_MAX_S: i64 = 300;
const IMG_RETRIES_MAX: i64 = 3;
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const URL_MAX_LEN: usize = 2048;
/// Statuses worth another attempt (transport faults always are).
const RETRYABLE_STATUS: [u16; 5] = [429, 500, 502, 503, 504];

// ── settings table ───────────────────────────────────────────────────────────

/// Idempotent migration (mirrors `video::derive::ensure_video_schema`).
pub fn ensure_provider_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS provider_settings (
            section    TEXT NOT NULL,
            key        TEXT NOT NULL,
            value      TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (section, key)
        );",
    )
    .map_err(|e| AppError::storage(format!("provider_settings schema: {e}")))?;
    Ok(())
}

fn settings_get(conn: &Connection, section: &str, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM provider_settings WHERE section = ?1 AND key = ?2",
        [section, key],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

fn settings_set(conn: &Connection, section: &str, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO provider_settings (section, key, value, updated_at)
         VALUES (?1, ?2, ?3, datetime('now'))
         ON CONFLICT (section, key) DO UPDATE SET value = ?3, updated_at = datetime('now')",
        [section, key, value],
    )
    .map_err(|e| AppError::storage(format!("provider_settings write: {e}")))?;
    Ok(())
}

fn settings_del(conn: &Connection, section: &str, key: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM provider_settings WHERE section = ?1 AND key = ?2",
        [section, key],
    )
    .map_err(|e| AppError::storage(format!("provider_settings delete: {e}")))?;
    Ok(())
}

// ── resolution ladder ────────────────────────────────────────────────────────

/// Validated image-provider configuration (the key is held, never echoed).
#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub endpoint_url: Option<String>,
    pub endpoint_source: &'static str, // "settings" | "env" | "none"
    pub model: String,
    pub model_source: &'static str,
    pub timeout_s: i64,
    pub retries: i64,
    pub api_key: Option<String>,
    pub key_source: &'static str, // "env" | "keychain" | "none"
}

/// Full ladder for the image provider. Never fails: `endpoint_url = None`
/// means fail-closed (callers decide — the R2I tier errors, the probe reports).
pub fn effective_image(conn: &Connection) -> Result<ImageConfig, AppError> {
    ensure_provider_schema(conn)?;
    let (endpoint_url, endpoint_source) = match settings_get(conn, "image", "endpoint_url") {
        Some(url) => (Some(url), "settings"),
        None => match std::env::var(IMG_ENV_ENDPOINT).ok().filter(|s| !s.is_empty()) {
            Some(url) => (Some(url), "env"),
            None => (None, "none"),
        },
    };
    let (model, model_source) = match settings_get(conn, "image", "model") {
        Some(m) if !m.is_empty() => (m, "settings"),
        _ => match std::env::var(IMG_ENV_MODEL).ok().filter(|s| !s.is_empty()) {
            Some(m) => (m, "env"),
            None => (String::new(), "none"),
        },
    };
    let timeout_s = settings_get(conn, "image", "timeout_s")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(IMG_DEFAULT_TIMEOUT_S)
        .clamp(IMG_TIMEOUT_MIN_S, IMG_TIMEOUT_MAX_S);
    let retries = settings_get(conn, "image", "retries")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(IMG_DEFAULT_RETRIES)
        .clamp(0, IMG_RETRIES_MAX);
    let (api_key, key_source) = match std::env::var(IMG_ENV_API_KEY).ok().filter(|s| !s.is_empty()) {
        Some(k) => (Some(k), "env"),
        None => {
            let via_keychain = crate::security::keychain::backend()
                .ok()
                .and_then(|b| b.get(IMG_KEYCHAIN_SERVICE, IMG_KEYCHAIN_KEY).ok())
                .filter(|v| !v.is_empty());
            match via_keychain {
                Some(k) => (Some(k), "keychain"),
                None => (None, "none"),
            }
        }
    };
    Ok(ImageConfig {
        endpoint_url,
        endpoint_source,
        model,
        model_source,
        timeout_s,
        retries,
        api_key,
        key_source,
    })
}

/// LLM server ladder: project settings > `AUTHR_LLAMA_SERVER` env > None
/// (None keeps the offline grammar engine — llm.rs semantics untouched).
pub fn resolve_llm_server(conn: &Connection) -> Option<String> {
    if ensure_provider_schema(conn).is_ok() {
        if let Some(url) = settings_get(conn, "llm", "server_url").filter(|s| !s.is_empty()) {
            return Some(url);
        }
    }
    std::env::var(LLM_ENV_SERVER).ok().filter(|s| !s.is_empty())
}

// ── validation ───────────────────────────────────────────────────────────────

/// http(s) URLs only; no userinfo (`user:pass@host`), no whitespace,
/// bounded length. Pragmatic std-only check (the runtime carries no url crate).
pub fn validate_url(url: &str) -> Result<(), AppError> {
    let invalid = |why: &str| AppError::ai(format!("invalid provider endpoint url: {why}"));
    if url.len() > URL_MAX_LEN {
        return Err(invalid("longer than 2048 chars"));
    }
    if url.chars().any(char::is_whitespace) {
        return Err(invalid("contains whitespace"));
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| invalid("must start with http:// or https://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(invalid("no host"));
    }
    if authority.contains('@') {
        return Err(invalid("userinfo (user:pass@) is not allowed — put credentials in the keychain"));
    }
    Ok(())
}

fn validate_retries(v: i64) -> Result<i64, AppError> {
    if !(0..=IMG_RETRIES_MAX).contains(&v) {
        return Err(AppError::ai(format!(
            "retries must be 0..={IMG_RETRIES_MAX}, got {v}"
        )));
    }
    Ok(v)
}

fn validate_timeout(v: i64) -> Result<i64, AppError> {
    if !(IMG_TIMEOUT_MIN_S..=IMG_TIMEOUT_MAX_S).contains(&v) {
        return Err(AppError::ai(format!(
            "timeout_s must be {IMG_TIMEOUT_MIN_S}..={IMG_TIMEOUT_MAX_S}, got {v}"
        )));
    }
    Ok(v)
}

// ── hardened fetch (shared by video_render + provider_test) ─────────────────

enum Attempt {
    Ok(Vec<u8>, &'static str),
    /// HTTP status + short body snippet (transport faults use None status).
    Fail(Option<u16>, String),
}

impl std::fmt::Debug for Attempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Attempt::Ok(bytes, ext) => f.debug_tuple("Ok").field(&bytes.len()).field(ext).finish(),
            Attempt::Fail(s, m) => f.debug_tuple("Fail").field(s).field(m).finish(),
        }
    }
}

/// Pure retry decision — unit-tested. `kind`: "ok", "status:<code>", "transport".
fn should_retry(kind: &str, attempt: u32, max_retries: u32) -> bool {
    if attempt >= max_retries {
        return false;
    }
    if kind == "transport" {
        return true;
    }
    if let Some(code) = kind.strip_prefix("status:") {
        if let Ok(n) = code.parse::<u16>() {
            return RETRYABLE_STATUS.contains(&n);
        }
    }
    false
}

fn retry_delay_ms(attempt: u32) -> u64 {
    // 500 ms, 1 s, 2 s, 4 s (capped).
    500u64.saturating_mul(1u64 << attempt.min(3)).min(4000)
}

fn parse_image_response(bytes: &[u8]) -> Result<(Vec<u8>, &'static str), Attempt> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| {
        Attempt::Fail(
            None,
            format!("image endpoint returned non-JSON body: {e}"),
        )
    })?;
    let b64 = v["data"][0]["b64_json"].as_str().ok_or_else(|| {
        Attempt::Fail(None, "image endpoint returned no data[0].b64_json".into())
    })?;
    let raw = decode_b64(b64).ok_or_else(|| {
        Attempt::Fail(None, "image endpoint b64_json failed to decode".into())
    })?;
    let ext = if raw.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if raw.starts_with(&[0xFF, 0xD8]) {
        "jpg"
    } else {
        return Err(Attempt::Fail(
            None,
            "image endpoint returned neither PNG nor JPEG — refusing to register it".into(),
        ));
    };
    Ok((raw, ext))
}

fn attempt_once(cfg: &ImageConfig, prompt: &str, size: &str) -> Attempt {
    let url = match cfg.endpoint_url.as_deref() {
        Some(u) => u,
        None => return Attempt::Fail(None, "no endpoint configured".into()),
    };
    let mut body = json!({ "prompt": prompt, "size": size, "n": 1, "response_format": "b64_json" });
    if !cfg.model.is_empty() {
        body["model"] = json!(cfg.model);
    }
    let mut req = ureq::post(url)
        .timeout(Duration::from_secs(cfg.timeout_s.max(1) as u64))
        .set("Content-Type", "application/json");
    if let Some(key) = &cfg.api_key {
        req = req.set("Authorization", &format!("Bearer {key}"));
    }
    match req.send_string(&body.to_string()) {
        Ok(resp) => {
            let mut bytes = Vec::new();
            if resp
                .into_reader()
                .take(MAX_RESPONSE_BYTES)
                .read_to_end(&mut bytes)
                .is_err()
            {
                return Attempt::Fail(None, "image endpoint read failed".into());
            }
            match parse_image_response(&bytes) {
                Ok(pair) => Attempt::Ok(pair.0, pair.1),
                Err(fail) => fail,
            }
        }
        Err(ureq::Error::Status(code, resp)) => {
            let mut snippet = String::new();
            let _ = resp
                .into_reader()
                .take(512)
                .read_to_string(&mut snippet);
            Attempt::Fail(Some(code), snippet.chars().take(160).collect())
        }
        Err(e) => Attempt::Fail(None, format!("image endpoint transport: {e}")),
    }
}

/// Retry loop over the hardened attempt path. Fails CLOSED: the error is the
/// canonical `AI_ENGINE_UNAVAILABLE` and no placeholder asset is produced.
pub fn fetch_image_with(
    cfg: &ImageConfig,
    prompt: &str,
    size: &str,
) -> Result<(Vec<u8>, &'static str), AppError> {
    let endpoint = cfg.endpoint_url.clone().ok_or_else(|| {
        AppError::ai_engine_unavailable(format!(
            "video image engine `R2I_ENDPOINT` requires a live provider — configure it with \
             `provider_set` (section image, endpoint_url) or env {IMG_ENV_ENDPOINT} \
             (OpenAI-compatible images endpoint); offline builds use engine=SCRIPTED"
        ))
    })?;
    if validate_url(&endpoint).is_err() {
        return Err(AppError::ai_engine_unavailable(format!(
            "configured image provider endpoint is invalid ({endpoint}) — fix it with provider_set \
             or clear it (env {IMG_ENV_ENDPOINT} takes over when no setting exists)"
        )));
    }
    let max = cfg.retries.max(0) as u32;
    let mut attempt: u32 = 0;
    loop {
        match attempt_once(cfg, prompt, size) {
            Attempt::Ok(raw, ext) => return Ok((raw, ext)),
            Attempt::Fail(status, msg) => {
                let kind = match status {
                    Some(c) => format!("status:{c}"),
                    None => "transport".to_string(),
                };
                if should_retry(&kind, attempt, max) {
                    std::thread::sleep(Duration::from_millis(retry_delay_ms(attempt)));
                    attempt += 1;
                    continue;
                }
                let detail = match status {
                    Some(c) => format!("image endpoint {endpoint} responded {c}: {msg}"),
                    None => msg,
                };
                return Err(AppError::ai_engine_unavailable(detail));
            }
        }
    }
}

/// Resolve through the full ladder, then fetch. The resolved config rides
/// along so callers (the asset registry) can record which source answered.
pub fn fetch_image_resolved(
    conn: &Connection,
    prompt: &str,
    size: &str,
) -> Result<(Vec<u8>, &'static str, ImageConfig), AppError> {
    let cfg = effective_image(conn)?;
    let (raw, ext) = fetch_image_with(&cfg, prompt, size)?;
    Ok((raw, ext, cfg))
}

// ── IPC payloads ─────────────────────────────────────────────────────────────

fn image_payload(conn: &Connection) -> Result<Value, AppError> {
    let cfg = effective_image(conn)?;
    Ok(json!({
        "settings": {
            "endpoint_url": settings_get(conn, "image", "endpoint_url"),
            "model": settings_get(conn, "image", "model"),
            "timeout_s": settings_get(conn, "image", "timeout_s").and_then(|v| v.parse::<i64>().ok()),
            "retries": settings_get(conn, "image", "retries").and_then(|v| v.parse::<i64>().ok()),
        },
        "resolved": {
            "endpoint_url": cfg.endpoint_url,
            "endpoint_source": cfg.endpoint_source,
            "model": if cfg.model.is_empty() { Value::Null } else { json!(cfg.model) },
            "model_source": cfg.model_source,
            "timeout_s": cfg.timeout_s,
            "retries": cfg.retries,
            "key_configured": cfg.api_key.is_some(),
            "key_source": cfg.key_source,
        },
    }))
}

fn llm_payload(conn: &Connection) -> Result<Value, AppError> {
    ensure_provider_schema(conn)?;
    let stored = settings_get(conn, "llm", "server_url").filter(|s| !s.is_empty());
    let env_url = std::env::var(LLM_ENV_SERVER).ok().filter(|s| !s.is_empty());
    let resolved = stored.clone().or(env_url.clone());
    Ok(json!({
        "settings": { "server_url": stored },
        "resolved": {
            "server_url": resolved,
            "source": if stored.is_some() { "settings" } else if env_url.is_some() { "env" } else { "offline" },
        },
    }))
}

/// `provider_get` — settings + redacted resolution (never the key itself).
pub fn get(conn: &Connection) -> Result<Value, AppError> {
    Ok(json!({
        "image": image_payload(conn)?,
        "llm": llm_payload(conn)?,
        "ladder": "args > settings > env > fail-closed",
    }))
}

/// `provider_set` — validate + persist; `null` clears a key. Sections share
/// one call: any recognized key present in args is applied.
pub fn set(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    ensure_provider_schema(conn)?;
    let mut applied: Vec<String> = Vec::new();
    if let Some(v) = a.get("endpoint_url") {
        match v.as_str() {
            Some(url) if !url.is_empty() => {
                validate_url(url)?;
                settings_set(conn, "image", "endpoint_url", url)?;
            }
            _ => settings_del(conn, "image", "endpoint_url")?,
        }
        applied.push("image.endpoint_url".into());
    }
    if let Some(v) = a.get("model") {
        match v.as_str() {
            Some(m) if !m.is_empty() => settings_set(conn, "image", "model", m)?,
            _ => settings_del(conn, "image", "model")?,
        }
        applied.push("image.model".into());
    }
    if let Some(v) = a.get("timeout_s") {
        match v.as_i64() {
            Some(t) => settings_set(conn, "image", "timeout_s", &validate_timeout(t)?.to_string())?,
            None => settings_del(conn, "image", "timeout_s")?,
        }
        applied.push("image.timeout_s".into());
    }
    if let Some(v) = a.get("retries") {
        match v.as_i64() {
            Some(r) => settings_set(conn, "image", "retries", &validate_retries(r)?.to_string())?,
            None => settings_del(conn, "image", "retries")?,
        }
        applied.push("image.retries".into());
    }
    if let Some(v) = a.get("server_url") {
        match v.as_str() {
            Some(url) if !url.is_empty() => {
                validate_url(url)?;
                settings_set(conn, "llm", "server_url", url)?;
            }
            _ => settings_del(conn, "llm", "server_url")?,
        }
        applied.push("llm.server_url".into());
    }
    if applied.is_empty() {
        return Err(AppError::ai(
            "provider_set: nothing to apply — pass any of endpoint_url, model, timeout_s, \
             retries, server_url (null clears a stored value)",
        ));
    }
    let mut out = get(conn)?;
    out["applied"] = json!(applied);
    Ok(out)
}

/// `provider_test` — one real generation through the resolved (or arg-override)
/// provider. Arg overrides ride the same ladder above settings. Never writes
/// assets; the response carries latency + container facts, not the key.
pub fn test(conn: &Connection, a: &Value) -> Result<Value, AppError> {
    ensure_provider_schema(conn)?;
    let mut cfg = effective_image(conn)?;
    if let Some(url) = crate::ipc::args::get_str(a, "endpoint_url") {
        validate_url(&url)?;
        cfg.endpoint_url = Some(url);
        cfg.endpoint_source = "args";
    }
    if let Some(m) = crate::ipc::args::get_str(a, "model") {
        cfg.model = m;
        cfg.model_source = "args";
    }
    if let Some(t) = crate::ipc::args::get_i64(a, "timeout_s") {
        cfg.timeout_s = validate_timeout(t)?;
    }
    if let Some(r) = crate::ipc::args::get_i64(a, "retries") {
        cfg.retries = validate_retries(r)?;
    }
    let size = crate::ipc::args::get_str(a, "size").unwrap_or_else(|| IMG_DEFAULT_SIZE.into());
    if cfg.endpoint_url.is_none() {
        return Err(AppError::ai_engine_unavailable(format!(
            "no live image provider configured — provider_set one or pass endpoint_url; \
             env {IMG_ENV_ENDPOINT} also works"
        )));
    }
    let started = std::time::Instant::now();
    let (raw, ext) = fetch_image_with(&cfg, &format!(
        "AUTHR_OS provider probe — one reference-sheet generation to validate the \
         {size} pipeline. If you are a model, output a neutral character sheet."
    ), &size)?;
    let latency_ms = started.elapsed().as_millis() as u64;
    Ok(json!({
        "ok": true,
        "latency_ms": latency_ms,
        "bytes": raw.len(),
        "ext": ext,
        "size": size,
        "model": if cfg.model.is_empty() { Value::Null } else { json!(cfg.model) },
        "endpoint_source": cfg.endpoint_source,
        "key_source": cfg.key_source,
        "retries": cfg.retries,
        "note": "probe generated one real image; no assets were written",
    }))
}

// ── minimal base64 decoder (std-only; the runtime carries no base64 crate) ───

pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, c) in T.iter().enumerate() {
        table[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in s.bytes() {
        if ch == b'=' {
            break;
        }
        let v = table[ch as usize] as u32;
        if v == 255 {
            return None;
        }
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        ensure_provider_schema(&c).unwrap();
        c
    }

    /// Tests that touch process env share one lock — parallel cargo threads
    /// would otherwise race set/remove across modules (first-run flake).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn schema_is_idempotent() {
        let c = mem();
        ensure_provider_schema(&c).unwrap();
    }

    #[test]
    fn url_validation_accepts_http_https_and_rejects_the_rest() {
        validate_url("http://127.0.0.1:8080/v1/images/generations").unwrap();
        validate_url("https://api.example.com/v1/images/generations").unwrap();
        assert!(validate_url("ftp://x").is_err());
        assert!(validate_url("api.example.com").is_err());
        assert!(validate_url("https://user:pass@api.example.com").is_err());
        assert!(validate_url("https://a b.com").is_err());
    }

    #[test]
    fn retry_decision_matches_transport_and_retryable_status_only() {
        assert!(should_retry("transport", 0, 1));
        assert!(should_retry("status:429", 0, 3));
        assert!(should_retry("status:503", 1, 3));
        assert!(!should_retry("status:503", 2, 2)); // budget exhausted
        assert!(!should_retry("status:401", 0, 3)); // permanent
        assert!(!should_retry("status:422", 0, 3));
        assert!(!should_retry("ok", 0, 3));
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        assert_eq!(retry_delay_ms(0), 500);
        assert_eq!(retry_delay_ms(1), 1000);
        assert_eq!(retry_delay_ms(2), 2000);
        assert_eq!(retry_delay_ms(9), 4000);
    }

    #[test]
    fn ladder_settings_beat_env_and_none_fails_closed() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(IMG_ENV_ENDPOINT);
        let c = mem();
        let cfg = effective_image(&c).unwrap();
        assert_eq!(cfg.endpoint_source, "none");
        assert!(cfg.endpoint_url.is_none());

        std::env::set_var(IMG_ENV_ENDPOINT, "http://env-host/v1/images/generations");
        let cfg = effective_image(&c).unwrap();
        assert_eq!(cfg.endpoint_source, "env");
        assert_eq!(cfg.api_key, None);

        settings_set(&c, "image", "endpoint_url", "http://settings-host/v1").unwrap();
        let cfg = effective_image(&c).unwrap();
        assert_eq!(cfg.endpoint_source, "settings");
        assert_eq!(cfg.endpoint_url.as_deref(), Some("http://settings-host/v1"));

        std::env::remove_var(IMG_ENV_ENDPOINT);
        assert!(fetch_image_resolved(&c, "p", "1024x1024").is_err());
    }

    #[test]
    fn env_key_beats_keychain_and_never_leaks_into_get() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let c = mem();
        let dir = std::env::temp_dir().join(format!("authr_prov_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ring = dir.join("ring.json");
        std::fs::write(&ring, "{}").unwrap();
        std::env::set_var("AUTHR_MOCK_KEYCHAIN", "1");
        std::env::set_var("AUTHR_MOCK_KEYRING_FILE", &ring);
        let secret = "sk-super-secret-provider-key";
        crate::security::keychain::backend()
            .unwrap()
            .store(IMG_KEYCHAIN_SERVICE, IMG_KEYCHAIN_KEY, secret)
            .unwrap();
        let cfg = effective_image(&c).unwrap();
        assert_eq!(cfg.key_source, "keychain");

        std::env::set_var(IMG_ENV_API_KEY, "sk-env-key");
        let cfg = effective_image(&c).unwrap();
        assert_eq!(cfg.key_source, "env");

        let payload = get(&c).unwrap();
        let text = payload.to_string();
        assert!(!text.contains(secret), "keychain secret leaked via provider_get");
        assert_eq!(payload["image"]["resolved"]["key_configured"], json!(true));
        assert_eq!(payload["image"]["resolved"]["key_source"], json!("env"));

        std::env::remove_var(IMG_ENV_API_KEY);
        std::env::remove_var("AUTHR_MOCK_KEYCHAIN");
        std::env::remove_var("AUTHR_MOCK_KEYRING_FILE");
        let _ = std::fs::remove_file(ring);
    }

    #[test]
    fn set_validates_and_clear_roundtrips() {
        let c = mem();
        let out = set(&c, &json!({ "endpoint_url": "https://api.example.com/v1/images/generations", "model": "img-1", "timeout_s": 45, "retries": 2 })).unwrap();
        assert_eq!(out["image"]["resolved"]["endpoint_source"], json!("settings"));
        assert_eq!(out["image"]["resolved"]["retries"], json!(2));
        assert!(set(&c, &json!({ "timeout_s": 1 })).is_err());
        assert!(set(&c, &json!({ "retries": 9 })).is_err());
        assert!(set(&c, &json!({ "endpoint_url": "notaurl" })).is_err());
        assert!(set(&c, &json!({})).is_err());
        let out = set(&c, &json!({ "endpoint_url": null })).unwrap();
        assert_eq!(out["image"]["settings"]["endpoint_url"], Value::Null);
        assert_eq!(out["image"]["resolved"]["endpoint_source"], json!("none"));
    }

    #[test]
    fn llm_ladder_settings_then_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(LLM_ENV_SERVER);
        let c = mem();
        assert_eq!(resolve_llm_server(&c), None);
        std::env::set_var(LLM_ENV_SERVER, "http://env-llama:8080");
        assert_eq!(resolve_llm_server(&c).as_deref(), Some("http://env-llama:8080"));
        set(&c, &json!({ "server_url": "http://settings-llama:8080" })).unwrap();
        assert_eq!(resolve_llm_server(&c).as_deref(), Some("http://settings-llama:8080"));
        std::env::remove_var(LLM_ENV_SERVER);
    }

    #[test]
    fn b64_roundtrip_and_garbage() {
        // Known-good: base64("AUTHR") = "QVVUSFI="
        let dec = decode_b64("QVVUSFI=").unwrap();
        assert_eq!(dec, b"AUTHR".to_vec());
        assert!(decode_b64("!!!!").is_none());
    }

    #[test]
    fn parse_image_response_sniffs_containers() {
        // 1x1 transparent PNG
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let body = json!({ "data": [{ "b64_json": png_b64 }] }).to_string();
        let (raw, ext) = match parse_image_response(body.as_bytes()) {
            Ok((raw, ext)) => (raw, ext),
            Err(other) => panic!("expected ok, got {other:?}"),
        };
        assert_eq!(ext, "png");
        assert!(raw.starts_with(&[0x89, b'P']));
        let bad = json!({ "data": [{ "b64_json": "////" }] }).to_string();
        assert!(matches!(
            parse_image_response(bad.as_bytes()),
            Err(Attempt::Fail(_, _))
        ));
        assert!(matches!(
            parse_image_response(b"{}"),
            Err(Attempt::Fail(_, _))
        ));
    }
}
