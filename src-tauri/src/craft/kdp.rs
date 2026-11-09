// AUTHR_OS — KDP / storefront readiness (contract v1.3).
//
//   kdp_check   — read-only compliance scan: manuscript, exports, cover,
//                 metadata, print pipeline, audio; weighted readiness score.
//   kdp_package — writes a storefront-ready package into exports/kdp/<slug>/
//                 (metadata.json, KDP-safe description.html, copyright page,
//                 launch checklist) and persists it in craft_state.
//
// Royalty math is an offline planning approximation driven by the frozen
// constants in catalog::pricing() — always re-verified against
// kdp.amazon.com before filing real prices.

use super::engine::ensure_craft_schema;
use crate::error::AppError;
use crate::publish::{iso_utc, load_book, slugify, xml_escape};
use serde_json::{json, Value};

// ─────────────────────────────────────────────────────────────────────────────
// Royalty math (pure, unit-tested)
// ─────────────────────────────────────────────────────────────────────────────

fn round2(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}

/// Estimated 6×9 print pages at ~250 words/page (catalog constant).
pub fn est_pages(words: i64) -> i64 {
    (words.max(0) / 250).max(1)
}

/// Estimated EPUB payload size (delivery-fee basis) before a real file exists.
pub fn est_ebook_mb(words: i64) -> f64 {
    round2(words.max(0) as f64 / 10_000.0 * 0.35)
}

/// Amazon ebook royalty. 70% tier inside $2.99–$9.99 minus delivery
/// ($0.15/MB); everything else pays 35% with no delivery fee.
pub fn royalty_ebook(price: f64, file_mb: f64) -> (f64, f64, f64) {
    if (2.99..=9.99).contains(&price) {
        let delivery = round2(0.15 * file_mb);
        (round2(price * 0.70 - delivery), 0.70, delivery)
    } else {
        (round2(price * 0.35), 0.35, 0.0)
    }
}

/// Print cost for KDP paperbacks/hardcovers (US B&W models).
pub fn print_cost(kind: &str, pages: i64) -> f64 {
    match kind {
        "hardcover" => round2(5.65 + 0.0122 * pages as f64),
        _ => round2(0.85 + 0.012 * pages as f64),
    }
}

/// Lowest list price that keeps a print sale non-negative (print/0.60).
pub fn min_print_price(kind: &str, pages: i64) -> f64 {
    (print_cost(kind, pages) / 0.60 * 100.0).ceil() / 100.0
}

