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

// ─────────────────────────────────────────────────────────────────────────────
// Cover assets (v1.3 craft) — header-only dimension probe + KDP spec checks.
// No image codec dependency: we only need intrinsic pixel dimensions for
// storefront validation, parsed straight from PNG IHDR / JPEG SOF markers.
// ─────────────────────────────────────────────────────────────────────────────

/// Intrinsic pixel dimensions decoded from the image header.
/// Supports PNG (IHDR) and JPEG (any SOF0–SOF15 frame header except the
/// marker-only DHT/DAC/JPG codes). Returns `(width, height)` or `None` when
/// the header is unrecognized/truncated.
pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 24 && bytes[0] == 0x89 && &bytes[1..4] == b"PNG" {
        // PNG: 8-byte signature, IHDR chunk header (8) + length/type (8);
        // width at offset 16, height at 20, both big-endian u32.
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        // JPEG: walk the marker chain until a frame header (SOFn) shows dims.
        let mut i = 2usize;
        while i + 4 <= bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            if marker == 0xFF {
                i += 1; // fill byte before a marker
                continue;
            }
            // Standalone markers carry no length segment.
            if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
                i += 2;
                continue;
            }
            let seg_len =
                usize::from(u16::from_be_bytes(bytes.get(i + 2..i + 4)?.try_into().ok()?));
            if seg_len < 2 {
                return None; // corrupt length — refuse to loop forever
            }
            // SOFn frame headers: C0–CF except C4 (DHT), C8 (JPG), CC (DAC).
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                if i + 9 <= bytes.len() {
                    let h = u16::from_be_bytes(bytes[i + 5..i + 7].try_into().ok()?) as u32;
                    let w = u16::from_be_bytes(bytes[i + 7..i + 9].try_into().ok()?) as u32;
                    return Some((w, h));
                }
                return None;
            }
            i += 2 + seg_len;
        }
    }
    None
}

/// A resolved, validated cover asset ready for EPUB embedding.
pub struct CoverAsset {
    pub bytes: Vec<u8>,
    /// Normalized extension: "jpg" | "png"
    pub ext: &'static str,
    /// MIME: "image/jpeg" | "image/png"
    pub media: &'static str,
    pub width: u32,
    pub height: u32,
}

/// KDP ebook-cover spec: ideal 2560×1600 (1.6:1), minimum 1000 px on the
/// longest side; JPEG/PNG, RGB. Returns human-readable WARN strings (empty
/// when the asset meets spec). Pure — unit-tested, shared by publish_epub
/// and kdp_check.
pub fn cover_spec_warnings(width: u32, height: u32) -> Vec<String> {
    let mut out = Vec::new();
    if width == 0 || height == 0 {
        return vec!["cover image has zero dimensions".to_string()];
    }
    let longest = width.max(height);
    if longest < 1000 {
        out.push(format!(
            "cover is {width}×{height}px — below KDP's 1000px minimum on the longest side; \
             upscale to at least 1000px (ideal 2560×1600)"
        ));
    } else if width < 2560 && height < 1600 {
        out.push(format!(
            "cover is {width}×{height}px — KDP ideal is 2560×1600 for crisp display"
        ));
    }
    let ratio = width as f64 / height as f64;
    if (ratio - 1.6).abs() / 1.6 > 0.15 {
        out.push(format!(
            "cover aspect ratio is {ratio:.2}:1 — KDP ebook covers should be ~1.6:1; \
             the storefront may crop or letterbox"
        ));
    }
    out
}

/// Resolve a book's `cover_image_path` into a validated `CoverAsset`.
/// Fails soft: any problem (unset, sandbox escape, unreadable, undecodable
/// header) yields `None` plus an explanatory `Err`-free path — callers
/// decide whether that is a WARN or a FAIL for their surface.
pub fn resolve_cover(root: &std::path::Path, rel: &str) -> Option<CoverAsset> {
    if rel.trim().is_empty() {
        return None;
    }
    let abs = crate::security::sandbox::resolve_in_root(root, std::path::Path::new(rel)).ok()?;
    if !abs.is_file() {
        return None;
    }
    let bytes = std::fs::read(&abs).ok()?;
    let (w, h) = image_dimensions(&bytes)?;
    let ext = abs
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("jpg") | Some("jpeg") => Some(CoverAsset {
            bytes,
            ext: "jpg",
            media: "image/jpeg",
            width: w,
            height: h,
        }),
        Some("png") => Some(CoverAsset {
            bytes,
            ext: "png",
            media: "image/png",
            width: w,
            height: h,
        }),
        _ => None,
    }
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

    /// Build a minimal PNG in-test: signature + IHDR with the given dims.
    fn png_with_dims(w: u32, h: u32) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(&[0, 0, 0, 13]); // IHDR data length
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth, color, …
        v.extend_from_slice(&[0, 0, 0, 0]); // CRC placeholder (unused here)
        v
    }

    /// Build a minimal JPEG: SOI + APP0 + SOF0 with dims + EOI.
    fn jpeg_with_dims(w: u16, h: u16) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8]; // SOI
        v.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x02]); // APP0, empty
        v.extend_from_slice(&[0xFF, 0xC0]); // SOF0
        v.extend_from_slice(&8u16.to_be_bytes()); // segment length
        v.push(8); // precision
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&[1, 0x11, 0]); // components
        v.extend_from_slice(&[0xFF, 0xD9]); // EOI
        v
    }

    #[test]
    fn image_dimensions_reads_png_ihdr() {
        assert_eq!(image_dimensions(&png_with_dims(2560, 1600)), Some((2560, 1600)));
        assert_eq!(image_dimensions(&png_with_dims(100, 62)), Some((100, 62)));
    }

    #[test]
    fn image_dimensions_reads_jpeg_sof() {
        assert_eq!(image_dimensions(&jpeg_with_dims(1600, 1000)), Some((1600, 1000)));
    }

    #[test]
    fn image_dimensions_rejects_garbage() {
        assert_eq!(image_dimensions(b"not an image at all"), None);
        assert_eq!(image_dimensions(&[]), None);
        assert_eq!(image_dimensions(&[0xFF, 0xD8, 0x00]), None); // truncated JPEG
    }

    #[test]
    fn cover_spec_warns_on_undersized_and_wrong_ratio() {
        // 1×1: below minimum AND wrong ratio.
        let w = cover_spec_warnings(100, 62);
        assert!(w.iter().any(|s| s.contains("1000px minimum")));
        // 1600×1000 = 1.6:1, above minimum, below ideal → only the "ideal" note.
        let w2 = cover_spec_warnings(1600, 1000);
        assert!(w2.len() == 1 && w2[0].contains("2560×1600"));
        // Exact KDP ideal → zero warnings.
        assert!(cover_spec_warnings(2560, 1600).is_empty());
        // Above minimum but 2:1 ratio → ratio warning only.
        let w3 = cover_spec_warnings(2560, 1280);
        assert!(w3.iter().any(|s| s.contains("aspect ratio")));
        assert!(!w3.iter().any(|s| s.contains("1000px minimum")));
    }
}
