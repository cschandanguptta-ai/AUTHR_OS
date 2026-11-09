// AUTHR_OS — EPUB 3 compiler with SMIL media overlays (Phase 6, Sprint 14).
//
// Structure (EPUBCheck-aligned):
//   mimetype                    STORED, first entry, "application/epub+zip"
//   META-INF/container.xml      rootfile → OEBPS/content.opf
//   OEBPS/content.opf           Dublin Core metadata, manifest, spine,
//                               media-overlay bindings, dcterms:modified
//   OEBPS/nav.xhtml             EPUB 3 navigation document (properties="nav")
//   OEBPS/style.css             typesetting stylesheet
//   OEBPS/chNN.xhtml            one per chapter, segments carry id="sN"
//   OEBPS/overlay/chNN.smil     media overlay per chapter (when synthesized)
//   OEBPS/audio/<hash>.wav      embedded synthesis chunks (when present)

use crate::error::AppError;
use crate::publish::{
    cast_names, cumulative_starts, exports_path, iso_utc, load_book, slugify, turns, xml_escape,
    BookData,
};
use serde_json::{json, Value};
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const EPUB_MIME: &str = "application/epub+zip";

/// #47 publish_epub — 1-click EPUB 3 with whisper-sync media overlays.
/// Storefront-grade output (v1.3): embeds the bundle cover when present,
/// carries title + copyright front matter in the spine, and enriches the
/// Dublin Core metadata from the persisted KDP storefront payload
/// (`kdp_package`) so the file is upload-ready for Amazon KDP et al.
pub fn publish_epub(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let data = load_book(&ctx.conn, book_id)?;
    let cast = cast_names(&ctx.conn);
    let title = data.book["title"].as_str().unwrap_or("Untitled");
    let slug = slugify(title);
    let out_rel = format!("{slug}.epub");
    let out_path = exports_path(&ctx.root, &out_rel)?;

    // Storefront payload (may be empty when kdp_package never ran). Purely
    // additive metadata: every field falls back to book-row values.
    let storefront = crate::craft::engine::get_state(&ctx.conn, book_id)
        .map(|s| s["storefront"].clone())
        .unwrap_or_else(|_| json!({}));

    // Cover: resolved from the sandboxed bundle root; soft-fail to no cover.
    let cover_rel = data.book["cover_image_path"].as_str().unwrap_or("").to_string();
    let cover = crate::publish::resolve_cover(&ctx.root, &cover_rel);
    let cover_warnings = match &cover {
        Some(c) => crate::publish::cover_spec_warnings(c.width, c.height),
        None => Vec::new(),
    };

    let mut overlays = 0usize;
    let mut audio_items = 0usize;
    let bytes = build_epub(&ctx.root, &data, &cast, &storefront, cover.as_ref(), &mut overlays, &mut audio_items)?;
    std::fs::write(&out_path, bytes).map_err(|e| AppError::bundle(format!("write epub: {e}")))?;
    ctx.after_commit()?;

    let duration_ms: u64 = data
        .chapters
        .iter()
        .map(|ch| cumulative_starts(&ch.segments).last().copied().unwrap_or(0))
        .sum();
    let dc = storefront_dc(&data, &storefront);
    Ok(json!({
        "out": format!("exports/{out_rel}"),
        "format": "EPUB 3",
        "entries": 4 + data.chapters.len() * 2 + audio_items
                   + if cover.is_some() { 2 } else { 0 } + 2,
        "chapters": data.chapters.len(),
        "overlays": overlays,
        "duration_ms": duration_ms,
        "dc": dc,
        "cover": match &cover {
            Some(c) => json!({
                "embedded": true,
                "path": cover_rel,
                "media": c.media,
                "width": c.width,
                "height": c.height,
                "ratio": format!("{:.3}", c.width as f64 / c.height as f64),
                "warnings": cover_warnings,
            }),
            None => json!({
                "embedded": false,
                "warnings": cover_warnings,
                "hint": if cover_rel.is_empty() {
                    "set cover_image_path (book_update) — storefronts require a cover"
                } else {
                    "cover_image_path did not resolve to a readable JPG/PNG inside the bundle"
                },
            }),
        },
        "front_matter": ["title.xhtml", "copyright.xhtml"],
    }))
}

