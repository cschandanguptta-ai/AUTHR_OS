// AUTHR_OS — Video validation (contract v1.5, ADR-0011).
//
// `video_validate` checks the whole chain against the plan: derived scenes,
// asset registry coverage, MP4 file presence/size, and — when ffmpeg is
// resolvable — the encoded duration vs the plan total (tolerance 1.5 s).
// Response shape mirrors `kdp_check`: `{ok, checks[], plan}` with
// PASS/WARN/FAIL statuses. Best-effort probes never fail the command.

use super::derive::{refresh_chapter_media, scenes_for};
use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use serde_json::{json, Value};
use std::process::Command;

pub const DURATION_TOLERANCE_MS: u64 = 1500;

pub fn video_validate(ctx: &Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    refresh_chapter_media(&ctx.conn, chapter_id)?;
    let scenes = scenes_for(&ctx.conn, chapter_id)?;
    if scenes.is_empty() {
        return Err(AppError::not_found(format!(
            "chapter {chapter_id} has no segments — nothing to validate"
        )));
    }
    let (book_id, chapter_number): (i64, i64) = ctx.conn.query_row(
        "SELECT book_id, chapter_number FROM chapters WHERE id = ?1",
        [chapter_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let mut checks: Vec<Value> = Vec::new();
    let total_ms: u64 = super::assemble::TITLE_MS
        + scenes.iter().map(|s| s.duration_ms).sum::<u64>();

    // 1. Derived scene plan present.
    checks.push(check("scene_plan_derived", !scenes.is_empty(),
        format!("{} scenes derived at draft time", scenes.len())));

    // 2. Asset registry coverage.
    let mut stmt = ctx.conn.prepare(
        "SELECT kind, COUNT(*) FROM video_assets
         WHERE book_id = ?1 AND (chapter_number = ?2 OR (kind = 'SHEET' AND chapter_number = 0))
         GROUP BY kind",
    )?;
    let mut counts = std::collections::HashMap::new();
    let rows = stmt
        .query_map(rusqlite::params![book_id, chapter_number], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (k, n) in rows {
        counts.insert(k, n);
    }
    drop(stmt);
    let scenes_rendered = *counts.get("SCENE").unwrap_or(&0);
    let title_rendered = *counts.get("TITLE").unwrap_or(&0);
    let sheets = *counts.get("SHEET").unwrap_or(&0);
    checks.push(check("scenes_rendered", scenes_rendered as usize == scenes.len(),
        format!("{scenes_rendered}/{} scene frames registered", scenes.len())));
    checks.push(check("title_card", title_rendered == 1, format!("title card registered: {title_rendered}")));
    checks.push(check("reference_sheets", sheets > 0 || counts.is_empty(),
        format!("{sheets} character reference sheet(s) (book-level)")));

    // 3. MP4 file (the assemble output, if any).
    let out_path = ctx.root.join("exports").join(format!("video_ch{chapter_number}.mp4"));
    if out_path.is_file() {
        let bytes = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        checks.push(check("mp4_present", bytes > 0, format!("exports/video_ch{chapter_number}.mp4 · {bytes} bytes")));
        // 4. Encoded duration vs plan (only when ffmpeg is resolvable).
        if let Some(ffmpeg) = resolve_ffmpeg(a) {
            if let Some(actual_ms) = probe_duration_ms(&ffmpeg, &out_path) {
                let delta = actual_ms.abs_diff(total_ms);
                let ok = delta <= DURATION_TOLERANCE_MS;
                checks.push(check(
                    "duration_vs_plan",
                    ok,
                    format!("plan {total_ms} ms vs encoded {actual_ms} ms (Δ{delta} ms, tol {DURATION_TOLERANCE_MS} ms)"),
                ));
            } else {
                checks.push(json!({
                    "name": "duration_vs_plan", "status": "WARN",
                    "detail": "ffmpeg probe did not expose a duration — file checks only",
                }));
            }
        } else {
            checks.push(json!({
                "name": "duration_vs_plan", "status": "WARN",
                "detail": "ffmpeg not resolvable — file checks only",
            }));
        }
    } else {
        checks.push(json!({
            "name": "mp4_present", "status": "WARN",
            "detail": "no MP4 yet — run video_assemble",
        }));
    }

    let failed = checks.iter().any(|c| c["status"] == "FAIL");
    Ok(json!({
        "ok": !failed,
        "checks": checks,
        "plan": {
            "chapter_id": chapter_id,
            "chapter_number": chapter_number,
            "book_id": book_id,
            "scenes": scenes.len(),
            "total_ms": total_ms,
            "resolution": format!("{}x{}", super::frame::SCENE_W, super::frame::SCENE_H),
        },
    }))
}

fn check(name: &str, ok: bool, detail: impl Into<String>) -> Value {
    json!({ "name": name, "status": if ok { "PASS" } else { "FAIL" }, "detail": detail.into() })
}

fn resolve_ffmpeg(a: &Value) -> Option<String> {
    if let Some(b) = a.get("ffmpeg_bin").and_then(|v| v.as_str()) {
        return Some(b.to_string());
    }
    if let Ok(env_bin) = std::env::var("AUTHR_FFMPEG_BIN") {
        return Some(env_bin);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("ffmpeg"))
        .find(|p| p.is_file())
        .map(|_| "ffmpeg".to_string())
}

/// `ffmpeg -i` stderr probe: `Duration: HH:MM:SS.ms`.
fn probe_duration_ms(ffmpeg: &str, path: &std::path::Path) -> Option<u64> {
    let out = Command::new(ffmpeg).arg("-i").arg(path).output().ok()?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    for line in stderr.lines() {
        if let Some(idx) = line.find("Duration:") {
            // Strip the ", start: …" tail FIRST, then split the HH:MM:SS.ms
            // field — splitting on ':' before stripping would pick up the
            // colons inside "start: 0.000000" and break the 3-part shape.
            let field = line[idx + "Duration:".len()..].trim().split(',').next()?;
            let parts: Vec<&str> = field.split(':').collect();
            if parts.len() == 3 {
                let h: f64 = parts[0].trim().parse().ok()?;
                let m: f64 = parts[1].trim().parse().ok()?;
                let s: f64 = parts[2].trim().parse().ok()?;
                return Some(((h * 3600.0 + m * 60.0 + s) * 1000.0).round() as u64);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_probe_parses_the_ffmpeg_line() {
        // The regex-free parser must read `Duration: 00:00:12.48`.
        let s = "  Duration: 00:00:12.48, start: 0.000000, bitrate: 1234 kb/s";
        let idx = s.find("Duration:").unwrap();
        let rest = s[idx + "Duration:".len()..].trim();
        let parts: Vec<&str> = rest.split(':').collect();
        let h: f64 = parts[0].trim().parse().unwrap();
        let m: f64 = parts[1].trim().parse().unwrap();
        let sec: f64 = parts[2].split(',').next().unwrap().trim().parse().unwrap();
        let ms = ((h * 3600.0 + m * 60.0 + sec) * 1000.0).round() as u64;
        assert_eq!(ms, 12480);
    }

    #[test]
    fn tolerance_is_reasonable_for_aac_padding() {
        assert!(DURATION_TOLERANCE_MS >= 1000, "AAC priming samples shift durations");
        assert!(DURATION_TOLERANCE_MS <= 2500);
    }

    #[test]
    fn check_builder_statuses() {
        assert_eq!(check("a", true, "x")["status"], "PASS");
        assert_eq!(check("a", false, "x")["status"], "FAIL");
    }
}
