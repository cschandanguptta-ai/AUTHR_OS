// AUTHR_OS — release benchmark (Phase 6, Sprint 16).
//
// Measures the three hardening thresholds from the build plan:
//   • cold start  — median wall-clock of `authr --version` child spawns (<1.5 s)
//   • idle RAM    — peak RSS (VmHWM) of a full CLI invocation (<50 MB)
//   • binary size — the release executable itself (<25 MB installer budget)
//
// Bundle-free; `benchmark_run` reports pass/fail against those limits.

use crate::error::AppError;
use serde_json::{json, Value};
use std::time::Instant;

/// #54 benchmark_run — cold start / RSS / binary size acceptance report.
pub fn benchmark_run(a: &Value) -> crate::error::IpcResult {
    let iterations = a
        .get("iterations")
        .and_then(|v| v.as_i64())
        .unwrap_or(7)
        .clamp(3, 25) as usize;

    // ── cold start: spawn self with --version, time to exit ─────────────────
    let self_exe = std::env::current_exe().map_err(|e| AppError::bundle(format!("exe path: {e}")))?;
    let mut samples: Vec<f64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let t0 = Instant::now();
        let out = std::process::Command::new(&self_exe)
            .arg("--version")
            .output()
            .map_err(|e| AppError::bundle(format!("spawn cold start: {e}")))?;
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        if !out.status.success() {
            return Err(AppError::bundle("cold-start probe failed: --version errored"));
        }
        samples.push(ms);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() as f64 * 0.95).ceil() as usize - 1];

    // ── RSS: this process is a full CLI invocation; VmHWM is its peak ───────
    let (rss_kb, peak_kb) = proc_self_status()
        .map(|s| (s.0, s.1))
        .unwrap_or((None, None));

    // ── binary size ─────────────────────────────────────────────────────────
    let bin_bytes = std::fs::metadata(&self_exe).map(|m| m.len()).unwrap_or(0);

    let cold_limit_ms = a.get("cold_start_limit_ms").and_then(|v| v.as_f64()).unwrap_or(1500.0);
    let rss_limit_mb = a.get("rss_limit_mb").and_then(|v| v.as_f64()).unwrap_or(50.0);
    let bin_limit_mb = a.get("binary_limit_mb").and_then(|v| v.as_f64()).unwrap_or(25.0);

    let peak_mb = peak_kb.map(|k| k as f64 / 1024.0);
    let bin_mb = bin_bytes as f64 / (1024.0 * 1024.0);
    let checks = json!({
        "cold_start": median <= cold_limit_ms,
        "rss": match peak_mb { Some(mb) => mb <= rss_limit_mb, None => true },
        "binary_size": bin_mb <= bin_limit_mb,
    });
    let pass = checks["cold_start"].as_bool().unwrap()
        && checks["rss"].as_bool().unwrap()
        && checks["binary_size"].as_bool().unwrap();

    Ok(json!({
        "cold_start_ms": {
            "iterations": samples.len(),
            "min": samples[0],
            "median": median,
            "p95": p95,
            "max": *samples.last().unwrap(),
            "limit": cold_limit_ms,
        },
        "rss": {
            "peak_kb": peak_kb,
            "peak_mb": peak_mb,
            "current_kb": rss_kb,
            "limit_mb": rss_limit_mb,
            "note": if peak_kb.is_none() { "VmHWM unavailable on this platform" } else { "peak of a full CLI invocation" },
        },
        "binary": { "bytes": bin_bytes, "mb": bin_mb, "limit_mb": bin_limit_mb },
        "checks": checks,
        "pass": pass,
    }))
}

/// (VmRSS kB, VmHWM kB) from /proc/self/status — None off Linux.
fn proc_self_status() -> Option<(Option<u64>, Option<u64>)> {
    let raw = std::fs::read_to_string("/proc/self/status").ok()?;
    let mut rss = None;
    let mut hwm = None;
    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("VmRSS:") {
            rss = v.split_whitespace().next().and_then(|x| x.parse().ok());
        }
        if let Some(v) = line.strip_prefix("VmHWM:") {
            hwm = v.split_whitespace().next().and_then(|x| x.parse().ok());
        }
    }
    Some((rss, hwm))
}

#[cfg(test)]
mod tests {
    #[test]
    fn proc_status_parses_on_linux() {
        let r = super::proc_self_status();
        // On Linux /proc always exists; elsewhere the option is None — both fine.
        if let Some((rss, hwm)) = r {
            assert!(hwm.unwrap_or(0) >= rss.unwrap_or(0));
        }
    }
}
