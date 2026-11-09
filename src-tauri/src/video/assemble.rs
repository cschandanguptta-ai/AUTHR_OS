// AUTHR_OS — Video assembly: MP4 export via the ffmpeg seam (ADR-0011).
//
// The visual contract mirrors the novelcast storyboard painter (its ADR-0008):
// 1280×720 @ 30 fps, Ken Burns drift per scene (alternating zoom-in/out for
// variety, anchored on the persisted focal point), hard cuts between scenes,
// subtitle soft-mux. Assembly is one ffmpeg pass over the stills:
//
//   per-scene: -loop 1 -t <sec> -i scene.bmp
//   filter:    scale → crop → zoompan(focal, alternating) → format yuv420p
//   concat     n scenes → [vout]
//   audio:     narration WAV/M4B (arg or auto-detected per-scene pipeline
//              audio from `chapter_segments.audio_cache_path`) — or a silent
//              track sized to the plan; -shortest when narration is shorter
//   subs:      optional .srt soft-muxed as mov_text
//
// `ffmpeg_bin` resolution is byte-for-byte the M4B pattern (args → env →
// PATH, missing ⇒ fail-closed BUNDLE_ERROR with install guidance).
// The filter graph itself is a pure function — unit-tested.

use super::derive::scenes_for;
use super::frame::{focal_point, SCENE_H, SCENE_W};
use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use serde_json::{json, Value};
use std::process::Command;

pub const DEFAULT_FPS: u32 = 30;
pub const TITLE_MS: u64 = 2000;

#[derive(Debug, Clone)]
pub struct Clip {
    pub path: String,
    pub duration_ms: u64,
    pub focal: String,
}

/// Pure Ken Burns filter for one clip. `zoom_in` alternates per scene index
/// (variety without randomness). Anchors derived from the focal point.
pub fn zoompan_filter(clip_idx: usize, frames: u32, fps: u32, focal: &str) -> String {
    let (fx, fy) = focal_point(focal);
    let z = if clip_idx.is_multiple_of(2) {
        "min(zoom+0.0006,1.12)".to_string() // slow push-in
    } else {
        "if(lte(zoom,1.0),1.12,max(1.001,zoom-0.0006))".to_string() // pull-out
    };
    // zoompan anchoring: keep the normalized focal point centered while zooming.
    let x = format!("iw*{fx}-(iw/zoom)*{fx}");
    let y = format!("ih*{fy}-(ih/zoom)*{fy}");
    format!(
        "[{clip_idx}:v]scale={SCENE_W}:{SCENE_H}:force_original_aspect_ratio=increase,\
         crop={SCENE_W}:{SCENE_H},zoompan=z='{z}':x='{x}':y='{y}':d={frames}:s={SCENE_W}x{SCENE_H}:fps={fps},\
         format=yuv420p,setsar=1[v{clip_idx}]"
    )
}

/// Pure builder: returns (filter_complex, per-clip -t seconds). `clips` must
/// be non-empty. Deterministic: same input → same string.
pub fn build_filter_complex(clips: &[Clip], fps: u32) -> Result<(String, Vec<f64>), AppError> {
    if clips.is_empty() {
        return Err(AppError::audio("video assembly needs at least one clip"));
    }
    let mut parts = Vec::new();
    let mut secs = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        let sec = (c.duration_ms as f64 / 1000.0).max(0.5);
        let frames = ((sec * fps as f64).round() as u32).max(2);
        parts.push(zoompan_filter(i, frames, fps, &c.focal));
        secs.push(sec);
    }
    let labels: Vec<String> = (0..clips.len()).map(|i| format!("[v{i}]")).collect();
    parts.push(format!(
        "{}concat=n={}:v=1:a=0[vout]",
        labels.join(""),
        clips.len()
    ));
    Ok((parts.join(";"), secs))
}