/// Metadata resolution for the OPF Dublin Core and the result payload.
/// Storefront fields (kdp_package) win, book-row fields are the fallback;
/// every default is deterministic so the same bundle republishes byte-stable
/// apart from the dcterms:modified timestamp.
fn storefront_dc(data: &BookData, sf: &Value) -> Value {
    let book = &data.book;
    let author = book["author"].as_str().unwrap_or("Anonymous");
    let description = sf["description"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            book["marketing_copy"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| book["premise"].as_str().unwrap_or(""))
        });
    let subject = sf["categories"]
        .as_array()
        .filter(|a| !a.is_empty())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_else(|| book["genre"].as_str().unwrap_or("Fiction").to_string());
    let publisher = sf["publisher"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            // One-person publishing house: the author is the imprint when
            // kdp_package never named a publisher.
            if author.is_empty() || author == "Anonymous" {
                "AUTHR_OS".to_string()
            } else {
                format!("{author} (independent)")
            }
        });
    let language = sf["language"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("en");
    // EPUBCheck-legal unique identifier: deterministic UUIDv5 over the book
    // id — stable across republishes (byte-stable EPUB, no random churn) and
    // always a valid `urn:uuid:` form.
    let uid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_DNS,
        format!("authr-os-book-{}", book["id"]).as_bytes(),
    );
    json!({
        "title": book["title"].as_str().unwrap_or("Untitled"),
        "creator": author,
        "identifier": format!("urn:uuid:{uid}"),
        "language": language,
        "publisher": publisher,
        "description": description,
        "subject": subject,
        "subtitle": sf["subtitle"].as_str().unwrap_or(""),
        "isbn": sf["isbn"].as_str().filter(|s| !s.is_empty()).unwrap_or_else(|| book["isbn"].as_str().unwrap_or("")),
    })
}

