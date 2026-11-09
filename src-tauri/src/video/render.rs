// AUTHR_OS — Video asset rendering (contract v1.6, ADR-0011 + ADR-0012).
//
// Engines mirror the audio stack's negotiation pattern exactly:
//   SCRIPTED      — deterministic offline painter (`video::frame`), the
//                   default that always works headless (like scripted TTS)
//   R2I_ENDPOINT  — OpenAI-compatible image endpoint through the provider
//                   settings ladder (ADR-0012: args > provider_settings table
//                   > env `AUTHR_IMG_ENDPOINT` > fail-closed). Fails CLOSED
//                   with the canonical `AI_ENGINE_UNAVAILABLE` error when no
//                   live provider resolves — never fakes output.
//
// v1.6 upgrades (novelcast pipeline parity):
//   * SCENE frames honor `engine = R2I_ENDPOINT` too — the scene prompt carries
//     the segment text verbatim + every in-frame character's bio verbatim (the
//     consistency ladder, ADR-0004 → ADR-0011) + the book's art style fragment.
//   * Art style (§4.18 `video_style`) joins the SHA-256 seed material of every
//     rendered asset, so a style switch deterministically repaints while a
//     given style stays byte-identical.
//   * `missing_only: true` renders only assets whose registry row (or backing
//     file) is absent — the assembler's ensure-render never clobbers live
//     endpoint art with a scripted card.
//
// Assets are registered in `video_assets` (kind: TITLE | SHEET | SCENE; the
// book-level sheet sentinel is chapter_number = 0). Rendering is idempotent:
// unchanged inputs reproduce byte-identical SCRIPTED output, so a re-render
// overwrites in place and the registry row is upserted.

use super::derive::{ensure_video_schema, scenes_for};
use super::frame;
use super::style;
use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Engine {
    Scripted,
    Endpoint,
}

impl Engine {
    pub fn parse(s: &str) -> Result<Engine, AppError> {
        match s {
            "SCRIPTED" | "DEVICE" => Ok(Engine::Scripted),
            "R2I_ENDPOINT" | "ENDPOINT" => Ok(Engine::Endpoint),
            other => Err(AppError::audio(format!(
                "unknown video image engine `{other}` (expected SCRIPTED|R2I_ENDPOINT)"
            ))),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Engine::Scripted => "SCRIPTED",
            Engine::Endpoint => "R2I_ENDPOINT",
        }
    }
}

struct Meta {
    book_id: i64,
    chapter_number: i64,
    book_title: String,
    chapter_title: String,
}

fn load_meta(conn: &rusqlite::Connection, chapter_id: i64) -> Result<Meta, AppError> {
    conn.query_row(
        "SELECT b.id, c.chapter_number, b.title, c.title
         FROM chapters c JOIN books b ON b.id = c.book_id
         WHERE c.id = ?1",
        [chapter_id],
        |r| {
            Ok(Meta {
                book_id: r.get(0)?,
                chapter_number: r.get(1)?,
                book_title: r.get(2)?,
                chapter_title: r.get(3)?,
            })
        },
    )
    .map_err(|_| AppError::not_found(format!("chapter {chapter_id} does not exist")))
}

struct CastEntry {
    key: String,
    display: String,
    role: String,
    bio: String,
}

fn load_cast(conn: &rusqlite::Connection) -> Result<Vec<CastEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT character_key, display_name, role, COALESCE(bio, '')
         FROM character_profiles ORDER BY character_key",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CastEntry {
                key: r.get(0)?,
                display: r.get(1)?,
                role: r.get(2)?,
                bio: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Segment text per segment_id (Endpoint scene tier needs the verbatim
/// narration for the prompt; SCRIPTED never loads it — no double work).
fn load_segment_text(
    conn: &rusqlite::Connection,
    chapter_id: i64,
) -> Result<HashMap<String, String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT segment_id, text_content FROM chapter_segments WHERE chapter_id = ?1",
    )?;
    let rows = stmt
        .query_map([chapter_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

fn asset_dir(root: &std::path::Path, book_id: i64, chapter_number: i64) -> PathBuf {
    root.join("media").join("video").join(format!("b{book_id}")).join(format!("ch{chapter_number}"))
}

fn sanitize_key(key: &str) -> String {
    let cleaned: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect();
    cleaned.chars().take(24).collect()
}

struct AssetRecord<'a> {
    book_id: i64,
    chapter_number: i64,
    kind: &'a str,
    ref_key: &'a str,
    engine: &'a str,
    path: &'a str,
    sha: &'a str,
    width: u32,
    height: u32,
    bytes: usize,
}

fn register_asset(conn: &rusqlite::Connection, r: &AssetRecord) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO video_assets(book_id, chapter_number, kind, ref_key, engine, path,
                                  sha256, width, height, bytes, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
         ON CONFLICT(book_id, chapter_number, kind, ref_key) DO UPDATE SET
             engine = excluded.engine, path = excluded.path, sha256 = excluded.sha256,
             width = excluded.width, height = excluded.height, bytes = excluded.bytes,
             updated_at = excluded.updated_at",
        rusqlite::params![r.book_id, r.chapter_number, r.kind, r.ref_key, r.engine,
                          r.path, r.sha, r.width as i64, r.height as i64, r.bytes as i64],
    )
    .map_err(AppError::from)?;
    Ok(())
}

