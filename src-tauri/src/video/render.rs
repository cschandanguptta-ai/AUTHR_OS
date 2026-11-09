// AUTHR_OS — Video asset rendering (contract v1.5, ADR-0011).
//
// Engines mirror the audio stack's negotiation pattern exactly:
//   SCRIPTED      — deterministic offline painter (`video::frame`), the
//                   default that always works headless (like scripted TTS)
//   R2I_ENDPOINT  — OpenAI-compatible image endpoint via `AUTHR_IMG_ENDPOINT`
//                   (the novelcast ADR-0002 "custom provider" tier). Fails
//                   CLOSED with the canonical `AI_ENGINE_UNAVAILABLE` error
//                   when the endpoint is not configured — never fakes output.
//
// Assets are registered in `video_assets` (kind: TITLE | SHEET | SCENE; the
// book-level sheet sentinel is chapter_number = 0). Rendering is idempotent:
// unchanged inputs reproduce byte-identical SCRIPTED output, so a re-render
// overwrites in place and the registry row is upserted.
//
// The consistency ladder (novelcast ADR-0004 → ADR-0011): every scene frame
// embeds the appearance text (bio) of its mentioned characters verbatim; the
// endpoint tier sends the same text as the prompt. Reference-image editing
// (multi-ref tier) is the documented future seam.

use super::derive::{ensure_video_schema, scenes_for};
use super::frame;
use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use serde_json::{json, Value};
use std::io::Read as _;
use sha2::{Digest, Sha256};
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

/// §4.16 `video_render` — render title card, character reference sheets and
/// scene frames for one chapter. Idempotent; `sheets_only` restricts to the
/// R2I sheet tier; `scene_index` restricts to one scene; `focal` (map of
/// scene_index → position) updates the persisted Ken Burns anchors.
pub fn video_render(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let chapter_id = args::req_i64(a, "chapter_id")?;
    let engine = Engine::parse(args::get_str(a, "engine").as_deref().unwrap_or("SCRIPTED"))?;
    let sheets_only = a.get("sheets_only").and_then(|v| v.as_bool()).unwrap_or(false);
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

    // Optional focal updates (VideoStudio focal selector persistence).
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
        let seed_b = format!("{}:{}", meta.book_id, c.key);
        let seed = Sha256::digest(seed_b.as_bytes());
        // Sheets live one level up (book scope): media/video/b{book}/char_*.bmp
        let sheet_path = ctx
            .root
            .join("media")
            .join("video")
            .join(format!("b{}", meta.book_id))
            .join(format!("char_{}.bmp", sanitize_key(&c.key)));
        let (rel, sha, w, h, n): (String, String, u32, u32, usize) = match engine {
            Engine::Scripted => {
                let surf = frame::render_sheet(&seed, &c.display, &c.role, &c.bio);
                let bytes = frame::write_bmp(&sheet_path, &surf)?;
                let sha = Sha256::digest(&bytes);
                (rel_of(&ctx.root, &sheet_path), hex(&sha), surf.w, surf.h, bytes.len())
            }
            Engine::Endpoint => {
                let prompt = sheet_prompt(&c.display, &c.role, &c.bio);
                let (bytes, ext) = endpoint_image(&prompt, "1024x1024")?;
                let p = sheet_path.with_extension(ext);
                std::fs::create_dir_all(sheet_path.parent().unwrap_or(p.as_path()))?;
                std::fs::write(&p, &bytes)?;
                let sha = Sha256::digest(&bytes);
                let (w, h) = crate::publish::image_dimensions(&bytes)
                    .ok_or_else(|| AppError::ai_engine_unavailable(
                        "R2I_ENDPOINT returned an image with an unreadable header — refusing to register it",
                    ))?;
                (rel_of(&ctx.root, &p), hex(&sha), w, h, bytes.len())
            }
        };
        register_asset(&ctx.conn, &AssetRecord {
            book_id: meta.book_id, chapter_number: 0, kind: "SHEET", ref_key: &c.key,
            engine: engine.as_str(), path: &rel, sha: &sha, width: w, height: h, bytes: n,
        })?;
        rendered.push(json!({
            "kind": "SHEET", "ref": c.key, "path": rel, "sha256": sha,
            "width": w, "height": h, "bytes": n, "engine": engine.as_str(),
        }));
    }
    if sheets_only {
        ctx.after_commit()?;
        return Ok(json!({
            "chapter_id": chapter_id, "book_id": meta.book_id,
            "chapter_number": meta.chapter_number,
            "engine": engine.as_str(), "rendered": rendered, "reused": 0,
        }));
    }

    // ── Title card ──
    if scene_filter.is_none() {
        let seed = Sha256::digest(format!("{}:title:{}", meta.book_id, meta.chapter_number).as_bytes());
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

    // ── Scene frames ──
    let total = scenes.len();
    for s in &scenes {
        if let Some(want) = scene_filter {
            if s.scene_index as i64 != want {
                continue;
            }
        }
        let seed_b = format!(
            "{}:{}:{}:{}",
            meta.book_id, meta.chapter_number, s.scene_index, s.segment_id
        );
        let seed = Sha256::digest(seed_b.as_bytes());
        let label = scene_label(&cast, s);
        let tag = format!("CH {} · {}", meta.chapter_number, meta.chapter_title);
        let surf = frame::render_scene(&seed, s.scene_index, total, &label, &tag);
        let path = dir.join(format!("scene_{}.bmp", s.scene_index));
        let bytes = frame::write_bmp(&path, &surf)?;
        let sha = hex(&Sha256::digest(&bytes));
        let rel = rel_of(&ctx.root, &path);
        let existed = existing_assets(&ctx.conn, meta.book_id, meta.chapter_number)?
            .iter()
            .any(|x| x["kind"] == "SCENE" && x["ref"] == s.segment_id && x["sha256"] == sha);
        if existed {
            reused += 1;
        }
        register_asset(&ctx.conn, &AssetRecord {
            book_id: meta.book_id, chapter_number: meta.chapter_number, kind: "SCENE",
            ref_key: &s.segment_id, engine: "SCRIPTED", path: &rel, sha: &sha,
            width: surf.w, height: surf.h, bytes: bytes.len(),
        })?;
        rendered.push(json!({
            "kind": "SCENE", "ref": s.segment_id, "scene_index": s.scene_index,
            "path": rel, "sha256": sha, "width": surf.w, "height": surf.h,
            "bytes": bytes.len(), "engine": "SCRIPTED",
            "characters": s.characters,
        }));
    }

    ctx.after_commit()?;
    Ok(json!({
        "chapter_id": chapter_id, "book_id": meta.book_id,
        "chapter_number": meta.chapter_number,
        "engine": engine.as_str(), "rendered": rendered, "reused": reused,
        "assets": existing_assets(&ctx.conn, meta.book_id, meta.chapter_number)?,
    }))
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

fn sheet_prompt(name: &str, role: &str, bio: &str) -> String {
    // Appearance text rides verbatim on every tier of the ladder (ADR-0011).
    format!(
        "Character reference sheet for {name} ({role}). Appearance (verbatim, authoritative): {bio}. Multi-view design sheet, consistent identity across views."
    )
}

/// OpenAI-compatible image endpoint (`AUTHR_IMG_ENDPOINT`): POST
/// `{model?, prompt, size, n:1, response_format:"b64_json"}` → `data[0]`.
/// Fails CLOSED when unset; never fakes a frame.
fn endpoint_image(prompt: &str, size: &str) -> Result<(Vec<u8>, &'static str), AppError> {
    let url = std::env::var("AUTHR_IMG_ENDPOINT").map_err(|_| {
        AppError::ai_engine_unavailable(
            "video image engine `R2I_ENDPOINT` requires AUTHR_IMG_ENDPOINT (OpenAI-compatible \
             images endpoint); offline builds use engine=SCRIPTED",
        )
    })?;
    let model = std::env::var("AUTHR_IMG_MODEL").unwrap_or_default();
    let mut body = json!({
        "prompt": prompt,
        "size": size,
        "n": 1,
        "response_format": "b64_json",
    });
    if !model.is_empty() {
        body["model"] = json!(model);
    }
    let resp = ureq::post(&url)
        .timeout(std::time::Duration::from_secs(120))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| AppError::ai_engine_unavailable(format!("image endpoint {url}: {e}")))?;
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::ai_engine_unavailable(format!("image endpoint read: {e}")))?;
    let v: Value = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::ai_engine_unavailable(format!("image endpoint json: {e}")))?;
    let b64 = v["data"][0]["b64_json"]
        .as_str()
        .ok_or_else(|| AppError::ai_engine_unavailable("image endpoint returned no data[0].b64_json"))?;
    use base64_decode::decode_b64;
    let raw = decode_b64(b64).ok_or_else(|| {
        AppError::ai_engine_unavailable("image endpoint b64_json failed to decode")
    })?;
    // Sniff the container: PNG or JPEG only (validated again by the caller).
    let ext = if raw.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if raw.starts_with(&[0xFF, 0xD8]) {
        "jpg"
    } else {
        return Err(AppError::ai_engine_unavailable(
            "image endpoint returned neither PNG nor JPEG — refusing to register it",
        ));
    };
    Ok((raw, ext))
}