fn build_epub(
    root: &std::path::Path,
    data: &BookData,
    cast: &std::collections::HashMap<String, String>,
    storefront: &Value,
    cover: Option<&crate::publish::CoverAsset>,
    overlays: &mut usize,
    audio_items: &mut usize,
) -> Result<Vec<u8>, AppError> {
    let title = data.book["title"].as_str().unwrap_or("Untitled");
    let author = data.book["author"].as_str().unwrap_or("Anonymous");
    let now = iso_utc();
    let dc = storefront_dc(data, storefront);
    let subtitle = dc["subtitle"].as_str().unwrap_or("");
    let publisher = dc["publisher"].as_str().unwrap_or("AUTHR_OS");

    // Collect audio chunks referenced by synthesized segments.
    let mut embedded: Vec<(String, std::path::PathBuf)> = Vec::new(); // (hash, path)
    let mut ch_has_audio: Vec<bool> = Vec::new();
    for ch in &data.chapters {
        let mut any = false;
        for seg in &ch.segments {
            if let (Some(h), Some(p)) = (
                seg["audio_hash"].as_str().map(str::to_string),
                seg["audio_cache_path"].as_str().map(str::to_string),
            ) {
                if !embedded.iter().any(|(eh, _)| *eh == h) {
                    let abs = crate::security::sandbox::resolve_in_root(
                        root,
                        std::path::Path::new(&p),
                    )?;
                    embedded.push((h, abs));
                }
                any = true;
            }
        }
        ch_has_audio.push(any);
    }
    *audio_items = embedded.len();

    let file = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    // mimetype must be the first entry, uncompressed (OCF requirement).
    zip.start_file(
        "mimetype",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .map_err(|e| AppError::bundle(format!("epub mimetype: {e}")))?;
    zip.write_all(EPUB_MIME.as_bytes())
        .map_err(|e| AppError::bundle(format!("epub mimetype: {e}")))?;

    let add = |zip: &mut ZipWriter<std::io::Cursor<Vec<u8>>>, name: &str, body: &str| -> Result<(), AppError> {
        #[allow(clippy::redundant_closure)]
        let name = name.to_string();
        zip.start_file(name.as_str(), opts)
            .map_err(|e| AppError::bundle(format!("epub entry {name}: {e}")))?;
        zip.write_all(body.as_bytes())
            .map_err(|e| AppError::bundle(format!("epub entry {name}: {e}")))?;
        Ok(())
    };

    add(
        &mut zip,
        "META-INF/container.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#,
    )?;

    // ── content.opf ─────────────────────────────────────────────────────────
    // Spine order: [cover page] → title → copyright → chapters. Front matter
    // is always present (storefront requirement); the cover page only when a
    // cover asset resolved.
    let mut manifest = String::from(
        r#"    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="style.css" media-type="text/css"/>
    <item id="title-page" href="title.xhtml" media-type="application/xhtml+xml"/>
    <item id="copyright-page" href="copyright.xhtml" media-type="application/xhtml+xml"/>
"#,
    );
    let mut spine = String::from(
        "    <itemref idref=\"title-page\"/>\n    <itemref idref=\"copyright-page\"/>\n",
    );
    if let Some(c) = cover {
        manifest.push_str(&format!(
            "    <item id=\"cover-image\" href=\"images/cover.{ext}\" media-type=\"{media}\" properties=\"cover-image\"/>\n    <item id=\"cover-page\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>\n",
            ext = c.ext,
            media = c.media,
        ));
        spine.insert_str(0, "    <itemref idref=\"cover-page\"/>\n");
    }
    for (i, _ch) in data.chapters.iter().enumerate() {
        let nn = i + 1;
        manifest.push_str(&format!(
            "    <item id=\"ch{nn:02}\" href=\"ch{nn:02}.xhtml\" media-type=\"application/xhtml+xml\"{}/>\n",
            if ch_has_audio[i] {
                format!(" media-overlay=\"ov{nn:02}\"")
            } else {
                String::new()
            }
        ));
        if ch_has_audio[i] {
            manifest.push_str(&format!(
                "    <item id=\"ov{nn:02}\" href=\"overlay/ch{nn:02}.smil\" media-type=\"application/smil+xml\"/>\n"
            ));
        }
        spine.push_str(&format!("    <itemref idref=\"ch{nn:02}\"/>\n"));
    }
    for (h, _) in &embedded {
        manifest.push_str(&format!(
            "    <item id=\"au{}\" href=\"audio/{h}.wav\" media-type=\"audio/wav\"/>\n",
            &h[..12.min(h.len())]
        ));
    }
    let total_dur_secs: u64 = data
        .chapters
        .iter()
        .map(|ch| {
            let starts = cumulative_starts(&ch.segments);
            starts.last().copied().unwrap_or(0)
        })
        .sum::<u64>()
        / 1000;
    let opf = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="bookid" xml:lang="en">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">{}</dc:identifier>
    <dc:title>{}</dc:title>
    <dc:creator>{}</dc:creator>
    <dc:language>{}</dc:language>
    <dc:publisher>{}</dc:publisher>
    <dc:description>{}</dc:description>
    <dc:subject>{}</dc:subject>
    <meta property="dcterms:modified">{}</meta>
    <meta property="media:duration" refines="#ovall">{:02}:{:02}:{:02}</meta>
  </metadata>
  <manifest>
{}
  </manifest>
  <spine>
{}
  </spine>
</package>
"##,
        xml_escape(&dc["identifier"].as_str().unwrap_or("urn:uuid:0")),
        xml_escape(title),
        xml_escape(author),
        xml_escape(dc["language"].as_str().unwrap_or("en")),
        xml_escape(publisher),
        xml_escape(dc["description"].as_str().unwrap_or("")),
        xml_escape(dc["subject"].as_str().unwrap_or("Fiction")),
        now,
        total_dur_secs / 3600,
        (total_dur_secs % 3600) / 60,
        total_dur_secs % 60,
        manifest,
        spine,
    );
    add(&mut zip, "OEBPS/content.opf", &opf)?;

    let nav_items = {
        let mut items = Vec::new();
        if cover.is_some() {
            items.push("      <li><a href=\"cover.xhtml\">Cover</a></li>".to_string());
        }
        items.push("      <li><a href=\"title.xhtml\">Title Page</a></li>".to_string());
        items.push("      <li><a href=\"copyright.xhtml\">Copyright</a></li>".to_string());
        for (i, ch) in data.chapters.iter().enumerate() {
            items.push(format!(
                "      <li><a href=\"ch{:02}.xhtml\">{}</a></li>",
                i + 1,
                xml_escape(ch.row["title"].as_str().unwrap_or("Chapter"))
            ));
        }
        items.join("\n")
    };
    add(
        &mut zip,
        "OEBPS/nav.xhtml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>{}</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>Contents</h1>
    <ol>
{}
    </ol>
  </nav>
</body>
</html>
"#,
            xml_escape(title),
            nav_items,
        ),
    )?;

    add(
        &mut zip,
        "OEBPS/style.css",
        "body{font-family:serif;line-height:1.6;margin:1em}\n\
         p.speaker{font-weight:bold}\np.mono{font-style:italic;color:#333}\n\
         h1.ch{page-break-before:always}\n\
         .title-page{text-align:center;margin-top:6em}\n\
         .title-page h1{font-size:2em;border:none}\n\
         .title-page .subtitle{font-style:italic;font-weight:normal}\n\
         .title-page .byline{margin-top:3em;font-size:1.2em}\n\
         .copyright{font-size:0.85em;color:#333;margin-top:3em}\n\
         .copyright p{text-indent:0}\n",
    )?;

    // ── cover page (when an asset resolved) + front matter ─────────────────
    if let Some(c) = cover {
        zip.start_file(format!("OEBPS/images/cover.{}", c.ext), opts)
            .map_err(|e| AppError::bundle(format!("epub cover: {e}")))?;
        zip.write_all(&c.bytes)
            .map_err(|e| AppError::bundle(format!("epub cover bytes: {e}")))?;
        add(
            &mut zip,
            "OEBPS/cover.xhtml",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Cover</title>
<meta name="viewport" content="width=1000, height=625"/>
<style type="text/css">@page {{ padding: 0; margin: 0; }}
body {{ padding: 0; margin: 0; text-align: center; }}
img {{ max-width: 100%; height: auto; }}</style>
</head>
<body>
  <div>
    <img src="images/cover.{ext}" alt="{alt}"/>
  </div>
</body>
</html>
"#,
                ext = c.ext,
                alt = xml_escape(&format!("Cover of {}", title)),
            ),
        )?;
    }

    add(
        &mut zip,
        "OEBPS/title.xhtml",
        &title_page_xhtml(&dc, subtitle, publisher),
    )?;
    add(
        &mut zip,
        "OEBPS/copyright.xhtml",
        &copyright_page_xhtml(&dc, storefront, title, author, publisher),
    )?;


    // ── chapter XHTML + overlays ────────────────────────────────────────────
    for (i, ch) in data.chapters.iter().enumerate() {
        let nn = i + 1;
        let ch_title = ch.row["title"].as_str().unwrap_or("Chapter");
        let ch_num = ch.row["chapter_number"].as_i64().unwrap_or(nn as i64);
        let mut body = String::new();
        for t in turns(ch, cast) {
            let esc = xml_escape(t.text);
            match t.kind {
                "DIALOGUE" => body.push_str(&format!(
                    "    <p id=\"s{}\"><span class=\"speaker\">{}</span>: \u{201C}{esc}\u{201D}</p>\n",
                    t.seq, xml_escape(&t.speaker)
                )),
                "INTERNAL_MONOLOGUE" => {
                    body.push_str(&format!("    <p class=\"mono\" id=\"s{}\"><em>{esc}</em></p>\n", t.seq))
                }
                _ => body.push_str(&format!("    <p id=\"s{}\">{esc}</p>\n", t.seq)),
            }
        }
        add(
            &mut zip,
            &format!("OEBPS/ch{nn:02}.xhtml"),
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>{}</title><link rel="stylesheet" type="text/css" href="style.css"/></head>
<body>
  <h1 class="ch">Chapter {}: {}</h1>
{}
</body>
</html>
"#,
                xml_escape(ch_title),
                ch_num,
                xml_escape(ch_title),
                body,
            ),
        )?;

        if ch_has_audio[i] {
            *overlays += 1;
            let starts = cumulative_starts(&ch.segments);
            let mut pars = String::new();
            for (si, seg) in ch.segments.iter().enumerate() {
                let dur = crate::publish::segment_duration_ms(seg);
                let begin = fmt_clock(starts[si]);
                let end = fmt_clock(starts[si] + dur);
                if let (Some(h), _) = (
                    seg["audio_hash"].as_str(),
                    seg["audio_cache_path"].as_str(),
                ) {
                    pars.push_str(&format!(
                        r#"    <par id="p{si}">
      <text src="ch{nn:02}.xhtml#s{si}"/>
      <audio src="../audio/{h}.wav" clipBegin="{begin}" clipEnd="{end}"/>
    </par>
"#
                    ));
                }
            }
            add(
                &mut zip,
                &format!("OEBPS/overlay/ch{nn:02}.smil"),
                &format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<smil xmlns="http://www.w3.org/ns/SMIL" version="3.0" profile="http://www.idpf.org/epub/30/profile/content/">
  <body>
{}
  </body>
</smil>
"#,
                pars,
            ),
        )?;
        }
    }

    // ── embedded audio chunks ───────────────────────────────────────────────
    for (h, abs) in &embedded {
        let bytes = std::fs::read(abs).map_err(|e| AppError::bundle(format!("read chunk: {e}")))?;
        zip.start_file(format!("OEBPS/audio/{h}.wav"), opts)
            .map_err(|e| AppError::bundle(format!("epub audio: {e}")))?;
        zip.write_all(&bytes)
            .map_err(|e| AppError::bundle(format!("epub audio: {e}")))?;
    }

    let cursor = zip
        .finish()
        .map_err(|e| AppError::bundle(format!("epub finish: {e}")))?;
    Ok(cursor.into_inner())
}

