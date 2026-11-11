// AUTHR_OS — Art style catalog & per-book selection (contract v1.6, §4.18).
//
// novelcast parity (novelcast `lib/art-styles.ts`): one shared style catalog
// injected into every image prompt — scenes AND character reference sheets —
// so a whole book shares one coherent look on every tier of the ladder:
//
//   SCRIPTED      — the style id joins the SHA-256 seed material, so a style
//                   switch deterministically repaints (byte-identical for a
//                   given style, different palettes between styles)
//   R2I_ENDPOINT  — the style prompt fragment rides verbatim in the payload
//
// Selection persists per book in `provider_settings` (section `art`,
// key `book:<id>`) — additive storage, same generic table the provider ladder
// uses; no frozen DDL touched. `video_style` without `style_id` is a read.

use crate::error::AppError;
use crate::ipc::args;
use crate::ipc::Ctx;
use rusqlite::Connection;
use serde_json::{json, Value};

pub const DEFAULT_STYLE_ID: &str = "cinematic";

pub struct ArtStyleDef {
    pub id: &'static str,
    pub label: &'static str,
    pub tagline: &'static str,
    /// Prompt fragment injected into image-generation prompts.
    pub prompt: &'static str,
}

/// The shared catalog — mirrored from novelcast `lib/art-styles.ts` so books
/// move between the two runtimes without changing look.
pub const ART_STYLES: &[ArtStyleDef] = &[
    ArtStyleDef {
        id: "cinematic",
        label: "Cinematic Realism",
        tagline: "Film-still look with dramatic natural light",
        prompt: "cinematic film still, photorealistic, shallow depth of field, dramatic natural lighting, rich color grading, ultra detailed",
    },
    ArtStyleDef {
        id: "storybook",
        label: "Watercolor Storybook",
        tagline: "Soft hand-painted warmth, gentle edges",
        prompt: "soft watercolor storybook illustration, hand-painted paper texture, gentle pastel palette, warm golden light, delicate ink outlines",
    },
    ArtStyleDef {
        id: "manhwa",
        label: "Modern Manhwa",
        tagline: "Glossy full-color webtoon, dynamic posing",
        prompt: "modern korean manhwa webtoon art, clean digital cel shading, glossy highlights, sharp confident lineart, vibrant saturated colors, dramatic composition",
    },
    ArtStyleDef {
        id: "anime",
        label: "Anime Film",
        tagline: "Hand-painted anime frames, nostalgic glow",
        prompt: "hand-painted anime film style, painterly gouache background art, expressive characters, soft nostalgic lighting, detailed clouds and foliage",
    },
    ArtStyleDef {
        id: "manga-ink",
        label: "Manga Ink",
        tagline: "Monochrome ink lines, screentone drama",
        prompt: "black and white manga ink illustration, bold confident linework, crosshatching and screentone shading, high contrast monochrome",
    },
    ArtStyleDef {
        id: "oil",
        label: "Classical Oil Painting",
        tagline: "Museum-grade brushwork and chiaroscuro",
        prompt: "classical oil painting, visible expressive brushstrokes, chiaroscuro lighting, rich earthy pigments, canvas texture, fine-art quality",
    },
    ArtStyleDef {
        id: "comic",
        label: "Western Comic Book",
        tagline: "Bold inks and halftone print dots",
        prompt: "western comic book art, bold ink outlines, halftone dot shading, saturated flat colors, dynamic poses, retro print texture",
    },
    ArtStyleDef {
        id: "pixar",
        label: "3D Animation",
        tagline: "Stylized CGI feature-film charm",
        prompt: "stylized 3D animated feature film still, soft global illumination, appealing stylized characters, subsurface scattering, vibrant production design",
    },
    ArtStyleDef {
        id: "dark-fantasy",
        label: "Dark Fantasy",
        tagline: "Moody concept art, epic and ominous",
        prompt: "dark fantasy concept art, moody atmospheric fog, epic scale, ornate detail, desaturated palette with ember accents, painterly digital art",
    },
    ArtStyleDef {
        id: "pulp",
        label: "Retro Pulp Adventure",
        tagline: "Vintage paperback-cover energy",
        prompt: "vintage pulp adventure paperback cover art, 1950s gouache illustration, dramatic heroic poses, aged paper grain, saturated retro palette",
    },
];