/// M4B-identical ffmpeg resolution (args → env → PATH; fail-closed).
fn ffmpeg_bin(a: &Value) -> Result<String, AppError> {
    if let Some(b) = a.get("ffmpeg_bin").and_then(|v| v.as_str()) {
        return Ok(b.to_string());
    }
    if let Ok(env_bin) = std::env::var("AUTHR_FFMPEG_BIN") {
        return Ok(env_bin);
    }
    let path = std::env::var_os("PATH").ok_or_else(ffmpeg_missing)?;
    if std::env::split_paths(&path)
        .map(|dir| dir.join("ffmpeg"))
        .find(|p| p.is_file())
        .is_some()
    {
        return Ok("ffmpeg".to_string());
    }
    Err(ffmpeg_missing())
}

fn ffmpeg_missing() -> AppError {
    AppError::bundle(
        "ffmpeg not found — install ffmpeg (apt/brew/winget) or set AUTHR_FFMPEG_BIN; \
         the video assembler shells out to it for MP4 encoding",
    )
}

fn abs_escape(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\'', "'\\''")
}

struct ClipPaths {
    title: String,
    /// segment_id → registry-relative path (media/video/b{n}/ch{n}/scene_*).
    scenes: std::collections::HashMap<String, String>,
}

/// Read the clip table from `video_assets` (kind TITLE + kind SCENE keyed by
/// segment_id). Returns `None` when ANY expected clip is missing or its file
/// is gone — the caller runs the SCRIPTED `missing_only` ensure-render.
fn registry_clip_paths(
    conn: &rusqlite::Connection,
    root: &std::path::Path,
    book_id: i64,
    chapter_number: i64,
    scenes: &[super::derive::SceneRow],
) -> Result<Option<ClipPaths>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT kind, ref_key, path FROM video_assets
         WHERE book_id = ?1 AND chapter_number IN (0, ?2) AND kind IN ('TITLE', 'SCENE')",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![book_id, chapter_number], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut title: Option<String> = None;
    let mut map = std::collections::HashMap::new();
    for (kind, ref_key, path) in rows {
        if !root.join(&path).is_file() {
            continue; // registry row points at a vanished file → gap
        }
        match kind.as_str() {
            "TITLE" => title = Some(path),
            "SCENE" => {
                map.insert(ref_key, path);
            }
            _ => {}
        }
    }
    let Some(title) = title else {
        return Ok(None);
    };
    if scenes
        .iter()
        .any(|s| !map.contains_key(&s.segment_id))
    {
        return Ok(None);
    }
    Ok(Some(ClipPaths { title, scenes: map }))
}

