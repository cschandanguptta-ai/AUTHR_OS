// AUTHR_OS — synchronized caption tracks (Phase 6, Sprint 14).
//
// Timings come from the segment audio durations (`audio_duration_ms` when
// synthesized, otherwise the same text-length estimator the scheduler uses),
// accumulated in publish order — identical to the EPUB SMIL overlays, so
// captions, overlays and the M4B chapter atoms all agree.
//
//   .lrc — line-synced lyrics:  [mm:ss.xx] text
//   .vtt — WebVTT cues:         HH:MM:SS.mmm --> HH:MM:SS.mmm

use crate::error::AppError;
use crate::publish::{cumulative_starts, exports_path, slugify, turns};
use serde_json::{json, Value};

/// #50 publish_captions — .lrc/.vtt caption tracks for a chapter.
pub fn publish_captions(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let chapter_id = crate::ipc::args::req_i64(a, "chapter_id")?;
    let formats: Vec<String> = match a.get("formats") {
        Some(Value::Array(arr)) if !arr.is_empty() => arr
            .iter()
            .map(|v| {
                let s = v.as_str().unwrap_or("");
                match s {
                    "lrc" | "vtt" => Ok(s.to_string()),
                    _ => Err(AppError::bundle(format!(
                        "unsupported caption format `{s}` (lrc|vtt)"
                    ))),
                }
            })
            .collect::<Result<_, _>>()?,
        _ => vec!["lrc".into(), "vtt".into()],
    };

    let chapter = crate::storage::chapters::get(&ctx.conn, chapter_id)?;
    let segments = crate::storage::segments::list(&ctx.conn, chapter_id)?["segments"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if segments.is_empty() {
        return Err(AppError::bundle(format!(
            "chapter {chapter_id} has no segments to caption"
        )));
    }
    let book_id = chapter["book_id"].as_i64().unwrap_or(0);
    let book = crate::storage::books::get(&ctx.conn, book_id)?;
    let cast = crate::publish::cast_names(&ctx.conn);
    let ch = crate::publish::ChapterData { row: chapter, segments };
    let ts = turns(&ch, &cast);
    let starts = cumulative_starts(&ch.segments);
    let total = starts.last().copied().unwrap_or(0)
        + ch.segments.last().map(crate::publish::segment_duration_ms).unwrap_or(0);

    let ch_title = ch.row["title"].as_str().unwrap_or("Chapter");
    let base = format!(
        "{}-ch{:02}",
        slugify(book["title"].as_str().unwrap_or("untitled")),
        ch.row["chapter_number"].as_i64().unwrap_or(1)
    );

    let mut out = json!({ "chapter_id": chapter_id, "cues": ts.len(), "duration_ms": total });
    for fmt in &formats {
        let (ext, body) = if fmt == "lrc" {
            (format!("{base}.lrc"), render_lrc(&ts, &starts, ch_title))
        } else {
            (format!("{base}.vtt"), render_vtt(&ts, &starts))
        };
        let path = exports_path(&ctx.root, &ext)?;
        std::fs::write(&path, body).map_err(|e| AppError::bundle(format!("write {ext}: {e}")))?;
        out[fmt.as_str()] = json!(format!("exports/{ext}"));
    }
    ctx.after_commit()?;
    Ok(out)
}

fn render_lrc(
    ts: &[crate::publish::Turn],
    starts: &[u64],
    chapter_title: &str,
) -> String {
    let mut out = String::from("[re:AUTHR_OS]\n");
    out.push_str(&format!("[ti:{}]\n", chapter_title.replace('[', "(").replace(']', ")")));
    out.push_str("[offset:0]\n\n");
    for (i, t) in ts.iter().enumerate() {
        let ms = starts[i];
        let mm = ms / 60_000;
        let ss = (ms % 60_000) / 1000;
        let cs = (ms % 1000) / 10;
        out.push_str(&format!("[{mm:02}:{ss:02}.{cs:02}]{}\n", t.text.replace('\n', " ")));
    }
    out
}

fn render_vtt(ts: &[crate::publish::Turn], starts: &[u64]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, t) in ts.iter().enumerate() {
        let dur = crate::publish::segment_duration_ms(t.seg);
        let begin = starts[i];
        let end = begin + dur;
        out.push_str(&format!("{}{}\n", if i > 0 { "\n" } else { "" }, i + 1));
        out.push_str(&format!("{} --> {}\n", vtt_clock(begin), vtt_clock(end)));
        out.push_str(&format!("{}\n", t.text.replace('\n', " ")));
    }
    out
}

fn vtt_clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}.{:03}", s / 3600, (s % 3600) / 60, s % 60, ms % 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(text: &str, dur: u64) -> Value {
        serde_json::json!({"text_content": text, "audio_duration_ms": dur})
    }

    #[test]
    fn vtt_cues_are_wellformed_and_monotonic() {
        let segs = vec![mk("first line", 1_500), mk("second line", 2_000)];
        let ch = crate::publish::ChapterData { row: json!({}), segments: segs };
        let ts = turns(&ch, &Default::default());
        let starts = cumulative_starts(&ch.segments);
        let vtt = render_vtt(&ts, &starts);
        assert!(vtt.starts_with("WEBVTT\n"));
        assert!(vtt.contains("00:00:00.000 --> 00:00:01.500"));
        assert!(vtt.contains("00:00:01.500 --> 00:00:03.500"));
        assert!(vtt.contains("second line"));
    }

    #[test]
    fn lrc_lines_match_expectation() {
        // LRC stamps are line STARTS: line 2 begins after line 1's 61.23 s.
        let segs = vec![mk("hello world", 61_230), mk("next line", 1_000)];
        let ch = crate::publish::ChapterData { row: json!({}), segments: segs };
        let ts = turns(&ch, &Default::default());
        let starts = cumulative_starts(&ch.segments);
        let lrc = render_lrc(&ts, &starts, "T");
        assert!(lrc.contains("[00:00.00]hello world"), "{lrc}");
        assert!(lrc.contains("[01:01.23]next line"), "{lrc}");
    }
}