fn fmt_clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}.{:03}", s / 3600, (s % 3600) / 60, s % 60, ms % 1000)
}

/// Title page: title, optional subtitle, byline, imprint.
fn title_page_xhtml(dc: &Value, subtitle: &str, publisher: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>{title}</title><link rel="stylesheet" type="text/css" href="style.css"/></head>
<body>
  <div class="title-page">
    <h1>{title}</h1>
    {subtitle_p}
    <p class="byline">by {author}</p>
    <p class="publisher">{publisher}</p>
  </div>
</body>
</html>
"#,
        title = xml_escape(dc["title"].as_str().unwrap_or("Untitled")),
        subtitle_p = if subtitle.is_empty() {
            String::new()
        } else {
            format!("    <h2 class=\"subtitle\">{}</h2>\n", xml_escape(subtitle))
        },
        author = xml_escape(dc["creator"].as_str().unwrap_or("Anonymous")),
        publisher = xml_escape(publisher),
    )
}

/// Copyright page: notice, rights boilerplate, imprint/edition/ISBN, and the
/// AI-assistance disclosure when the storefront payload declared one.
/// Amazon requires an AI-content declaration at upload time; carrying the
/// disclosure inside the book keeps the artifact self-consistent.
fn copyright_page_xhtml(dc: &Value, sf: &Value, title: &str, author: &str, publisher: &str) -> String {
    let year = chrono::Utc::now().format("%Y");
    let pub_date = chrono::Utc::now().format("%B %d, %Y");
    // ISBN line only when the book actually carries one — KDP assigns an
    // ASIN automatically, so an invented placeholder would be a lie.
    let isbn = sf["isbn"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| dc["isbn"].as_str().filter(|s| !s.is_empty()).map(str::to_string));
    let edition = sf["edition"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("First Edition");
    let ai = sf["ai_disclosure"].as_bool().unwrap_or(false);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Copyright</title><link rel="stylesheet" type="text/css" href="style.css"/></head>
<body>
  <h1>Copyright</h1>
  <div class="copyright">
    <p><b>{title}</b></p>
    <p>Copyright &#169; {year} by {author}</p>
    <p>All rights reserved.</p>
    <p>No part of this publication may be reproduced, distributed, or transmitted in any form or by any means, including photocopying, recording, or other electronic or mechanical methods, without the prior written permission of the publisher, except in the case of brief quotations embodied in critical reviews and certain other noncommercial uses permitted by copyright law.</p>
    <p>Publisher: {publisher}</p>
    <p>Edition: {edition}</p>
    <p>Published: {pub_date}</p>
    {isbn_p}
    {ai_p}
  </div>
</body>
</html>
"#,
        title = xml_escape(title),
        author = xml_escape(author),
        publisher = xml_escape(publisher),
        edition = xml_escape(edition),
        isbn_p = match &isbn {
            Some(v) => format!("    <p>ISBN: {}</p>", xml_escape(v)),
            None => String::new(),
        },
        ai_p = if ai {
            "\n    <p>This book was created with AI-assisted tooling; the author reviewed, edited, and approved the final text.</p>"
        } else {
            ""
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_format_is_hhmmss_mmm() {
        assert_eq!(fmt_clock(0), "00:00:00.000");
        assert_eq!(fmt_clock(3_723_456), "01:02:03.456");
    }

    #[test]
    fn storefront_dc_prefers_storefront_then_book_row() {
        let data = BookData {
            book: json!({"id": 7, "title": "The Ember Gate", "author": "R. Vale",
                         "genre": "Sci-Fi", "premise": "premise text",
                         "marketing_copy": "marketing text"}),
            chapters: vec![],
        };
        // No storefront: marketing_copy wins over premise, genre is subject,
        // the author becomes the imprint.
        let dc = storefront_dc(&data, &json!({}));
        assert_eq!(dc["description"], "marketing text");
        assert_eq!(dc["subject"], "Sci-Fi");
        assert_eq!(dc["publisher"], "R. Vale (independent)");
        assert_eq!(dc["language"], "en");
        // Storefront overrides.
        let sf = json!({"description": "sf desc", "categories": ["Fiction / Thriller"],
                        "publisher": "Vale Press", "language": "de",
                        "subtitle": "A Station Signal Novel"});
        let dc2 = storefront_dc(&data, &sf);
        assert_eq!(dc2["description"], "sf desc");
        assert_eq!(dc2["subject"], "Fiction / Thriller");
        assert_eq!(dc2["publisher"], "Vale Press");
        assert_eq!(dc2["language"], "de");
        assert_eq!(dc2["subtitle"], "A Station Signal Novel");
    }

    #[test]
    fn storefront_dc_anonymous_falls_back_to_imprint() {
        let data = BookData {
            book: json!({"id": 1, "title": "T", "author": "Anonymous"}),
            chapters: vec![],
        };
        let dc = storefront_dc(&data, &json!({}));
        assert_eq!(dc["publisher"], "AUTHR_OS");
    }

    #[test]
    fn title_page_renders_subtitle_only_when_present() {
        let dc = json!({"title": "T <1>", "creator": "A & B"});
        let no_sub = title_page_xhtml(&dc, "", "Imprint");
        assert!(!no_sub.contains("subtitle"));
        let sub = title_page_xhtml(&dc, "A <Novel>", "Imprint");
        assert!(sub.contains("<h2 class=\"subtitle\">A &lt;Novel&gt;</h2>"));
        // XML-escaped title/author, no raw metachars.
        assert!(no_sub.contains("T &lt;1&gt;"));
        assert!(no_sub.contains("A &amp; B"));
    }

    #[test]
    fn storefront_dc_uses_deterministic_valid_uuid() {
        let data = BookData {
            book: json!({"id": 7, "title": "T", "author": "A"}),
            chapters: vec![],
        };
        let dc = storefront_dc(&data, &json!({}));
        let id = dc["identifier"].as_str().unwrap();
        assert!(id.starts_with("urn:uuid:"));
        let uid = uuid::Uuid::parse_str(id.trim_start_matches("urn:uuid:")).expect("valid uuid");
        // Republish stability: same book id → same UUID (byte-stable EPUB).
        let again = storefront_dc(&data, &json!({}));
        assert_eq!(again["identifier"], id);
        assert_eq!(uid.get_version_num(), 5);
    }

    #[test]
    fn copyright_page_carries_ai_disclosure_when_declared() {
        let dc = json!({"identifier": "urn:uuid:abc", "isbn": ""});
        let plain = copyright_page_xhtml(&dc, &json!({}), "T", "A", "P");
        assert!(plain.contains("Copyright &#169;"));
        assert!(plain.contains("All rights reserved."));
        // No ISBN anywhere → the ISBN line is omitted, never invented.
        assert!(!plain.contains("ISBN:"));
        assert!(!plain.contains("AI-assisted tooling"));
        // Book-row ISBN shows without a storefront payload.
        let row_isbn = copyright_page_xhtml(
            &json!({"identifier": "urn:uuid:abc", "isbn": "978-3-000000-00-0"}),
            &json!({}), "T", "A", "P",
        );
        assert!(row_isbn.contains("ISBN: 978-3-000000-00-0"));
        let disclosed = copyright_page_xhtml(
            &json!({"identifier": "urn:uuid:abc", "isbn": ""}),
            &json!({"ai_disclosure": true, "isbn": "978-1-00000-000-0"}), "T", "A", "P",
        );
        assert!(disclosed.contains("AI-assisted tooling"));
        assert!(disclosed.contains("ISBN: 978-1-00000-000-0"));
    }
}