fn find_style(id: &str) -> Option<&'static ArtStyleDef> {
    ART_STYLES.iter().find(|s| s.id == id)
}

fn style_key(book_id: i64) -> String {
    format!("book:{book_id}")
}

/// Resolved style for a book — validated catalog id or the default. Never
/// fails for reads; unknown stored ids fall back to the default (catalog
/// renames must not strand old bundles).
pub fn style_for(conn: &Connection, book_id: i64) -> Result<&'static ArtStyleDef, AppError> {
    ensure_provider_art(conn)?;
    let stored = conn
        .query_row(
            "SELECT value FROM provider_settings WHERE section = 'art' AND key = ?1",
            [style_key(book_id)],
            |r| r.get::<_, String>(0),
        )
        .ok();
    Ok(find_style(stored.as_deref().unwrap_or(DEFAULT_STYLE_ID))
        .unwrap_or_else(|| find_style(DEFAULT_STYLE_ID).expect("default style exists")))
}

fn ensure_provider_art(conn: &Connection) -> Result<(), AppError> {
    // provider_settings is created idempotently by the provider ladder; the
    // art section rides the same table so both entry points can run first.
    crate::providers::ensure_provider_schema(conn)
}

/// §4.18 `video_style` — read the resolved style (and catalog) for a book;
/// with `style_id`, validate against the catalog and persist the selection.
pub fn video_style(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let book_id = args::req_i64(a, "book_id")?;
    {
        let exists: i64 = ctx.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE id = ?1",
            [book_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::not_found(format!("book {book_id} does not exist")));
        }
    }
    ensure_provider_art(&ctx.conn)?;

    if let Some(style_id) = args::get_str(a, "style_id") {
        let def = find_style(&style_id).ok_or_else(|| {
            AppError::craft(format!(
                "unknown art style `{style_id}` — pick one of the catalog ids (video_style returns them)"
            ))
        })?;
        ctx.conn.execute(
            "INSERT INTO provider_settings (section, key, value, updated_at)
             VALUES ('art', ?1, ?2, datetime('now'))
             ON CONFLICT (section, key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
            rusqlite::params![style_key(book_id), def.id],
        )?;
        ctx.after_commit()?;
    }

    let current = style_for(&ctx.conn, book_id)?;
    let styles: Vec<Value> = ART_STYLES
        .iter()
        .map(|s| json!({ "id": s.id, "label": s.label, "tagline": s.tagline, "prompt": s.prompt }))
        .collect();
    Ok(json!({
        "book_id": book_id,
        "style_id": current.id,
        "label": current.label,
        "prompt": current.prompt,
        "styles": styles,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_default_resolves() {
        let mut seen = Vec::new();
        for s in ART_STYLES {
            assert!(!seen.contains(&s.id), "duplicate style id {}", s.id);
            seen.push(s.id);
            assert!(!s.prompt.is_empty());
        }
        assert!(find_style(DEFAULT_STYLE_ID).is_some());
    }

    #[test]
    fn unknown_stored_style_falls_back_to_default() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::providers::ensure_provider_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO provider_settings (section, key, value, updated_at)
             VALUES ('art', 'book:1', 'warp-drive', datetime('now'))",
            [],
        )
        .unwrap();
        assert_eq!(style_for(&conn, 1).unwrap().id, DEFAULT_STYLE_ID);
        conn.execute(
            "INSERT INTO provider_settings (section, key, value, updated_at)
             VALUES ('art', 'book:2', 'manga-ink', datetime('now'))",
            [],
        )
        .unwrap();
        assert_eq!(style_for(&conn, 2).unwrap().id, "manga-ink");
    }
}
