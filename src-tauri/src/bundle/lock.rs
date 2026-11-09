// AUTHR_OS — `.authr_lock` lease lock state machine (§6.2 step 1, F11):
// 120 s TTL, PID liveness recovery, cross-host isolation, read-only mode.

use crate::error::AppError;
use serde_json::{json, Value};
use std::path::Path;

pub const LEASE_TTL_SECONDS: i64 = 120;

#[derive(Debug, Clone, PartialEq)]
pub struct Lease {
    pub pid: i64,
    pub hostname: String,
    pub user: String,
    pub locked_at: String,
    pub lease_expires_at: String,
    pub session_id: String,
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn iso_plus(secs: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(secs))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn parse_ts(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok()
}

pub fn lock_path(root: &Path) -> std::path::PathBuf {
    root.join(".authr_lock")
}

impl Lease {
    pub fn acquire() -> Self {
        Lease {
            pid: std::process::id() as i64,
            hostname: whoami::fallible::hostname().unwrap_or_else(|_| "unknown".into()),
            user: whoami::username(),
            locked_at: iso_now(),
            lease_expires_at: iso_plus(LEASE_TTL_SECONDS),
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "pid": self.pid,
            "hostname": self.hostname,
            "user": self.user,
            "locked_at": self.locked_at,
            "lease_expires_at": self.lease_expires_at,
            "session_id": self.session_id,
        })
    }

    pub fn write(&self, root: &Path) -> Result<(), AppError> {
        let data = serde_json::to_vec_pretty(&self.to_json())?;
        std::fs::write(lock_path(root), &data)?;
        Ok(())
    }

    pub fn read(root: &Path) -> Result<Option<Lease>, AppError> {
        let p = lock_path(root);
        if !p.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&p)?;
        if bytes.is_empty() {
            return Err(AppError::bundle("empty lock file (corrupt lease)"));
        }
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|_| AppError::bundle("lock file is not valid JSON (corrupt lease)"))?;
        let lease = Lease {
            pid: v.get("pid").and_then(|x| x.as_i64()).ok_or_else(|| {
                AppError::bundle("lock missing `pid` (corrupt lease)")
            })?,
            hostname: v
                .get("hostname")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            user: v
                .get("user")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            locked_at: v
                .get("locked_at")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            lease_expires_at: v
                .get("lease_expires_at")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            session_id: v
                .get("session_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        };
        Ok(Some(lease))
    }
}

pub fn local_hostname() -> String {
    whoami::fallible::hostname().unwrap_or_else(|_| "unknown".into())
}

pub fn is_expired(lease: &Lease) -> bool {
    match parse_ts(&lease.lease_expires_at) {
        Some(exp) => exp < chrono::Utc::now(),
        None => true, // unparseable expiry = stale
    }
}

fn pid_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    // /proc on Linux; on other platforms a lock from this host is trusted.
    let proc_dir = std::path::PathBuf::from(format!("/proc/{pid}"));
    if proc_dir.exists() {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Classify the current lock state of a bundle.
#[derive(Debug, Clone, PartialEq)]
pub enum LockState {
    Unlocked,
    /// Fresh lease held by a live local process (or a foreign host).
    ForeignActive(Lease),
    /// Dead local PID / expired TTL / corrupt content — reclaimable.
    Stale(Option<Lease>),
}

pub fn classify(root: &Path) -> Result<LockState, AppError> {
    let p = lock_path(root);
    if !p.exists() {
        return Ok(LockState::Unlocked);
    }
    let lease = match Lease::read(root) {
        Ok(Some(l)) => l,
        Ok(None) => return Ok(LockState::Unlocked),
        Err(_) => return Ok(LockState::Stale(None)), // corrupt
    };
    if is_expired(&lease) {
        return Ok(LockState::Stale(Some(lease)));
    }
    let same_host = lease.hostname == local_hostname();
    if same_host && !pid_alive(lease.pid) {
        return Ok(LockState::Stale(Some(lease)));
    }
    // Fresh + (foreign host OR live local pid) → active.
    Ok(LockState::ForeignActive(lease))
}

/// Remove a stale lock file (recovery). Refuses to touch a foreign active lease.
pub fn recover_stale(root: &Path) -> Result<(), AppError> {
    match classify(root)? {
        LockState::Unlocked => Ok(()),
        LockState::ForeignActive(_) => Err(AppError::lease_active(
            "cannot recover: lease is fresh and held by an active process",
        )),
        LockState::Stale(_) => {
            let _ = std::fs::remove_file(lock_path(root));
            Ok(())
        }
    }
}

/// Take the lease for this process. Returns Err(LEASE_LOCK_ACTIVE) on conflict.
pub fn acquire(root: &Path) -> Result<Lease, AppError> {
    match classify(root)? {
        LockState::ForeignActive(l) => Err(AppError::lease_active(format!(
            "lease held by pid {} on {} until {}",
            l.pid, l.hostname, l.lease_expires_at
        ))),
        LockState::Stale(_) => {
            let _ = std::fs::remove_file(lock_path(root));
            let lease = Lease::acquire();
            lease.write(root)?;
            Ok(lease)
        }
        LockState::Unlocked => {
            let lease = Lease::acquire();
            lease.write(root)?;
            Ok(lease)
        }
    }
}

/// Release a lease we (or a stale process) hold. Foreign active → left intact.
pub fn release(root: &Path) -> Result<bool, AppError> {
    match classify(root)? {
        LockState::Unlocked => Ok(false),
        LockState::ForeignActive(l) => {
            let ours = l.hostname == local_hostname() && l.pid == std::process::id() as i64;
            if ours {
                let _ = std::fs::remove_file(lock_path(root));
                Ok(true)
            } else {
                Ok(false)
            }
        }
        LockState::Stale(_) => {
            let _ = std::fs::remove_file(lock_path(root));
            Ok(true)
        }
    }
}

/// Read-only inspection payload (contract #4).
pub fn status_json(root: &Path) -> Result<Value, AppError> {
    match classify(root)? {
        LockState::Unlocked => Ok(json!({ "locked": false, "readonly": false })),
        LockState::Stale(maybe) => Ok(json!({
            "locked": true,
            "readonly": false,
            "expired": true,
            "pid": maybe.as_ref().map(|l| l.pid),
            "hostname": maybe.as_ref().map(|l| l.hostname.clone()),
            "user": maybe.as_ref().map(|l| l.user.clone()),
            "locked_at": maybe.as_ref().map(|l| l.locked_at.clone()),
            "lease_expires_at": maybe.as_ref().map(|l| l.lease_expires_at.clone()),
            "session_id": maybe.as_ref().map(|l| l.session_id.clone()),
        })),
        LockState::ForeignActive(l) => Ok(json!({
            "locked": true,
            "readonly": true,
            "expired": false,
            "pid": l.pid,
            "hostname": l.hostname,
            "user": l.user,
            "locked_at": l.locked_at,
            "lease_expires_at": l.lease_expires_at,
            "session_id": l.session_id,
        })),
    }
}

/// Guard for mutating commands: fresh foreign lease ⇒ READ_ONLY_PROJECT.
pub fn ensure_writable(root: &Path) -> Result<(), AppError> {
    match classify(root)? {
        LockState::Unlocked => Ok(()),
        LockState::Stale(_) => {
            let _ = std::fs::remove_file(lock_path(root));
            Ok(())
        }
        LockState::ForeignActive(l) => Err(AppError::readonly(format!(
            "bundle is open read-only: active lease held by pid {} on {} until {}",
            l.pid, l.hostname, l.lease_expires_at
        ))),
    }
}
