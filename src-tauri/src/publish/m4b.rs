// AUTHR_OS — ACX audiobook M4B exporter (Phase 6, Sprint 14).
//
// Pipeline: per-chapter segment WAVs (16-bit PCM mono from the audio cache)
// → ffmpeg concat demuxer → AAC-LC mono 44.1 kHz → ISO-MP4 (`-f ipod`) with
// ffmetadata chapter atoms (one per book chapter). Requires `ffmpeg` on PATH
// (`AUTHR_FFMPEG_BIN` overrides); missing binary ⇒ `BUNDLE_ERROR` with
// placement guidance. Chapters containing unsynthesized segments are rejected
// so the audiobook can never silently drop a turn.

use crate::error::AppError;
use crate::publish::{exports_path, load_book, slugify, BookData};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

/// #51 publish_m4b — audiobook export with chapter atoms.
pub fn publish_m4b(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let (book_id, chapter_id): (i64, Option<i64>) = if let Some(b) = a.get("book_id").and_then(|v| v.as_i64()) {
        (b, None)
    } else {
        let c = crate::ipc::args::req_i64(a, "chapter_id")?;
        (0, Some(c))
    };

    // Resolve scope + title.
    let data: BookData = if chapter_id.is_none() {
        load_book(&ctx.conn, book_id)?
    } else {
        let ch_row = crate::storage::chapters::get(&ctx.conn, chapter_id.unwrap())?;
        let bid = ch_row["book_id"].as_i64().unwrap_or(0);
        let mut book = load_book(&ctx.conn, bid)?;
        book.chapters.retain(|c| c.row["id"].as_i64() == chapter_id);
        if book.chapters.is_empty() {
            return Err(AppError::bundle("chapter has no segments to publish"));
        }
        book
    };
    let title = data.book["title"].as_str().unwrap_or("untitled");
    let slug = slugify(title);
    let out_rel = format!("{slug}.m4b");
    let out_path = exports_path(&ctx.root, &out_rel)?;
    let ffmpeg = ffmpeg_bin(a)?;

    // Every segment must be synthesized; collect absolute chunk paths.
    let mut wav_list: Vec<Vec<PathBuf>> = Vec::new();
    let mut durations_ms: Vec<u64> = Vec::new();
    for ch in &data.chapters {
        let mut cur = Vec::new();
        let mut dur = 0u64;
        for seg in &ch.segments {
            let rel = seg["audio_cache_path"].as_str().ok_or_else(|| {
                AppError::bundle(format!(
                    "chapter {} has unsynthesized segments — run audio_synthesize first",
                    ch.row["chapter_number"].as_i64().unwrap_or(0)
                ))
            })?;
            let abs = crate::security::sandbox::resolve_in_root(&ctx.root, std::path::Path::new(rel))?;
            dur += wav_duration_ms(&abs)?;
            cur.push(abs);
        }
        wav_list.push(cur);
        durations_ms.push(dur);
    }
    let total_ms: u64 = durations_ms.iter().sum();

    // Stage concat list + ffmetadata chapter file inside exports/.tmp-m4b/.
    let tmp_dir = ctx.root.join("exports").join(".tmp-m4b");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| AppError::bundle(format!("tmp dir: {e}")))?;
    let mut list = String::new();
    for group in &wav_list {
        for wav in group {
            list.push_str(&format!("file '{}'\n", abs_escape(wav)));
        }
    }
    let list_path = tmp_dir.join("list.txt");
    std::fs::write(&list_path, &list).map_err(|e| AppError::bundle(format!("write list: {e}")))?;

    let mut meta = String::from(";FFMETADATA1\n");
    meta.push_str(&format!("title={}\n", title.replace(';', ",")));
    meta.push_str(&format!(
        "artist={}\n",
        data.book["author"].as_str().unwrap_or("Anonymous").replace(';', ",")
    ));
    let mut t = 0u64;
    for (i, ch) in data.chapters.iter().enumerate() {
        let num = ch.row["chapter_number"].as_i64().unwrap_or(i as i64 + 1);
        let ch_title = ch.row["title"].as_str().unwrap_or("Chapter");
        meta.push_str("[CHAPTER]\nTIMEBASE=1/1000\n");
        meta.push_str(&format!("START={t}\nEND={}\n", t + durations_ms[i]));
        meta.push_str(&format!(
            "title=Ch{:02} {}\n",
            num,
            ch_title.replace(';', ",")
        ));
        t += durations_ms[i];
    }
    let meta_path = tmp_dir.join("chapters.txt");
    std::fs::write(&meta_path, &meta).map_err(|e| AppError::bundle(format!("write meta: {e}")))?;

    // One ffmpeg pass: concat → AAC → ipod muxer with chapter atoms.
    let out = Command::new(&ffmpeg)
        .arg("-y")
        .arg("-f").arg("concat")
        .arg("-safe").arg("0")
        .arg("-i").arg(&list_path)
        .arg("-i").arg(&meta_path)
        .arg("-map_metadata").arg("1")
        .arg("-c:a").arg("aac")
        .arg("-b:a").arg("64k")
        .arg("-ar").arg("44100")
        .arg("-ac").arg("1")
        .arg("-f").arg("ipod")
        .arg(&out_path)
        .output()
        .map_err(|e| AppError::bundle(format!("spawn ffmpeg: {e}")))?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::bundle(format!(
            "ffmpeg m4b encode failed: {}",
            stderr.lines().last().unwrap_or("unknown error")
        )));
    }
    let bytes = std::fs::metadata(&out_path)
        .map_err(|e| AppError::bundle(format!("stat m4b: {e}")))?
        .len();
    ctx.after_commit()?;

    Ok(json!({
        "out": format!("exports/{out_rel}"),
        "bytes": bytes,
        "chapters": data.chapters.len(),
        "duration_ms": total_ms,
        "codec": "aac-lc 64k mono 44.1kHz",
        "ffmpeg": ffmpeg,
    }))
}

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
         the M4B exporter shells out to it for AAC encoding",
    )
}

/// Null-byte/apostrophe-safe path quoting for the concat demuxer list.
fn abs_escape(p: &PathBuf) -> String {
    p.to_string_lossy().replace('\'', "'\\''")
}

/// Header-only WAV duration (hound reads the fmt/data chunk sizes lazily).
fn wav_duration_ms(path: &std::path::Path) -> Result<u64, AppError> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| AppError::bundle(format!("wav header {}: {e}", path.display())))?;
    let spec = reader.spec();
    let frames = reader.duration() as u64;
    Ok(frames * 1000 / u64::from(spec.sample_rate.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_list_escapes_quotes() {
        assert_eq!(abs_escape(&PathBuf::from("/a'b/c.wav")), "/a'\\''b/c.wav");
    }
}
