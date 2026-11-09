// AUTHR_OS — publishing pipelines (Phase 6, build plan Sprint 14).
//
// Layout:
//   epub.rs     — EPUB 3 compiler (Dublin Core, nav, SMIL media overlays)
//   docx.rs     — standard manuscript DOCX exporter (WordprocessingML)
//   typst.rs    — Typst print-ready source generator (+ optional PDF compile)
//   captions.rs — synchronized .lrc / .vtt caption tracks
//   m4b.rs      — ACX audiobook M4B exporter (ffmpeg AAC + chapter atoms)
//   release.rs  — updater manifest signing/verification (ed25519)
//   bench.rs    — cold-start / RSS / binary-size benchmark
//
// All exporters write inside `<bundle>/exports/` through the same sandboxed
// slug machinery and share the book/chapter/segment loader below.

pub mod bench;
pub mod captions;
pub mod docx;
pub mod epub;
pub mod m4b;
pub mod release;
pub mod typst;

use crate::error::AppError;
use rusqlite::Connection;
use serde_json::Value;

/// A chapter with its ordered segments, ready for every exporter.
pub struct ChapterData {
    pub row: Value,
    pub segments: Vec<Value>,
}

pub struct BookData {
    pub book: Value,
    pub chapters: Vec<ChapterData>,
}

/// Load a book and all of its chapters+segments in publish order.
/// Fails `ENTITY_NOT_FOUND` for an unknown book, `BUNDLE_ERROR` when the
/// book has no chapters (nothing to publish).
pub fn load_book(conn: &Connection, book_id: i64) -> Result<BookData, AppError> {
    let book = crate::storage::books::get(conn, book_id)?;
    let chapter_rows: Vec<Value> = crate::storage::chapters::list(conn, Some(book_id))?["chapters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if chapter_rows.is_empty() {
        return Err(AppError::bundle(format!(
            "book {book_id} has no chapters to publish"
        )));
    }
    let mut chapters = Vec::with_capacity(chapter_rows.len());
    for ch in chapter_rows {
        let cid = ch["id"].as_i64().unwrap_or(0);
        let segments = crate::storage::segments::list(conn, cid)?["segments"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        chapters.push(ChapterData { row: ch, segments });
    }
    Ok(BookData { book, chapters })
}

/// Voice-cast display names keyed by character_key (speaker_id fallback).
pub fn cast_names(conn: &Connection) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    if let Ok(rows) = crate::storage::characters::list(conn) {
        if let Some(arr) = rows["characters"].as_array() {
            for c in arr {
                if let (Some(k), Some(n)) = (
                    c["character_key"].as_str().map(str::to_string),
                    c["display_name"].as_str().map(str::to_string),
                ) {
                    out.insert(k, n);
                }
            }
        }
    }
    out
}

/// Estimated spoken duration (ms) — delegates the `chars×55+400` formula to
/// the canonical `audio::scheduler::estimate_duration_ms` (tech-debt T-3), so
/// publish timing agrees with the scheduler for unsynthesized segments.
pub fn segment_duration_ms(seg: &Value) -> u64 {
    seg["audio_duration_ms"]
        .as_u64()
        .filter(|d| *d > 0)
        .unwrap_or_else(|| {
            crate::audio::scheduler::estimate_duration_ms(seg["text_content"].as_str().unwrap_or(""))
        })
}

/// Cumulative start offsets (ms) per segment, in publish order.
pub fn cumulative_starts(segments: &[Value]) -> Vec<u64> {
    let mut starts = Vec::with_capacity(segments.len());
    let mut t = 0u64;
    for s in segments {
        starts.push(t);
        t += segment_duration_ms(s);
    }
    starts
}

