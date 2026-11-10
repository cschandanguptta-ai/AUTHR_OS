// AUTHR_OS — Bundle manifest v1 (§6.2): identity + schema version.

use crate::error::AppError;
use serde_json::{json, Value};
use std::path::Path;

pub const AUTHR_VERSION: &str = "3.0.0-PROD";
pub const SCHEMA_VERSION: i64 = 1;

pub struct Manifest {
    pub schema_version: i64,
    pub project_id: String,
    pub title: String,
    pub author: String,
    pub created_at: i64,
    pub last_modified_at: i64,
    pub authr_version: String,
}

impl Manifest {
    pub fn new(title: &str, author: &str) -> Self {
        let now = chrono::Utc::now().timestamp();
        Manifest {
            schema_version: SCHEMA_VERSION,
            project_id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            author: author.to_string(),
            created_at: now,
            last_modified_at: now,
            authr_version: AUTHR_VERSION.to_string(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "schema_version": self.schema_version,
            "project_id": self.project_id,
            "title": self.title,
            "author": self.author,
            "created_at": self.created_at,
            "last_modified_at": self.last_modified_at,
            "authr_version": self.authr_version,
        })
    }

    pub fn write(&self, root: &Path) -> Result<Value, AppError> {
        let path = root.join("manifest.json");
        atomic_write(&path, &serde_json::to_vec_pretty(&self.to_json())?)?;
        Ok(self.to_json())
    }

    pub fn read(root: &Path) -> Result<Value, AppError> {
        let path = root.join("manifest.json");
        let bytes = std::fs::read(&path)
            .map_err(|_| AppError::bundle(format!("manifest.json missing at {}", path.display())))?;
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::bundle(format!("manifest.json corrupt: {e}")))?;
        if v.get("schema_version").and_then(|s| s.as_i64()) != Some(SCHEMA_VERSION) {
            return Err(AppError::bundle(format!(
                "unsupported manifest schema_version: {:?}",
                v.get("schema_version")
            )));
        }
        Ok(v)
    }

    pub fn touch(root: &Path) -> Result<(), AppError> {
        let mut m = Manifest::read(root)?;
        m["last_modified_at"] = json!(chrono::Utc::now().timestamp());
        atomic_write(
            &root.join("manifest.json"),
            &serde_json::to_vec_pretty(&m)?,
        )?;
        Ok(())
    }
}

/// Staged `.tmp` write + atomic rename (TEST_INFRA §5.3).
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let tmp = path.with_extension("tmp");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