fn royalties_block(price: f64, pages: i64, file_mb: f64) -> Value {
    let (eb70, rate, delivery) = royalty_ebook(price, file_mb);
    let (eb35, _, _) = royalty_ebook(if price >= 2.99 && price <= 9.99 { 0.0 } else { price }, file_mb);
    let pb_print = print_cost("paperback", pages);
    let hc_print = print_cost("hardcover", pages);
    json!({
        "list_price_usd": price,
        "ebook": {
            "tier_70": {"eligible": rate == 0.70, "royalty": eb70, "rate": rate,
                        "delivery_fee": delivery, "file_mb_est": file_mb},
            "tier_35": {"royalty": eb35, "rate": 0.35}
        },
        "paperback": {
            "est_pages": pages, "print_cost": pb_print,
            "royalty": round2(price * 0.60 - pb_print), "rate": 0.60,
            "min_list_price": min_print_price("paperback", pages)
        },
        "hardcover": {
            "est_pages": pages, "print_cost": hc_print,
            "royalty": round2(price * 0.60 - hc_print), "rate": 0.60,
            "min_list_price": min_print_price("hardcover", pages)
        },
        "audiobook_acx": {
            "royalty_exclusive_40pct": round2(price * 0.40),
            "royalty_non_exclusive_25pct": round2(price * 0.25),
            "note": "sold via ACX/Audible, not KDP itself"
        },
        "disclaimer": "planning approximation — verify current rates on kdp.amazon.com"
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// KDP-safe description HTML (whitelist sanitizer)
// ─────────────────────────────────────────────────────────────────────────────

/// Escape ALL input, then rebuild structure from the KDP-safe subset only:
/// blank-line-separated paragraphs, leading "- " bullet lists, *bold*,
/// _italic_. No tag from the source can survive — output is generated, never
/// copied, so nothing can smuggle HTML through to the storefront.
pub fn sanitize_description_html(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").trim().to_string();
    if normalized.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        if lines.iter().all(|l| l.trim_start().starts_with("- ")) {
            out.push_str("<ul>");
            for l in lines {
                out.push_str("<li>");
                out.push_str(&inline(&l.trim_start()[2..]));
                out.push_str("</li>");
            }
            out.push_str("</ul>");
        } else {
            out.push_str("<p>");
            out.push_str(&lines
                .iter()
                .map(|l| inline(l.trim()))
                .collect::<Vec<_>>()
                .join("<br/>"));
            out.push_str("</p>");
        }
    }
    out
}

/// Inline markup on ALREADY-ESCAPED text: *b* → <b>, _i_ → <i>.
fn inline(escaped_line: &str) -> String {
    let esc = xml_escape(escaped_line);
    let mut out = String::with_capacity(esc.len());
    let mut emph: Option<(char, Option<String>)> = None; // (marker, buffer)
    for c in esc.chars() {
        if c == '*' || c == '_' {
            match emph.take() {
                Some((m, buf)) if m == c => {
                    // closing marker
                    let body = buf.unwrap_or_default();
                    if body.is_empty() {
                        // bare marker pair — keep it literally
                        out.push(if m == '*' { '*' } else { '_' });
                        out.push(if m == '*' { '*' } else { '_' });
                        // one marker was already pushed as literal below? no —
                        // we pushed none for the opening; push both now.
                    } else if m == '*' {
                        out.push_str(&format!("<b>{body}</b>"));
                    } else {
                        out.push_str(&format!("<i>{body}</i>"));
                    }
                }
                Some((m, buf)) => {
                    // mismatched marker — flush literally
                    out.push(if m == '*' { '*' } else { '_' });
                    if let Some(b) = buf {
                        out.push_str(&b);
                    }
                    emph = Some((c, Some(String::new())));
                    continue;
                }
                None => {
                    emph = Some((c, Some(String::new())));
                    continue;
                }
            }
        }
        match &mut emph {
            Some((_, buf)) => {
                if let Some(b) = buf.as_mut() {
                    b.push(c);
                }
            }
            None => out.push(c),
        }
    }
    if let Some((m, buf)) = emph {
        out.push(if m == '*' { '*' } else { '_' });
        if let Some(b) = buf {
            out.push_str(&b);
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// #60 kdp_check — read-only compliance scan
// ─────────────────────────────────────────────────────────────────────────────

pub fn kdp_check(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let book = crate::storage::books::get(&ctx.conn, book_id)?;
    ensure_craft_schema(&ctx.conn)?;
    let state = super::engine::get_state(&ctx.conn, book_id)?;

    let title = book["title"].as_str().unwrap_or("Untitled");
    let slug = slugify(title);
    let exports = ctx.root.join("exports");
    let exists = |name: &str| exports.join(name).is_file();

    let chapters = crate::storage::chapters::list(&ctx.conn, Some(book_id))?["chapters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let words: i64 = chapters
        .iter()
        .map(|c| c["actual_word_count"].as_i64().unwrap_or(0))
        .sum();

    let epub_ok = exists(&format!("{slug}.epub"));
    let docx_ok = exists(&format!("{slug}.docx"));
    let pdf_ok = exists(&format!("{slug}.pdf"));
    let typ_ok = exists(&format!("{slug}.typ"));
    let m4b_ok = exists(&format!("{slug}.m4b"));

    // Cover: row field must point at an existing file inside the bundle AND
    // decode as a JPG/PNG. Dimension quality (KDP ideal 2560×1600, minimum
    // 1000px longest side, ~1.6:1) is reported as WARN — never silently OK.
    let cover_field = book["cover_image_path"].as_str().unwrap_or("").to_string();
    let cover_ok = !cover_field.is_empty()
        && crate::security::sandbox::resolve_in_root(
            &ctx.root,
            std::path::Path::new(&cover_field),
        )
        .map(|p| p.is_file())
        .unwrap_or(false);
    let cover_asset = if cover_ok {
        crate::publish::resolve_cover(&ctx.root, &cover_field)
    } else {
        None
    };
    let cover_warnings = match &cover_asset {
        Some(c) => crate::publish::cover_spec_warnings(c.width, c.height),
        None => Vec::new(),
    };

    // Metadata: the persisted storefront payload (from kdp_package).
    let sf = state["storefront"].clone();
    let desc = sf["description"].as_str().unwrap_or("");
    let keywords = sf["keywords"].as_array().cloned().unwrap_or_default();
    let categories = sf["categories"].as_array().cloned().unwrap_or_default();
    let mut issues: Vec<Value> = Vec::new();
    let mut metadata_score = 0u32;
    if !desc.is_empty() && (20..=4000).contains(&(desc.chars().count() as i64)) {
        metadata_score += 6;
    } else if !desc.is_empty() {
        issues.push(json!({"severity":"WARN","area":"metadata","message":
            format!("storefront description is {} chars (valid band 20-4000)", desc.chars().count()),
            "fix":"rerun kdp_package with a corrected description"}));
    } else {
        issues.push(json!({"severity":"FAIL","area":"metadata","message":
            "no storefront description packaged","fix":"run kdp_package with description/marketing_copy"}));
    }
    if !keywords.is_empty() && keywords.len() <= 7 {
        metadata_score += 5;
    } else {
        issues.push(json!({"severity":"FAIL","area":"metadata","message":
            format!("{} keyword slots packaged (KDP requires 1-7)", keywords.len()),
            "fix":"run kdp_package with up to 7 keywords"}));
    }
    if !categories.is_empty() && categories.len() <= 2 {
        metadata_score += 4;
    } else {
        issues.push(json!({"severity":"WARN","area":"metadata","message":
            format!("{} categories packaged (storefronts take 1-2)", categories.len()),
            "fix":"run kdp_package with 1-2 categories"}));
    }

    // Craft application state.
    let craft_applied = !state["plan"].as_array().unwrap_or(&vec![]).is_empty()
        || book["persona_id"].as_str().unwrap_or("Standard") != "Standard";
    if !craft_applied {
        issues.push(json!({"severity":"WARN","area":"craft","message":
            "no craft persona/arc applied — beat scaffolding and LLM directives are on defaults",
            "fix":"run craft_apply then craft_scaffold"}));
    }

    // Audio coverage.
    let (total_segs, voiced): (i64, i64) = ctx.conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN s.audio_hash IS NOT NULL THEN 1 ELSE 0 END),0)
         FROM chapter_segments s JOIN chapters c ON c.id = s.chapter_id
         WHERE c.book_id = ?1",
        rusqlite::params![book_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let audio_pct = if total_segs > 0 {
        (voiced as f64 * 100.0 / total_segs as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };

    // Scoring (weights sum to 100).
    let manuscript_score = if chapters.len() > 0 && words > 0 {
        25u32
    } else if !chapters.is_empty() {
        10
    } else {
        0
    };
    if chapters.is_empty() {
        issues.push(json!({"severity":"FAIL","area":"manuscript","message":
            "book has no chapters","fix":"craft_scaffold then write segments"}));
    } else if words == 0 {
        issues.push(json!({"severity":"FAIL","area":"manuscript","message":
            "chapters exist but contain no prose yet","fix":"author segments in the Novel studio"}));
    }
    let epub_score = if epub_ok { 20 } else { 0 };
    if !epub_ok {
        issues.push(json!({"severity":"FAIL","area":"epub","message":
            format!("exports/{slug}.epub not built yet"),
            "fix":"publish_epub book_id"}));
    }
    let cover_score = if cover_ok {
        15
    } else if !cover_field.is_empty() {
        5
    } else {
        0
    };
    if !cover_ok {
        issues.push(json!({"severity":"FAIL","area":"cover","message":
            "no cover image on file (storefronts require one)",
            "fix":"set cover_image_path (book_update) — KDP ideal 2560×1600 JPG, ratio 1.6:1"}));
    } else if cover_asset.is_none() {
        issues.push(json!({"severity":"WARN","area":"cover","message":
            format!("cover at {cover_field} is not a decodable JPG/PNG (header unreadable)"),
            "fix":"replace with a JPG/PNG inside the bundle"}));
    } else {
        for wmsg in &cover_warnings {
            issues.push(json!({"severity":"WARN","area":"cover","message": wmsg,
                "fix":"replace the cover asset with a KDP-spec image (2560×1600 JPG, 1.6:1)"}));
        }
    }
    let print_score = if pdf_ok {
        10
    } else if typ_ok {
        6
    } else {
        0
    };
    if !pdf_ok && !typ_ok {
        issues.push(json!({"severity":"WARN","area":"print","message":
            "no print-ready PDF/typst source (paperback unavailable)",
            "fix":"publish_typst book_id with a typst binary"}));
    }
    let audio_score = if m4b_ok {
        10
    } else if audio_pct >= 80.0 {
        7
    } else if audio_pct >= 30.0 {
        4
    } else {
        0
    };
    if !m4b_ok {
        issues.push(json!({"severity":"INFO","area":"audio","message":
            format!("no M4B audiobook export; {audio_pct}% of segments voiced"),
            "fix":"audio_synthesize (SCRIPTED works offline) then publish_m4b"}));
    }
    let polished = chapters.iter().all(|c| {
        matches!(
            c["status"].as_str().unwrap_or("PLANNED"),
            "POLISHED" | "AUDIO_READY" | "PUBLISHED"
        )
    });
    let polish_score = if polished && !chapters.is_empty() { 5 } else { 0 };

    let readiness_score = manuscript_score
        + epub_score
        + cover_score
        + metadata_score
        + print_score
        + audio_score
        + polish_score;
    let has_fail = issues.iter().any(|i| i["severity"] == "FAIL");
    let pass = readiness_score >= 70 && !has_fail;

    let pages = est_pages(words);
    let file_mb = match std::fs::metadata(exports.join(format!("{slug}.epub"))) {
        Ok(m) => round2(m.len() as f64 / 1_000_000.0),
        _ => est_ebook_mb(words),
    };
    let price = crate::ipc::args::get_f64(a, "price_usd").unwrap_or(4.99);

    Ok(json!({
        "book_id": book_id,
        "pass": pass,
        "readiness_score": readiness_score,
        "checks": {
            "manuscript": {"score": manuscript_score, "chapters": chapters.len(), "words": words},
            "epub": {"score": epub_score, "out": format!("exports/{slug}.epub"), "present": epub_ok},
            "docx": {"present": docx_ok},
            "cover": {"score": cover_score, "path": cover_field, "present": cover_ok,
                      "decodable": cover_asset.is_some(),
                      "width": cover_asset.as_ref().map(|c| c.width).unwrap_or(0),
                      "height": cover_asset.as_ref().map(|c| c.height).unwrap_or(0),
                      "warnings": cover_warnings},
            "metadata": {"score": metadata_score, "keywords": keywords.len(),
                         "categories": categories.len(), "description_chars": desc.chars().count()},
            "print": {"score": print_score, "pdf": pdf_ok, "typ": typ_ok},
            "audio": {"score": audio_score, "m4b": m4b_ok, "voiced_pct": audio_pct},
            "polish": {"score": polish_score, "all_polished": polished}
        },
        "issues": issues,
        "pricing_hint": royalties_block(price, pages, file_mb),
        "words_per_print_page": 250,
        "est_pages": pages
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// #61 kdp_package — write the storefront bundle into exports/kdp/<slug>/
// ─────────────────────────────────────────────────────────────────────────────

pub fn kdp_package(ctx: &mut crate::ipc::Ctx, a: &Value) -> crate::error::IpcResult {
    let book_id = crate::ipc::args::req_i64(a, "book_id")?;
    let data = load_book(&ctx.conn, book_id)?; // ENTITY_NOT_FOUND + no-chapters guard
    ensure_craft_schema(&ctx.conn)?;
    let state = super::engine::get_state(&ctx.conn, book_id)?;
    let book = &data.book;

    // ── validate args ────────────────────────────────────────────────────────
    let price = crate::ipc::args::get_f64(a, "price_usd").unwrap_or(4.99);
    if !(0.99..=200.0).contains(&price) {
        return Err(AppError::craft(format!(
            "price_usd {price} outside KDP planning band 0.99-200.00"
        )));
    }
    let mut keywords: Vec<String> = Vec::new();
    if let Some(arr) = a.get("keywords").and_then(|v| v.as_array()) {
        for k in arr {
            let s = k.as_str().unwrap_or_default().trim().to_string();
            if !s.is_empty() {
                keywords.push(s);
            }
        }
    }
    if keywords.len() > 7 {
        return Err(AppError::craft(format!(
            "{} keywords given — KDP allows at most 7 keyword slots",
            keywords.len()
        )));
    }
    for k in &keywords {
        if k.chars().count() > 50 {
            return Err(AppError::craft(format!(
                "keyword `{k}` exceeds 50 characters (KDP limit)"
            )));
        }
    }
    let mut categories: Vec<String> = Vec::new();
    if let Some(arr) = a.get("categories").and_then(|v| v.as_array()) {
        for c in arr {
            let s = c.as_str().unwrap_or_default().trim().to_string();
            if !s.is_empty() {
                categories.push(s);
            }
        }
    }
    if categories.len() > 2 {
        return Err(AppError::craft(format!(
            "{} categories given — storefronts take at most 2",
            categories.len()
        )));
    }
    for c in &categories {
        if c.chars().count() > 100 {
            return Err(AppError::craft(format!(
                "category `{c}` exceeds 100 characters"
            )));
        }
    }
    let title = book["title"].as_str().unwrap_or("Untitled").to_string();
    let author = book["author"].as_str().unwrap_or("Anonymous").to_string();
    let subtitle = crate::ipc::args::get_str(a, "subtitle").unwrap_or_default();
    let language = crate::ipc::args::get_str(a, "language").unwrap_or_else(|| "en".into());
    let isbn = crate::ipc::args::get_str(a, "isbn")
        .unwrap_or_else(|| book["isbn"].as_str().unwrap_or_default().to_string());
    let publisher = crate::ipc::args::get_str(a, "publisher").unwrap_or_else(|| author.clone());
    let edition = crate::ipc::args::get_str(a, "edition").unwrap_or_else(|| "First Edition".into());
    let ai_disclosure = a.get("ai_disclosure").and_then(|v| v.as_bool()).unwrap_or(false);
    let drm = a.get("drm").and_then(|v| v.as_bool()).unwrap_or(false);
    let publication_date = crate::ipc::args::get_str(a, "publication_date")
        .unwrap_or_else(|| iso_utc()[..10].to_string());

    let words: i64 = data
        .chapters
        .iter()
        .map(|c| c.row["actual_word_count"].as_i64().unwrap_or(0))
        .sum();
    let slug = slugify(&title);
    let exports = ctx.root.join("exports");
    let file_mb = match std::fs::metadata(exports.join(format!("{slug}.epub"))) {
        Ok(m) => round2(m.len() as f64 / 1_000_000.0),
        _ => est_ebook_mb(words),
    };
    let pages = est_pages(words);

    let raw_description = crate::ipc::args::get_str(a, "description")
        .or_else(|| {
            let mc = book["marketing_copy"].as_str().unwrap_or_default();
            if mc.trim().is_empty() {
                None
            } else {
                Some(mc.to_string())
            }
        })
        .unwrap_or_default();
    let desc_len = raw_description.trim().chars().count();
    if !(20..=4000).contains(&desc_len) {
        return Err(AppError::craft(format!(
            "description is {desc_len} chars (KDP band: 20-4000); pass `description` or fill book marketing_copy"
        )));
    }
    let description_html = sanitize_description_html(&raw_description);

    // ── package directory (exports sandbox) ─────────────────────────────────
    let pkg_rel = format!("kdp/{slug}");
    let pkg_dir = ctx.root.join("exports").join("kdp").join(&slug);
    if pkg_rel.contains("..") || slug.contains("..") {
        return Err(AppError::traversal(format!("bad package dir {pkg_rel}")));
    }
    std::fs::create_dir_all(&pkg_dir).map_err(|e| AppError::bundle(format!("create kdp dir: {e}")))?;

    let mut issues: Vec<Value> = Vec::new();
    let epub_ok = exports.join(format!("{slug}.epub")).is_file();
    if !epub_ok {
        issues.push(json!({"severity":"WARN","message":"EPUB not built yet — run publish_epub before uploading"}));
    }

    let metadata = json!({
        "title": title,
        "subtitle": subtitle,
        "author": author,
        "publisher": publisher,
        "edition": edition,
        "language": language,
        "publication_date": publication_date,
        "isbn": isbn,
        "drm": drm,
        "ai_disclosure": ai_disclosure,
        "description": raw_description.trim(),
        "keywords": keywords,
        "categories": categories,
        "pricing": {"list_price_usd": price},
        "format_id": state["format_id"],
        "plot_arc_id": state["plot_arc_id"],
        "style_persona_id": state["style_persona_id"],
        "words": words,
        "est_pages": pages,
        "generated_at": iso_utc(),
        "generator": "AUTHR_OS kdp_package (contract v1.3)"
    });
    let royalties = royalties_block(price, pages, file_mb);

    // ── metadata.json ────────────────────────────────────────────────────────
    let mut files: Vec<Value> = Vec::new();
    let write = |name: &str, body: &str, files: &mut Vec<Value>| -> Result<(), AppError> {
        let path = pkg_dir.join(name);
        std::fs::write(&path, body).map_err(|e| AppError::bundle(format!("write {name}: {e}")))?;
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        files.push(json!({"name": name, "bytes": bytes}));
        Ok(())
    };
    write(
        "metadata.json",
        &format!("{}\n", serde_json::to_string_pretty(&metadata).unwrap_or_default()),
        &mut files,
    )?;
    write("description.html", &format!("{description_html}\n"), &mut files)?;
    write("copyright-page.xhtml", &copyright_page(&metadata), &mut files)?;
    write(
        "checklist.md",
        &checklist_md(&metadata, &royalties, &slug, epub_ok),
        &mut files,
    )?;

    // ── persist storefront payload in craft_state ───────────────────────────
    let storefront = json!({
        "description": raw_description.trim(),
        "keywords": keywords,
        "categories": categories,
        "language": language,
        "subtitle": subtitle,
        "isbn": isbn,
        "publisher": publisher,
        "edition": edition,
        "drm": drm,
        "ai_disclosure": ai_disclosure,
        "price_usd": price,
        "publication_date": publication_date,
        "package_dir": format!("exports/{pkg_rel}"),
        "packaged_at": iso_utc()
    });
    ctx.conn.execute(
        "INSERT INTO craft_state(book_id, format_id, plot_arc_id, style_persona_id, plan_json, storefront_json)
         VALUES (?1,
                 COALESCE((SELECT format_id FROM craft_state WHERE book_id = ?1), 'NOVEL'),
                 COALESCE((SELECT plot_arc_id FROM craft_state WHERE book_id = ?1), 'SAVE_THE_CAT'),
                 COALESCE((SELECT style_persona_id FROM craft_state WHERE book_id = ?1), 'EDGE_THRILLER'),
                 COALESCE((SELECT plan_json FROM craft_state WHERE book_id = ?1), '[]'),
                 ?2)
         ON CONFLICT(book_id) DO UPDATE SET
           storefront_json = excluded.storefront_json,
           updated_at = strftime('%s','now')",
        rusqlite::params![book_id, storefront.to_string()],
    )?;
    ctx.after_commit()?;

    Ok(json!({
        "book_id": book_id,
        "package_dir": format!("exports/{pkg_rel}"),
        "files": files,
        "metadata": metadata,
        "royalties": royalties,
        "issues": issues,
        "words": words,
        "est_pages": pages
    }))
}

/// Front-matter copyright page (XHTML fragment, all values escaped).
fn copyright_page(m: &Value) -> String {
    let title = m["title"].as_str().unwrap_or("Untitled");
    let author = m["author"].as_str().unwrap_or("Anonymous");
    let subtitle = m["subtitle"].as_str().unwrap_or("");
    let year: String = m["publication_date"].as_str().unwrap_or("2026")[..4].to_string();
    let isbn = m["isbn"].as_str().unwrap_or("");
    let publisher = m["publisher"].as_str().unwrap_or(author);
    let edition = m["edition"].as_str().unwrap_or("First Edition");
    let disclosure = m["ai_disclosure"].as_bool().unwrap_or(false);

    let mut out = String::new();
    out.push_str("<section class=\"copyright-page\">\n");
    out.push_str(&format!("  <p><b>{}</b></p>\n", xml_escape(title)));
    if !subtitle.is_empty() {
        out.push_str(&format!("  <p>{}</p>\n", xml_escape(subtitle)));
    }
    out.push_str(&format!(
        "  <p>Copyright \u{00A9} {year} {}. All rights reserved.</p>\n",
        xml_escape(author)
    ));
    if !isbn.is_empty() {
        out.push_str(&format!("  <p>ISBN: {}</p>\n", xml_escape(isbn)));
    }
    out.push_str(&format!(
        "  <p>Published by {}.</p>\n  <p>{edition}.</p>\n",
        xml_escape(publisher)
    ));
    if disclosure {
        out.push_str(
            "  <p>This work was drafted with AI-assisted tooling (AUTHR_OS) under the author's creative direction and final editorial control.</p>\n",
        );
    }
    out.push_str(
        "  <p>No part of this publication may be reproduced, distributed, or transmitted in any form without the prior written permission of the publisher, except for brief quotations in reviews.</p>\n",
    );
    out.push_str("  <p>This is a work of the author's creation; names, characters and events are the author's own or used fictitiously.</p>\n");
    out.push_str("</section>\n");
    out
}

/// Storefront launch checklist (markdown) with the packaged values inlined.
fn checklist_md(m: &Value, r: &Value, slug: &str, epub_ok: bool) -> String {
    let title = m["title"].as_str().unwrap_or("Untitled");
    let price = m["pricing"]["list_price_usd"].as_f64().unwrap_or(4.99);
    let keywords = m["keywords"].as_array().cloned().unwrap_or_default();
    let categories = m["categories"].as_array().cloned().unwrap_or_default();
    let kw = keywords
        .iter()
        .map(|k| k.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("; ");
    let cats = categories
        .iter()
        .map(|k| k.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" | ");
    let eb = &r["ebook"]["tier_70"];
    let pb = &r["paperback"];
    let hc = &r["hardcover"];
    let epub_state = if epub_ok { "(built — ready)" } else { "(NOT built yet — run publish_epub)" };
    let drm_state = if m["drm"].as_bool().unwrap_or(false) {
        "on"
    } else {
        "off (recommended for wide reach)"
    };
    let language = m["language"].as_str().unwrap_or("en");
    let pub_date = m["publication_date"].as_str().unwrap_or("");
    let ai_disc = m["ai_disclosure"].as_bool().unwrap_or(false);
    let eb_royalty = eb["royalty"].as_f64().unwrap_or(0.0);
    let pages = pb["est_pages"].as_i64().unwrap_or(0);
    let pb_print = pb["print_cost"].as_f64().unwrap_or(0.0);
    let pb_royalty = pb["royalty"].as_f64().unwrap_or(0.0);
    let pb_min = pb["min_list_price"].as_f64().unwrap_or(0.0);
    let hc_print = hc["print_cost"].as_f64().unwrap_or(0.0);
    let hc_royalty = hc["royalty"].as_f64().unwrap_or(0.0);
    let hc_min = hc["min_list_price"].as_f64().unwrap_or(0.0);
    format!(
        r#"# Storefront Launch Checklist — {title}

Generated by AUTHR_OS `kdp_package`. Package: `exports/kdp/{slug}/`.

## 1. Amazon KDP — eBook (kdp.amazon.com)
- [ ] Manuscript upload: `exports/{slug}.epub` {epub_state}
- [ ] Cover: 2560×1600 JPG (ratio 1.6:1), min 1000px on the long edge
- [ ] Title / subtitle / author exactly as in `metadata.json`
- [ ] Description: paste `description.html` (KDP-safe subset already enforced)
- [ ] 7 keyword slots: {kw}
- [ ] Categories (pick closest 2): {cats}
- [ ] Price ${price:.2} → projected royalty ${eb_royalty} (70% tier, minus delivery) — or 35% tier outside $2.99-$9.99
- [ ] DRM: {drm_state}
- [ ] Language: {language} · Publication date: {pub_date}

## 2. Amazon KDP — Paperback (6"×9")
- [ ] Interior PDF: `exports/{slug}.pdf` via `publish_typst` (install a Typst binary)
- [ ] est. {pages} pages → print cost ${pb_print} · royalty ${pb_royalty} · minimum list ${pb_min}
- [ ] Hardcover option: print ${hc_print} · royalty ${hc_royalty} · minimum list ${hc_min}

## 3. Front matter
- [ ] `copyright-page.xhtml` reviewed (ISBN, edition, AI disclosure: {ai_disc})

## 4. Wide distribution (non-Amazon)
- [ ] Draft2Digital / Apple Books / Kobo Writing Life / Google Play Books — reuse `metadata.json` + `description.html`
- [ ] Keep pricing parity; enable pre-order where offered

## 5. Audiobook (optional)
- [ ] `publish_m4b` → `exports/{slug}.m4b`, then ACX (40% exclusive / 25% non-exclusive share)
- [ ] ACX audio spec enforced by `audio_validate` (RMS −23…−18 dB, peak ≤ −3 dB, noise ≤ −60 dB)

## 6. Marketing assets already in the bundle
- [ ] `books.marketing_copy` → description seed
- [ ] `books.cover_prompt` → image-generation brief
- [ ] Beat map (`craft_progress`) → series/episode planning

Rates are offline planning approximations — confirm on kdp.amazon.com before publishing.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn royalty_math_golden_values() {
        // 70% tier, $4.99, 1.0 MB → 3.493 − 0.15 = 3.34
        let (r, rate, d) = royalty_ebook(4.99, 1.0);
        assert_eq!((r, rate, d), (3.34, 0.70, 0.15));
        // outside tier → 35%, no delivery
        let (r2, rate2, d2) = royalty_ebook(12.0, 1.0);
        assert_eq!((r2, rate2, d2), (4.20, 0.35, 0.0));
        // tier edges
        assert_eq!(royalty_ebook(2.99, 0.0).1, 0.70);
        assert_eq!(royalty_ebook(9.99, 0.0).1, 0.70);
        assert_eq!(royalty_ebook(2.98, 0.0).1, 0.35);
        assert_eq!(royalty_ebook(10.0, 0.0).1, 0.35);
    }

    #[test]
    fn print_math_golden_values() {
        assert_eq!(est_pages(80_000), 320);
        assert_eq!(est_pages(0), 1);
        // 320 pages: 0.85 + 3.84 = 4.69
        assert_eq!(print_cost("paperback", 320), 4.69);
        assert_eq!(print_cost("hardcover", 320), 9.55); // 5.65 + 3.904 → 9.554 → 9.55
        // min price = ceil(print/0.60)
        assert_eq!(min_print_price("paperback", 320), 7.82); // 4.69/0.6 = 7.8166
    }

    #[test]
    fn royalties_block_shape() {
        let block = royalties_block(4.99, 320, 1.0);
        assert_eq!(block["ebook"]["tier_70"]["royalty"], 3.34);
        assert_eq!(block["paperback"]["min_list_price"], 7.82);
        assert!(block["audiobook_acx"]["royalty_exclusive_40pct"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn sanitizer_is_whitelist_only() {
        let raw = "<script>alert(1)</script>\n\n*A* bold and _i_ ital\n\n- one\n- two\n\nplain tail";
        let html = sanitize_description_html(raw);
        assert!(!html.contains("<script"), "no source tag may survive: {html}");
        assert!(html.starts_with("<p>"));
        assert!(html.contains("<b>A</b>"));
        assert!(html.contains("<i>i</i>"));
        assert!(html.contains("<ul><li>one</li><li>two</li></ul>"));
        assert!(html.ends_with("<p>plain tail</p>"));
    }

    #[test]
    fn sanitizer_handles_empty_and_crlf() {
        assert_eq!(sanitize_description_html(""), "");
        assert_eq!(sanitize_description_html("   \n  "), "");
        assert_eq!(
            sanitize_description_html("a\r\n\r\nb"),
            "<p>a</p><p>b</p>"
        );
    }

    #[test]
    fn copyright_page_escapes_and_discloses() {
        let m = json!({
            "title": "The <Clock> & Tide", "author": "Ann O'Bryan",
            "subtitle": "", "publication_date": "2026-11-09", "isbn": "978-1-00000-000-X",
            "publisher": "Merrow House", "edition": "First Edition",
            "ai_disclosure": true
        });
        let page = copyright_page(&m);
        assert!(page.contains("The &lt;Clock&gt; &amp; Tide"));
        assert!(page.contains("Ann O&#39;Bryan"));
        assert!(page.contains("ISBN: 978-1-00000-000-X"));
        assert!(page.contains("AI-assisted tooling"));
        assert!(page.contains("Copyright \u{00A9} 2026"));
    }
}
