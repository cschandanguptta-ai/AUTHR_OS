// AUTHR_OS — Unified error taxonomy (docs/CLI_CONTRACT.md §3).
// Exactly 9 canonical codes across 3 categories.

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Storage,
    Bundle,
    Security,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Storage => "storage",
            Category::Bundle => "bundle",
            Category::Security => "security",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub category: Category,
}

impl AppError {
    pub fn new(code: &'static str, category: Category, message: impl Into<String>) -> Self {
        AppError { code, message: message.into(), category }
    }

    // ── storage ─────────────────────────────────────────────────────────────
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new("ENTITY_NOT_FOUND", Category::Storage, msg)
    }
    pub fn fk_violation(msg: impl Into<String>) -> Self {
        Self::new("FOREIGN_KEY_VIOLATION", Category::Storage, msg)
    }
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::new("STORAGE_ERROR", Category::Storage, msg)
    }

    // ── bundle ──────────────────────────────────────────────────────────────
    pub fn lease_active(msg: impl Into<String>) -> Self {
        Self::new("LEASE_LOCK_ACTIVE", Category::Bundle, msg)
    }
    pub fn readonly(msg: impl Into<String>) -> Self {
        Self::new("READ_ONLY_PROJECT", Category::Bundle, msg)
    }
    pub fn bundle(msg: impl Into<String>) -> Self {
        Self::new("BUNDLE_ERROR", Category::Bundle, msg)
    }

    // ── security ────────────────────────────────────────────────────────────
    pub fn traversal(msg: impl Into<String>) -> Self {
        Self::new("PATH_TRAVERSAL_VIOLATION", Category::Security, msg)
    }
    pub fn zip_slip(msg: impl Into<String>) -> Self {
        Self::new("ZIP_SLIP_VIOLATION", Category::Security, msg)
    }
    pub fn security(msg: impl Into<String>) -> Self {
        Self::new("SECURITY_ERROR", Category::Security, msg)
    }

    /// Contract envelope fragment (§2): {"code","message","category"}.
    pub fn to_json(&self) -> Value {
        json!({"code": self.code, "message": self.message, "category": self.category.as_str()})
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{} {}] {}", self.category.as_str(), self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        let msg = e.to_string();
        if msg.contains("FOREIGN KEY constraint") {
            AppError::fk_violation(msg)
        } else if let rusqlite::Error::SqliteFailure(_, Some(detail)) = &e {
            if detail.contains("FOREIGN KEY constraint") {
                AppError::fk_violation(msg)
            } else {
                AppError::storage(msg)
            }
        } else {
            AppError::storage(msg)
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::storage(format!("io: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::storage(format!("json: {e}"))
    }
}

/// Handler result type for the IPC dispatcher.
pub type IpcResult = Result<Value, AppError>;