/// Minimal base64 decoder (std-only; the runtime carries no base64 crate).
mod base64_decode {
    pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut table = [255u8; 256];
        for (i, c) in T.iter().enumerate() {
            table[*c as usize] = i as u8;
        }
        let mut out = Vec::with_capacity(s.len() / 4 * 3);
        let mut acc: u32 = 0;
        let mut bits = 0u32;
        for ch in s.bytes() {
            if ch == b'=' || ch == b'\n' || ch == b'\r' || ch == b' ' {
                continue;
            }
            let v = table[ch as usize];
            if v == 255 {
                return None;
            }
            acc = (acc << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        Some(out)
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
    fn base64_decoder_handles_standard_and_padded_input() {
        let d = super::base64_decode::decode_b64;
        assert_eq!(d("TWFpbg==").unwrap(), b"Main".to_vec());
        assert_eq!(d("TQ==").unwrap(), b"M".to_vec());
        assert_eq!(d("TW Fp\nbg").unwrap(), b"Main".to_vec());
        assert!(d("!!!!").is_none());
    }

    #[test]
    fn sanitize_key_is_filename_safe() {
        assert_eq!(sanitize_key("char.kira-9"), "CHAR_KIRA_9");
        assert_eq!(sanitize_key("CHAR_KIRA").len(), 9);
    }

    #[test]
    fn sheet_prompt_embeds_bio_verbatim() {
        let p = sheet_prompt("Kira", "PROTAGONIST", "gray coat, sea-glass eyes");
        assert!(p.contains("gray coat, sea-glass eyes"));
    }
}