/// §4.16 `video_assemble` — ensure every clip exists (SCRIPTED auto-render,
/// `missing_only` so live endpoint art is never clobbered), then one ffmpeg
/// pass to `exports/video_ch{n}.mp4`. Clips are registry-driven (v1.6):
/// whatever the painter produced — SCRIPTED BMPs or R2I PNG/JPEGs — is what
/// gets encoded (novelcast storyboard-painter parity).
pub fn video_assemble(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    let fps = args::get_i64(a, "fps").unwrap_or(DEFAULT_FPS as i64) as u32;
    if !(12..=60).contains(&fps) {
        return Err(AppError::audio(format!("fps {fps} outside 12–60")));
    }
    super::derive::refresh_chapter_media(&ctx.conn, chapter_id)?;
    let scenes = scenes_for(&ctx.conn, chapter_id)?;
    if scenes.is_empty() {
        return Err(AppError::not_found(format!(
            "chapter {chapter_id} has no segments — nothing to assemble"
        )));
    }
    let (book_id, chapter_number): (i64, i64) = ctx.conn.query_row(
        "SELECT book_id, chapter_number FROM chapters WHERE id = ?1",
        [chapter_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    // Registry-driven clip table: title + one clip per scene in plan order.
    // A gap (no registry row, or the backing file vanished) triggers the
    // idempotent SCRIPTED ensure-render with `missing_only: true` — which
    // paints ONLY the gaps, leaving live R2I_ENDPOINT art untouched.
    let clip_paths = registry_clip_paths(&ctx.conn, &ctx.root, book_id, chapter_number, &scenes)?;
    if clip_paths.is_none() {
        super::render::video_render(
            ctx,
            &json!({ "chapter_id": chapter_id, "engine": "SCRIPTED", "missing_only": true }),
        )?;
    }
    let clip_paths = registry_clip_paths(&ctx.conn, &ctx.root, book_id, chapter_number, &scenes)?
        .ok_or_else(|| AppError::bundle(
            "video assembly: registry gaps remain after the ensure-render — media tree is not writable?",
        ))?;

    // Clips: title card first, then scenes (plan order, persisted focals).
    let mut clips = vec![Clip {
        path: ctx.root.join(&clip_paths.title).to_string_lossy().into_owned(),
        duration_ms: TITLE_MS,
        focal: "center".into(),
    }];
    for s in &scenes {
        let path = clip_paths
            .scenes
            .get(&s.segment_id)
            .ok_or_else(|| {
                AppError::bundle(format!(
                    "video assembly: no registered frame for scene {} ({})",
                    s.scene_index, s.segment_id
                ))
            })?
            .clone();
        clips.push(Clip {
            path: ctx.root.join(path).to_string_lossy().into_owned(),
            duration_ms: s.duration_ms,
            focal: s.focal.clone(),
        });
    }
    let total_ms: u64 = TITLE_MS + scenes.iter().map(|s| s.duration_ms).sum::<u64>();
    let (filter, _secs) = build_filter_complex(&clips, fps)?;

    // Narration: explicit arg (bundle-relative) wins; else auto-concat the
    // per-scene pipeline audio when every scene carries one; else silent.
    let exports = ctx.root.join("exports");
    std::fs::create_dir_all(&exports).map_err(|e| AppError::bundle(format!("mkdir exports: {e}")))?;
    let out_path = exports.join(format!("video_ch{chapter_number}.mp4"));
    let ffmpeg = ffmpeg_bin(a)?;

    let tmp_dir = ctx.root.join("exports").join(format!(".video_tmp_ch{chapter_number}"));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).map_err(|e| AppError::bundle(format!("mkdir tmp: {e}")))?;

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y");
    // Single-frame still inputs: zoompan(d=frames, fps) generates the full
    // per-clip timeline itself. Feeding `-loop 1 -t sec` stills into zoompan
    // would multiply every duplicated input frame by d again (the classic
    // zoompan explosion — measured: a 9 s plan rendered as a 12-minute MP4).
    for c in &clips {
        cmd.arg("-i").arg(&c.path);
    }

    // Audio input index = clips.len() (after all still inputs).
    let mut audio_idx: Option<usize> = None;
    let mut used_narration = String::new();
    if let Some(n) = args::get_str(a, "narration") {
        let p = ctx.root.join(n.as_str());
        if !p.is_file() {
            return Err(AppError::not_found(format!("narration `{n}` not found in bundle")));
        }
        cmd.arg("-i").arg(&p);
        audio_idx = Some(clips.len());
        used_narration = n;
    } else {
        let cache_paths: Vec<&str> = scenes
            .iter()
            .map(|s| s.audio_cache_path.as_str())
            .filter(|p| !p.is_empty())
            .collect();
        if cache_paths.len() == scenes.len() {
            let list_path = tmp_dir.join("narration.txt");
            let mut list = String::new();
            for p in &cache_paths {
                list.push_str(&format!("file '{}'\n", abs_escape(&ctx.root.join(p))));
            }
            std::fs::write(&list_path, &list).map_err(|e| AppError::bundle(format!("write list: {e}")))?;
            cmd.arg("-f").arg("concat").arg("-safe").arg("0").arg("-i").arg(&list_path);
            audio_idx = Some(clips.len());
            used_narration = "auto:pipeline-audio".into();
        }
    }

    // Optional subtitles (soft-mux).
    let mut srt_idx: Option<usize> = None;
    if let Some(srt) = args::get_str(a, "srt") {
        let p = ctx.root.join(srt.as_str());
        if !p.is_file() {
            return Err(AppError::not_found(format!("srt `{srt}` not found in bundle")));
        }
        cmd.arg("-i").arg(&p);
        srt_idx = Some(clips.len() + if audio_idx.is_some() { 1 } else { 0 });
    }

    // Silent track when there is no narration at all.
    if audio_idx.is_none() {
        cmd.arg("-f").arg("lavfi")
            .arg("-t").arg(format!("{:.3}", total_ms as f64 / 1000.0))
            .arg("-i").arg("anullsrc=r=44100:cl=mono");
    }

    cmd.arg("-filter_complex").arg(&filter)
        .arg("-map").arg("[vout]");
    let audio_in = audio_idx.unwrap_or(clips.len()); // silent track index
    cmd.arg("-map").arg(format!("{audio_in}:a"));
    if let Some(si) = srt_idx {
        cmd.arg("-map").arg(format!("{si}:s")).arg("-c:s").arg("mov_text");
    }
    cmd.arg("-c:v").arg("libx264")
        .arg("-preset").arg("veryfast")
        .arg("-crf").arg("22")
        .arg("-pix_fmt").arg("yuv420p")
        .arg("-r").arg(fps.to_string())
        .arg("-c:a").arg("aac").arg("-b:a").arg("96k").arg("-ar").arg("44100").arg("-ac").arg("1")
        .arg("-movflags").arg("+faststart");
    // Deliberately no -shortest: the title card's 2 s tail must survive even
    // when the narration stream is shorter; audio drift beyond tolerance is
    // caught by video_validate instead of being silently clipped.
    cmd.arg(&out_path);

    let out = cmd.output().map_err(|e| AppError::bundle(format!("spawn ffmpeg: {e}")))?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::bundle(format!(
            "ffmpeg video encode failed: {}",
            stderr.lines().last().unwrap_or("unknown error")
        )));
    }
    let bytes = std::fs::metadata(&out_path)
        .map_err(|e| AppError::bundle(format!("stat mp4: {e}")))?
        .len();
    ctx.after_commit()?;
    let rel = out_path
        .strip_prefix(&ctx.root)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| out_path.to_string_lossy().into_owned());

    Ok(json!({
        "out": rel,
        "bytes": bytes,
        "scenes": scenes.len(),
        "clips": clips.len(),
        "duration_ms": total_ms,
        "fps": fps,
        "resolution": format!("{SCENE_W}x{SCENE_H}"),
        "narration": used_narration,
        "srt": args::get_str(a, "srt").unwrap_or_default(),
        "codec": "h264 yuv420p + aac 96k mono 44.1kHz",
        "ffmpeg": ffmpeg,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(i: usize, ms: u64, focal: &str) -> Clip {
        Clip { path: format!("/tmp/scene_{i}.bmp"), duration_ms: ms, focal: focal.into() }
    }

    #[test]
    fn graph_concatenates_all_scenes_and_is_deterministic() {
        let clips = vec![clip(0, 2000, "center"), clip(1, 1500, "left")];
        let (f1, secs) = build_filter_complex(&clips, 30).unwrap();
        let (f2, _) = build_filter_complex(&clips, 30).unwrap();
        assert_eq!(f1, f2);
        assert_eq!(secs, vec![2.0, 1.5]);
        assert!(f1.contains("concat=n=2:v=1:a=0[vout]"));
        assert_eq!(f1.matches("[0:v]").count(), 1);
        assert!(f1.contains("[v0][v1]concat"));
    }

    #[test]
    fn kenburns_alternates_and_anchors_on_focal() {
        let zin = zoompan_filter(0, 60, 30, "center");
        let zout = zoompan_filter(1, 45, 30, "right");
        assert!(zin.contains("min(zoom+0.0006,1.12)"), "even scenes push in");
        assert!(zout.contains("max(1.001,zoom-0.0006)"), "odd scenes pull out");
        assert!(zin.contains("x='iw*0.5-(iw/zoom)*0.5'"));
        assert!(zout.contains("x='iw*0.66-(iw/zoom)*0.66'"), "focal anchor rides into the graph");
        assert!(!zin.contains("s=1280x1280"));
        assert!(zin.contains("s=1280x720"));
    }

    #[test]
    fn empty_clip_list_fails_closed() {
        assert!(build_filter_complex(&[], 30).is_err());
    }

    #[test]
    fn tiny_durations_get_minimum_half_second_clip() {
        let clips = vec![clip(0, 1, "center")];
        let (f, secs) = build_filter_complex(&clips, 30).unwrap();
        // Clip floor is 0.5 s → 15 frames at 30 fps (the max(2) guard only
        // bites at very low fps values).
        assert_eq!(secs[0], 0.5);
        assert!(f.contains("d=15"));
    }
}
