// AUTHR_OS — OS Keychain credential manager (F3).
// Trait with two backends: OS keyring (`keyring-rs`, feature `os-keychain`)
// and a file-backed mock for headless/CI environments
// (`AUTHR_MOCK_KEYCHAIN=1`, build plan §7.2 "zero hardcoded secrets").

use crate::error::AppError;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub trait KeyringBackend: Send + Sync {
    fn store(&self, service: &str, key: &str, value: &str) -> Result<(), AppError>;
    fn get(&self, service: &str, key: &str) -> Result<String, AppError>;
    fn delete(&self, service: &str, key: &str) -> Result<(), AppError>;
}

/// OS-backed backend (real desktops). Compiled only with feature `os-keychain`;
/// without the feature this backend is unavailable and the CLI reports
/// `SECURITY_ERROR` instructing the caller to enable the mock.
#[cfg(feature = "os-keychain")]
pub struct OsKeyring;

#[cfg(feature = "os-keychain")]
impl OsKeyring {
    fn entry(service: &str, key: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(service, key)
            .map_err(|e| AppError::security(format!("keyring entry: {e}")))
    }
}

#[cfg(feature = "os-keychain")]
impl KeyringBackend for OsKeyring {
    fn store(&self, service: &str, key: &str, value: &str) -> Result<(), AppError> {
        let entry = Self::entry(service, key)?;
        entry
            .set_password(value)
            .map_err(|e| AppError::security(format!("keyring set: {e}")))
    }
    fn get(&self, service: &str, key: &str) -> Result<String, AppError> {
        let entry = Self::entry(service, key)?;
        match entry.get_password() {
            Ok(v) => Ok(v),
            Err(keyring::Error::NoEntry) => Err(AppError::not_found(format!(
                "no credential `{service}/{key}` in OS keychain"
            ))),
            Err(e) => Err(AppError::security(format!("keyring get: {e}"))),
        }
    }
    fn delete(&self, service: &str, key: &str) -> Result<(), AppError> {
        let entry = Self::entry(service, key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Err(AppError::not_found(format!(
                "no credential `{service}/{key}` in OS keychain"
            ))),
            Err(e) => Err(AppError::security(format!("keyring delete: {e}"))),
        }
    }
}

/// File-backed mock (headless CI / sandbox). Deterministic and inspectable.
pub struct MockKeyring {
    path: PathBuf,
}

impl MockKeyring {
    pub fn new() -> Self {
        let path = std::env::var("AUTHR_MOCK_KEYRING_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".authr").join("mock_keyring.json")
            });
        MockKeyring { path }
    }

    fn load(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => BTreeMap::new(),
        }
    }

    fn save(&self, map: &BTreeMap<String, BTreeMap<String, String>>) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(map)?;
        // Atomic staged write.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &data)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl Default for MockKeyring {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyringBackend for MockKeyring {
    fn store(&self, service: &str, key: &str, value: &str) -> Result<(), AppError> {
        let mut map = self.load();
        map.entry(service.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
        self.save(&map)
    }
    fn get(&self, service: &str, key: &str) -> Result<String, AppError> {
        let map = self.load();
        map.get(service)
            .and_then(|m| m.get(key))
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("no credential `{service}/{key}` in mock keyring")))
    }
    fn delete(&self, service: &str, key: &str) -> Result<(), AppError> {
        let mut map = self.load();
        let removed = map
            .get_mut(service)
            .and_then(|m| m.remove(key))
            .ok_or_else(|| {
                AppError::not_found(format!("no credential `{service}/{key}` in mock keyring"))
            })?;
        let _ = removed;
        self.save(&map)
    }
}

/// Backend selection per contract §4.9.
pub fn backend() -> Result<Box<dyn KeyringBackend>, AppError> {
    if std::env::var("AUTHR_MOCK_KEYCHAIN").as_deref() == Ok("1") {
        return Ok(Box::new(MockKeyring::new()));
    }
    #[cfg(feature = "os-keychain")]
    {
        Ok(Box::new(OsKeyring))
    }
    #[cfg(not(feature = "os-keychain"))]
    {
        Err(AppError::security(
            "OS keychain backend not compiled into this build (feature `os-keychain`); \
             set AUTHR_MOCK_KEYCHAIN=1 for headless operation",
        ))
    }
}
