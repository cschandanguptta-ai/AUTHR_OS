// AUTHR_OS — standard manuscript DOCX exporter (Phase 6, Sprint 14).
//
// Minimal, valid OOXML: [Content_Types].xml, _rels/.rels, word/document.xml,
// word/styles.xml. Chapter headings use the Heading1 style; NARRATION is a
// normal justified paragraph, DIALOGUE renders a bold speaker run followed by
// the quoted line, INTERNAL_MONOLOGUE renders italic.

use crate::error::AppError;
use crate::publish::{cast_names, exports_path, iso_utc, load_book, slugify, turns, xml_escape};
use serde_json::{json, Value};
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// #48 publish_docx — standard manuscript export for editors/agents.
pub fn publish_docx(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let data = load_book(&ctx.conn, book_id)?;
    let cast = cast_names(&ctx.conn);
    let title = data.book["title"].as_str().unwrap_or("Untitled");
    let slug = slugify(title);
    let out_rel = format!("{slug}.docx");
    let out_path = exports_path(&ctx.root, &out_rel)?;

    let bytes = build_docx(&data, &cast)?;
    std::fs::write(&out_path, bytes).map_err(|e| AppError::bundle(format!("write docx: {e}")))?;
    ctx.after_commit()?;

    let paragraphs: usize = 1
        + data.chapters.len()
        + data
            .chapters
            .iter()
            .map(|ch| ch.segments.len())
            .sum::<usize>();
    Ok(json!({
        "out": format!("exports/{out_rel}"),
        "chapters": data.chapters.len(),
        "paragraphs": paragraphs,
        "words": data.chapters.iter()
            .map(|ch| ch.segments.iter()
                .map(|s| s["text_content"].as_str().map(|t| t.split_whitespace().count()).unwrap_or(0))
                .sum::<usize>())
            .sum::<usize>(),
    }))
}

fn w_para(runs: &str, style: Option<&str>) -> String {
    let p_pr = style
        .map(|s| format!(r#"<w:pPr><w:pStyle w:val="{s}"/></w:pPr>"#))
        .unwrap_or_default();
    format!(r#"<w:p>{p_pr}{runs}</w:p>"#)
}

fn w_run(text: &str, bold: bool, italic: bool) -> String {
    let mut rpr = String::new();
    if bold {
        rpr.push_str("<w:b/>");
    }
    if italic {
        rpr.push_str("<w:i/>");
    }
    let rpr = if rpr.is_empty() {
        String::new()
    } else {
        format!(r#"<w:rPr>{rpr}</w:rPr>"#)
    };
    format!(
        r#"<w:r>{rpr}<w:t xml:space="preserve">{}</w:t></w:r>"#,
        xml_escape(text)
    )
}

fn build_docx(
    data: &crate::publish::BookData,
    cast: &std::collections::HashMap<String, String>,
) -> Result<Vec<u8>, AppError> {
    let title = data.book["title"].as_str().unwrap_or("Untitled");
    let author = data.book["author"].as_str().unwrap_or("Anonymous");

    let mut body = String::new();
    body.push_str(&w_para(&w_run(title, true, false), Some("Title")));
    body.push_str(&w_para(&w_run(author, false, true), Some("Subtitle")));

    for (i, ch) in data.chapters.iter().enumerate() {
        let ch_title = ch.row["title"].as_str().unwrap_or("Chapter");
        let ch_num = ch.row["chapter_number"].as_i64().unwrap_or(i as i64 + 1);
        body.push_str(&w_para(
            &w_run(&format!("Chapter {ch_num}: {ch_title}"), true, false),
            Some("Heading1"),
        ));
        for t in turns(ch, cast) {
            match t.kind {
                "DIALOGUE" => {
                    let runs = format!(
                        "{}{}",
                        w_run(&format!("{}: ", t.speaker), true, false),
                        w_run(&format!("\u{201C}{}\u{201D}", t.text), false, false)
                    );
                    body.push_str(&w_para(&runs, None));
                }
                "INTERNAL_MONOLOGUE" => {
                    body.push_str(&w_para(&w_run(t.text, false, true), None))
                }
                _ => body.push_str(&w_para(&w_run(t.text, false, false), None)),
            }
        }
    }

    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
{body}
    <w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>
  </w:body>
</w:document>
"#
    );

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
</Types>
"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
</Relationships>
"#;
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:sz w:val="48"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:i/><w:sz w:val="28"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
  <w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/><w:pPr><w:jc w:val="both"/></w:pPr></w:style>
</w:styles>
"#;

    let file = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default();
    let mut add = |name: &str, body: &str| -> Result<(), AppError> {
        zip.start_file(name, opts)
            .map_err(|e| AppError::bundle(format!("docx entry {name}: {e}")))?;
        zip.write_all(body.as_bytes())
            .map_err(|e| AppError::bundle(format!("docx entry {name}: {e}")))?;
        Ok(())
    };
    add("[Content_Types].xml", content_types)?;
    add("_rels/.rels", rels)?;
    add("word/document.xml", &document)?;
    add("word/styles.xml", styles)?;
    add(
        "docProps/core.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{}</dc:title><dc:creator>{}</dc:creator><dcterms:modified xsi:type="dcterms:W3CDTF">{}</dcterms:modified>
</cp:coreProperties>
"#,
            xml_escape(title),
            xml_escape(author),
            iso_utc()
        ),
    )?;
    let cursor = zip
        .finish()
        .map_err(|e| AppError::bundle(format!("docx finish: {e}")))?;
    Ok(cursor.into_inner())
}