/// Filesystem-safe slug: lowercase alnum runs joined by `-`. Hostile titles
/// (`../../evil`, `CON:…`, unicode noise) collapse to safe export names.
///
/// Tech-debt T-11: titles sharing a 64-char prefix used to overwrite each
/// other's export file. When truncation actually occurs, a short SHA-256
/// prefix of the FULL slug is appended — so two colliding titles diverge,
/// while republishing the same title stays idempotent (same input → same
/// hash → same file name → overwrite as before).
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = true;
    for c in title.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("untitled");
    }
    if slug.len() > 64 {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(slug.as_bytes());
        let digest = h.finalize();
        let suffix: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
        let mut keep = slug[..55].to_string();
        while keep.ends_with('-') {
            keep.pop();
        }
        slug = format!("{keep}-{suffix}");
    }
    slug
}

/// XML (&, <, >, ", ') and XHTML text escaping for every generated document.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// `<bundle>/exports/` path for a slug-derived file name (bare name only —
/// slugify already guarantees it, this is the defensive re-check).
pub fn exports_path(root: &std::path::Path, name: &str) -> Result<std::path::PathBuf, AppError> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains(':') {
        return Err(AppError::traversal(format!(
            "publish output name must be a bare file name: `{name}`"
        )));
    }
    let dir = root.join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| AppError::bundle(format!("create exports: {e}")))?;
    Ok(dir.join(name))
}

/// Publish-time summary row for one segment (used by all exporters).
pub struct Turn<'a> {
    pub seq: usize,
    pub seg: &'a Value,
    pub speaker: String,
    pub text: &'a str,
    pub kind: &'a str,
}

pub fn turns<'a>(
    ch: &'a ChapterData,
    cast: &std::collections::HashMap<String, String>,
) -> Vec<Turn<'a>> {
    ch.segments
        .iter()
        .enumerate()
        .map(|(i, seg)| Turn {
            seq: i,
            seg,
            speaker: cast
                .get(seg["speaker_id"].as_str().unwrap_or(""))
                .cloned()
                .unwrap_or_else(|| seg["speaker_id"].as_str().unwrap_or("NARRATOR").to_string()),
            text: seg["text_content"].as_str().unwrap_or(""),
            kind: seg["segment_type"].as_str().unwrap_or("NARRATION"),
        })
        .collect()
}

/// ISO-8601 UTC timestamp with `Z` (EPUB dcterms:modified, DOCX core props).
pub fn iso_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slugify_is_sandbox_safe() {
        assert_eq!(slugify("The Lighthouse Keeper"), "the-lighthouse-keeper");
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("CON: audit ??"), "con-audit");
        assert_eq!(slugify("   ---   "), "untitled");
        let long = slugify(&"x".repeat(300));
        assert!(long.len() <= 64);
    }

    #[test]
    fn slugify_disambiguates_long_prefix_collisions() {
        // T-11: both titles share the first 64+ slug chars; the differing
        // word lies beyond the truncation point. Old behavior: one export
        // clobbered the other. New: distinct names, same title → same name.
        let base = "the lighthouse keeper of port merrow chapter and the long voyage ";
        let a = slugify(&format!("{base}home"));
        let b = slugify(&format!("{base}away"));
        assert_ne!(a, b, "colliding prefixes must disambiguate: {a} vs {b}");
        assert_eq!(a, slugify(&format!("{base}home")), "republish must be idempotent");
        assert!(a.len() <= 64 && b.len() <= 64);
    }

    #[test]
    fn xml_escape_covers_all_metachars() {
        assert_eq!(xml_escape(r#"a<&>"'"#), "a&lt;&amp;&gt;&quot;&#39;");
        assert_eq!(xml_escape("plain text"), "plain text");
    }

    #[test]
    fn cumulative_starts_are_monotonic() {
        // est(text) = chars*55 + 400 ms: "one two three"=13 → 1115, "four"=4 → 620
        let segs = vec![
            json!({"text_content": "one two three"}),
            json!({"text_content": "four"}),
            json!({"audio_duration_ms": 1500, "text_content": "x"}),
        ];
        let starts = cumulative_starts(&segs);
        assert_eq!(starts, vec![0, 1115, 1735]);
    }
}