fn existing_assets(conn: &rusqlite::Connection, book_id: i64, chapter_number: i64) -> Result<Vec<Value>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT kind, ref_key, engine, path, sha256, width, height, bytes
         FROM video_assets
         WHERE book_id = ?1 AND chapter_number IN (0, ?2)
         ORDER BY kind, ref_key",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![book_id, chapter_number], |r| {
            Ok(json!({
                "kind": r.get::<_, String>(0)?,
                "ref": r.get::<_, String>(1)?,
                "engine": r.get::<_, String>(2)?,
                "path": r.get::<_, String>(3)?,
                "sha256": r.get::<_, String>(4)?,
                "width": r.get::<_, i64>(5)?,
                "height": r.get::<_, i64>(6)?,
                "bytes": r.get::<_, i64>(7)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Registry row present AND backing file alive → the asset is "live".
/// `missing_only` renders skip live assets (no endpoint-art clobbering).
fn asset_live(
    conn: &rusqlite::Connection,
    root: &std::path::Path,
    book_id: i64,
    chapter_number: i64,
    kind: &str,
    ref_key: &str,
) -> Result<bool, AppError> {
    let path: Option<String> = conn
        .query_row(
            "SELECT path FROM video_assets
             WHERE book_id = ?1 AND chapter_number = ?2 AND kind = ?3 AND ref_key = ?4",
            rusqlite::params![book_id, chapter_number, kind, ref_key],
            |r| r.get(0),
        )
        .ok();
    Ok(match path {
        Some(p) => root.join(&p).is_file(),
        None => false,
    })
}

fn fetch_endpoint_image(
    prompt: &str,
    size: &str,
    conn: &rusqlite::Connection,
    target: &std::path::Path,
) -> Result<(String, String, u32, u32, usize), AppError> {
    let (bytes, ext, _cfg) = crate::providers::fetch_image_resolved(conn, prompt, size)?;
    let p = target.with_extension(ext);
    std::fs::create_dir_all(target.parent().unwrap_or(p.as_path()))?;
    std::fs::write(&p, &bytes)?;
    let sha = Sha256::digest(&bytes);
    let (w, h) = crate::publish::image_dimensions(&bytes).ok_or_else(|| {
        AppError::ai_engine_unavailable(
            "R2I_ENDPOINT returned an image with an unreadable header — refusing to register it",
        )
    })?;
    Ok((p.to_string_lossy().into_owned(), hex(&sha), w, h, bytes.len()))
}

/// §4.16 `video_render` — render title card, character reference sheets and
/// scene frames for one chapter. Idempotent; `sheets_only` restricts to the
/// R2I sheet tier; `scene_index` restricts to one scene; `missing_only`
/// renders only assets missing from the registry (assembler ensure path);
/// `focal` (map of scene_index → position) updates the persisted Ken Burns
/// anchors.
pub fn video_render(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    let engine = Engine::parse(args::get_str(a, "engine").as_deref().unwrap_or("SCRIPTED"))?;
    let sheets_only = a.get("sheets_only").and_then(|v| v.as_bool()).unwrap_or(false);
    let missing_only = a.get("missing_only").and_then(|v| v.as_bool()).unwrap_or(false);
    let scene_filter = args::get_i64(a, "scene_index");
    ensure_video_schema(&ctx.conn)?;

    // Draft-time contract: derive (cheap catch-up) before consuming.
    super::derive::refresh_chapter_media(&ctx.conn, chapter_id)?;
    let meta = load_meta(&ctx.conn, chapter_id)?;
    let scenes = scenes_for(&ctx.conn, chapter_id)?;
    if scenes.is_empty() {
        return Err(AppError::not_found(format!(
            "chapter {chapter_id} has no segments — nothing to render"
        )));
    }
    let style = style::style_for(&ctx.conn, meta.book_id)?;

    // Optional focal updates (Scene Studio focal selector persistence).
    if let Some(focal_map) = a.get("focal").and_then(|v| v.as_object()) {
        for (k, v) in focal_map {
            if let (Ok(idx), Some(pos)) = (k.parse::<i64>(), v.as_str()) {
                if ["center", "left", "right", "top", "bottom"].contains(&pos) {
                    ctx.conn.execute(
                        "UPDATE video_scenes SET focal = ?1 WHERE chapter_id = ?2 AND scene_index = ?3",
                        rusqlite::params![pos, chapter_id, idx],
                    )?;
                }
            }
        }
    }

    let cast = load_cast(&ctx.conn)?;
    let texts = match engine {
        Engine::Endpoint => load_segment_text(&ctx.conn, chapter_id)?,
        Engine::Scripted => HashMap::new(),
    };
    let dir = asset_dir(&ctx.root, meta.book_id, meta.chapter_number);
    let mut rendered: Vec<Value> = Vec::new();
    let mut reused = 0usize;

    // ── Character reference sheets (book-level sentinel chapter_number = 0) ──
    // Principal roles mentioned in this chapter, minus those already sheeted.
    let mentioned: Vec<&String> = scenes
        .iter()
        .flat_map(|s| s.characters.iter())
        .collect();
    let mut sheet_targets: Vec<&CastEntry> = cast
        .iter()
        .filter(|c| {
            (c.role.eq_ignore_ascii_case("PROTAGONIST")
                || c.role.eq_ignore_ascii_case("SUPPORTING"))
                && mentioned.contains(&&c.key)
        })
        .collect();
    if let Some(list) = a.get("characters").and_then(|v| v.as_array()) {
        sheet_targets = cast
            .iter()
            .filter(|c| {
                list.iter()
                    .filter_map(|x| x.as_str())
                    .any(|k| k == c.key)
            })
            .collect();
    }
    for c in &sheet_targets {
        if missing_only
            && asset_live(&ctx.conn, &ctx.root, meta.book_id, 0, "SHEET", &c.key)?
        {
            reused += 1;
            continue;
        }
        let seed = Sha256::digest(sheet_seed(meta.book_id, &c.key, style.id).as_bytes());
        // Sheets live one level up (book scope): media/video/b{book}/char_*.bmp
        let sheet_path = ctx
            .root
            .join("media")
            .join("video")
            .join(format!("b{}", meta.book_id))
            .join(format!("char_{}.bmp", sanitize_key(&c.key)));
        let (path, sha, w, h, n): (String, String, u32, u32, usize) = match engine {
            Engine::Scripted => {
                let surf = frame::render_sheet(&seed, &c.display, &c.role, &c.bio);
                let bytes = frame::write_bmp(&sheet_path, &surf)?;
                let sha = Sha256::digest(&bytes);
                (rel_of(&ctx.root, &sheet_path), hex(&sha), surf.w, surf.h, bytes.len())
            }
            Engine::Endpoint => {
                let prompt = sheet_prompt(&c.display, &c.role, &c.bio, style.prompt);
                let (rel, sha, w, h, n) =
                    fetch_endpoint_image(&prompt, "1024x1024", &ctx.conn, &sheet_path)?;
                (rel_of(&ctx.root, std::path::Path::new(&rel)), sha, w, h, n)
            }
        };
        register_asset(&ctx.conn, &AssetRecord {
            book_id: meta.book_id, chapter_number: 0, kind: "SHEET", ref_key: &c.key,
            engine: engine.as_str(), path: &path, sha: &sha, width: w, height: h, bytes: n,
        })?;
        rendered.push(json!({
            "kind": "SHEET", "ref": c.key, "path": path, "sha256": sha,
            "width": w, "height": h, "bytes": n, "engine": engine.as_str(),
        }));
    }
    if sheets_only {
        ctx.after_commit()?;
        return Ok(json!({
            "chapter_id": chapter_id, "book_id": meta.book_id,
            "chapter_number": meta.chapter_number,
            "engine": engine.as_str(), "style": style.id,
            "rendered": rendered, "reused": reused,
        }));
    }

    // ── Title card (typography — stays SCRIPTED on every tier) ──
    if scene_filter.is_none() {
        if missing_only
            && asset_live(&ctx.conn, &ctx.root, meta.book_id, meta.chapter_number, "TITLE", "title")?
        {
            reused += 1;
        } else {
            let seed = Sha256::digest(title_seed(meta.book_id, meta.chapter_number, style.id).as_bytes());
            let path = dir.join("title.bmp");
            let surf = frame::render_title(&seed, &meta.book_title, &meta.chapter_title, meta.chapter_number);
            let bytes = frame::write_bmp(&path, &surf)?;
            let sha = hex(&Sha256::digest(&bytes));
            let rel = rel_of(&ctx.root, &path);
            register_asset(&ctx.conn, &AssetRecord {
                book_id: meta.book_id, chapter_number: meta.chapter_number, kind: "TITLE",
                ref_key: "title", engine: "SCRIPTED", path: &rel, sha: &sha,
                width: surf.w, height: surf.h, bytes: bytes.len(),
            })?;
            rendered.push(json!({
                "kind": "TITLE", "ref": "title", "path": rel, "sha256": sha,
                "width": surf.w, "height": surf.h, "bytes": bytes.len(),
                "engine": "SCRIPTED",
            }));
        }
    }

    // ── Scene frames (SCRIPTED painter or R2I_ENDPOINT live tier) ──
    let total = scenes.len();
    for s in &scenes {
        if let Some(want) = scene_filter {
            if s.scene_index as i64 != want {
                continue;
            }
        }
        if missing_only
            && asset_live(
                &ctx.conn, &ctx.root, meta.book_id, meta.chapter_number,
                "SCENE", &s.segment_id,
            )?
        {
            reused += 1;
            continue;
        }
        let seed = Sha256::digest(
            scene_seed(meta.book_id, meta.chapter_number, s.scene_index, &s.segment_id, style.id)
                .as_bytes(),
        );
        let label = scene_label(&cast, s);
        let tag = format!("CH {} · {}", meta.chapter_number, meta.chapter_title);
        let path_base = dir.join(format!("scene_{}.bmp", s.scene_index));
        let (path, sha, w, h, n, chars_json): (String, String, u32, u32, usize, Value) = match engine {
            Engine::Scripted => {
                let surf = frame::render_scene(&seed, s.scene_index, total, &label, &tag);
                let bytes = frame::write_bmp(&path_base, &surf)?;
                let sha = hex(&Sha256::digest(&bytes));
                (rel_of(&ctx.root, &path_base), sha, surf.w, surf.h, bytes.len(), json!(s.characters))
            }
            Engine::Endpoint => {
                let text = texts.get(&s.segment_id).map(|t| t.as_str()).unwrap_or("");
                let bios: Vec<&CastEntry> = cast
                    .iter()
                    .filter(|c| s.characters.contains(&c.key))
                    .collect();
                let prompt = scene_prompt(
                    s.scene_index, total, &meta.book_title, &meta.chapter_title,
                    text, &bios, style.prompt,
                );
                let (rel, sha, w, h, n) =
                    fetch_endpoint_image(&prompt, "1280x720", &ctx.conn, &path_base)?;
                (rel, sha, w, h, n, json!(s.characters))
            }
        };
        let existed = existing_assets(&ctx.conn, meta.book_id, meta.chapter_number)?
            .iter()
            .any(|x| x["kind"] == "SCENE" && x["ref"] == s.segment_id && x["sha256"] == sha);
        if existed {
            reused += 1;
        }
        register_asset(&ctx.conn, &AssetRecord {
            book_id: meta.book_id, chapter_number: meta.chapter_number, kind: "SCENE",
            ref_key: &s.segment_id, engine: engine.as_str(), path: &path, sha: &sha,
            width: w, height: h, bytes: n,
        })?;
        rendered.push(json!({
            "kind": "SCENE", "ref": s.segment_id, "scene_index": s.scene_index,
            "path": path, "sha256": sha, "width": w, "height": h,
            "bytes": n, "engine": engine.as_str(),
            "characters": chars_json,
        }));
    }

    ctx.after_commit()?;
    Ok(json!({
        "chapter_id": chapter_id, "book_id": meta.book_id,
        "chapter_number": meta.chapter_number,
        "engine": engine.as_str(), "style": style.id,
        "rendered": rendered, "reused": reused,
        "assets": existing_assets(&ctx.conn, meta.book_id, meta.chapter_number)?,
    }))
}

// ── deterministic seed + prompt builders (pure, unit-tested) ────────────────

fn sheet_seed(book_id: i64, key: &str, style_id: &str) -> String {
    format!("{book_id}:{key}:{style_id}")
}

fn title_seed(book_id: i64, chapter_number: i64, style_id: &str) -> String {
    format!("{book_id}:title:{chapter_number}:{style_id}")
}

fn scene_seed(book_id: i64, chapter_number: i64, scene_index: usize, segment_id: &str, style_id: &str) -> String {
    format!("{book_id}:{chapter_number}:{scene_index}:{segment_id}:{style_id}")
}

fn sheet_prompt(name: &str, role: &str, bio: &str, style_prompt: &str) -> String {
    // Appearance text rides verbatim on every tier of the ladder (ADR-0011).
    format!(
        "Character reference sheet for {name} ({role}). Appearance (verbatim, authoritative): {bio}. \
         Art style (authoritative): {style_prompt}. Multi-view design sheet, consistent identity across views."
    )
}

#[allow(clippy::too_many_arguments)]
fn scene_prompt(
    scene_index: usize,
    scene_total: usize,
    book_title: &str,
    chapter_title: &str,
    scene_text: &str,
    cast_in_frame: &[&CastEntry],
    style_prompt: &str,
) -> String {
    let cast_lines: String = cast_in_frame
        .iter()
        .map(|c| format!("{} — {}", c.display, c.bio))
        .collect::<Vec<_>>()
        .join("; ");
    let cast_block = if cast_lines.is_empty() {
        "none".to_string()
    } else {
        cast_lines
    };
    format!(
        "Story scene illustration {scene_index} of {scene_total} for the video-novel adaptation of \
         \"{book_title}\" — chapter \"{chapter_title}\". \
         Art style (authoritative): {style_prompt}. \
         Characters in frame (appearance verbatim, authoritative): {cast_block}. \
         Scene text (verbatim): {scene_text} \
         Single frame, 16:9 cinematic composition, no text, no captions, no watermarks."
    )
}

fn scene_label(cast: &[CastEntry], s: &super::derive::SceneRow) -> String {
    let speaker_display = cast
        .iter()
        .find(|c| c.key == s.speaker_id)
        .map(|c| c.display.clone())
        .unwrap_or_else(|| s.speaker_id.clone());
    if s.characters.is_empty() {
        speaker_display
    } else {
        format!("{speaker_display} + {}", s.characters.len())
    }
}

fn rel_of(root: &std::path::Path, p: &std::path::Path) -> String {
    p.strip_prefix(root)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_parse_mirrors_tts_conventions() {
        assert_eq!(Engine::parse("SCRIPTED").unwrap(), Engine::Scripted);
        assert_eq!(Engine::parse("DEVICE").unwrap(), Engine::Scripted);
        assert_eq!(Engine::parse("R2I_ENDPOINT").unwrap(), Engine::Endpoint);
        assert!(Engine::parse("WARP_DRIVE").is_err());
    }

    #[test]
    fn sanitize_key_is_filename_safe() {
        assert_eq!(sanitize_key("char.kira-9"), "CHAR_KIRA_9");
        assert_eq!(sanitize_key("CHAR_KIRA").len(), 9);
    }

    #[test]
    fn sheet_prompt_embeds_bio_and_style_verbatim() {
        let p = sheet_prompt("Kira", "PROTAGONIST", "gray coat, sea-glass eyes", "manga ink, monochrome");
        assert!(p.contains("gray coat, sea-glass eyes"));
        assert!(p.contains("manga ink, monochrome"));
    }

    #[test]
    fn scene_prompt_embeds_text_bios_and_style() {
        let kira = CastEntry {
            key: "KIRA".into(),
            display: "Kira".into(),
            role: "PROTAGONIST".into(),
            bio: "gray coat, sea-glass eyes".into(),
        };
        let p = scene_prompt(
            2, 9, "The Salt Road", "Harbor Lights",
            "Kira watched the tide eat the pier.",
            &[&kira],
            "cinematic film still",
        );
        assert!(p.contains("Kira watched the tide eat the pier."));
        assert!(p.contains("gray coat, sea-glass eyes"));
        assert!(p.contains("cinematic film still"));
        assert!(p.contains("scene illustration 2 of 9"));
        assert!(p.contains("no watermarks"));
        // Empty cast degrades gracefully.
        let p0 = scene_prompt(0, 1, "B", "C", "text", &[], "style");
        assert!(p0.contains("Characters in frame (appearance verbatim, authoritative): none"));
    }

    #[test]
    fn seeds_diverge_by_style_and_coordinates() {
        let a = scene_seed(1, 1, 0, "seg-a", "cinematic");
        let b = scene_seed(1, 1, 0, "seg-a", "pulp");
        let c = scene_seed(1, 1, 1, "seg-a", "cinematic");
        assert_ne!(a, b, "style switch must repaint");
        assert_ne!(a, c, "scene index must diverge");
        assert_eq!(a, scene_seed(1, 1, 0, "seg-a", "cinematic"), "same input → same seed");
        assert_ne!(sheet_seed(1, "KIRA", "cinematic"), sheet_seed(1, "RON", "cinematic"));
        assert_ne!(title_seed(1, 1, "cinematic"), title_seed(1, 2, "cinematic"));
    }
}
