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
pub fn publish_epub(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let data = load_book(&ctx.conn, book_id)?;
    let cast = cast_names(&ctx.conn);
    let title = data.book["title"].as_str().unwrap_or("Untitled");
    let slug = slugify(title);
    let out_rel = format!("{slug}.epub");
    let out_path = exports_path(&ctx.root, &out_rel)?;

    let mut overlays = 0usize;
    let mut audio_items = 0usize;
    let bytes = build_epub(&ctx.root, &data, &cast, &mut overlays, &mut audio_items)?;
    std::fs::write(&out_path, bytes).map_err(|e| AppError::bundle(format!("write epub: {e}")))?;
    ctx.after_commit()?;

    let duration_ms: u64 = data
        .chapters
        .iter()
        .map(|ch| cumulative_starts(&ch.segments).last().copied().unwrap_or(0))
        .sum();
    Ok(json!({
        "out": format!("exports/{out_rel}"),
        "format": "EPUB 3",
        "entries": 4 + data.chapters.len() * 2 + audio_items,
        "chapters": data.chapters.len(),
        "overlays": overlays,
        "duration_ms": duration_ms,
        "dc": {
            "title": title,
            "creator": data.book["author"].as_str().unwrap_or("Anonymous"),
            "identifier": format!("urn:uuid:{}", data.book["id"]),
            "language": "en",
            "publisher": "AUTHR_OS",
        },
    }))
}

fn build_epub(
    root: &std::path::Path,
    data: &BookData,
    cast: &std::collections::HashMap<String, String>,
    overlays: &mut usize,
    audio_items: &mut usize,
) -> Result<Vec<u8>, AppError> {
    let title = data.book["title"].as_str().unwrap_or("Untitled");
    let author = data.book["author"].as_str().unwrap_or("Anonymous");
    let now = iso_utc();

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
    let mut manifest = String::from(
        r#"    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="style.css" media-type="text/css"/>
"#,
    );
    let mut spine = String::new();
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
    <dc:identifier id="bookid">urn:uuid:{}</dc:identifier>
    <dc:title>{}</dc:title>
    <dc:creator>{}</dc:creator>
    <dc:language>en</dc:language>
    <dc:publisher>AUTHR_OS</dc:publisher>
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
        data.book["id"],
        xml_escape(title),
        xml_escape(author),
        now,
        total_dur_secs / 3600,
        (total_dur_secs % 3600) / 60,
        total_dur_secs % 60,
        manifest,
        spine,
    );
    add(&mut zip, "OEBPS/content.opf", &opf)?;

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
            data.chapters
                .iter()
                .enumerate()
                .map(|(i, ch)| format!(
                    "      <li><a href=\"ch{:02}.xhtml\">{}</a></li>",
                    i + 1,
                    xml_escape(ch.row["title"].as_str().unwrap_or("Chapter"))
                ))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    )?;

    add(
        &mut zip,
        "OEBPS/style.css",
        "body{font-family:serif;line-height:1.6;margin:1em}\n\
         p.speaker{font-weight:bold}\np.mono{font-style:italic;color:#333}\n\
         h1.ch{page-break-before:always}\n",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_format_is_hhmmss_mmm() {
        assert_eq!(fmt_clock(0), "00:00:00.000");
        assert_eq!(fmt_clock(3_723_456), "01:02:03.456");
    }
}
