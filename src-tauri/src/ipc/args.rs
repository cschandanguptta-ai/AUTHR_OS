// AUTHR_OS — JSON args access helpers (contract §1: `--args '{...}'`).

use crate::error::AppError;
use serde_json::Value;

pub fn req_str(a: &Value, key: &str) -> Result<String, AppError> {
    a.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::storage(format!("missing required string arg `{key}`")))
}

pub fn get_str(a: &Value, key: &str) -> Option<String> {
    a.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

pub fn get_str_opt(a: &Value, key: &str) -> Option<String> {
    get_str(a, key)
}

pub fn req_i64(a: &Value, key: &str) -> Result<i64, AppError> {
    a.get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::storage(format!("missing required integer arg `{key}`")))
}

pub fn get_i64(a: &Value, key: &str) -> Option<i64> {
    a.get(key).and_then(|v| v.as_i64())
}

pub fn get_f64(a: &Value, key: &str) -> Option<f64> {
    a.get(key).and_then(|v| v.as_f64())
}
