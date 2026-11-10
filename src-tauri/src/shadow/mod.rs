//! Shadow Markdown & companion AST JSON layer (Feature F14).
//!
//! Every chapter is auto-rendered into a human-readable `manuscript/chXX.md`
//! (YAML frontmatter + verbatim narration + `**SPEAKER**: "…"` dialogue +
//! `*…*` internal monologue) and a machine-exact `manuscript/chXX.ast.json`
//! companion array. Writes are atomic (`.tmp` staged, renamed).

use crate::error::AppError;
use crate::models::ast::{NarrativeSegmentAst, SegmentType};
use crate::storage::{books, chapters, segments};
use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct Frontmatter<'a> {
    chapter_index: i64,
    chapter_title: &'a str,
    book: &'a str,
    author: &'a str,
    status: &'a str,
    generated_at: String,
}

pub fn shadow_dir(root: &Path) -> PathBuf {
    root.join("manuscript")
}

pub fn markdown_path(root: &Path, chapter_index: i64) -> PathBuf {
    shadow_dir(root).join(format!("ch{chapter_index:02}.md"))
}

pub fn ast_json_path(root: &Path, chapter_index: i64) -> PathBuf {
    shadow_dir(root).join(format!("ch{chapter_index:02}.ast.json"))
}

/// Render one chapter's shadow Markdown + AST JSON pair.
pub fn render_chapter(
    conn: &Connection,
    root: &Path,
    chapter_id: i64,
) -> Result<serde_json::Value, AppError> {
    let chapter = chapters::get(conn, chapter_id)?;
    let book = books::get(conn, chapter.book_id)?;
    let records = segments::list(conn, chapter_id)?;
    let asts: Vec<NarrativeSegmentAst> = records.iter().map(|r| r.ast.clone()).collect();

    let frontmatter = Frontmatter {
        chapter_index: chapter.chapter_index,
        chapter_title: &chapter.title,
        book: &book.title,
        author: &book.author,
        status: &chapter.status,
        generated_at: crate::bundle::lock::now_iso(),
    };
    let yaml = serde_yaml::to_string(&frontmatter)?;

    let mut body = String::new();
    for seg in &asts {
        match seg.segment_type {
            SegmentType::Narration => body.push_str(&seg.text_content),
            SegmentType::Dialogue => body.push_str(&format!(
                "**{}**: \"{}\"",
                seg.speaker_id,
                seg.text_content.replace('"', "\\\"")
            )),
            SegmentType::InternalMonologue => {
                body.push_str(&format!("*{}*", seg.text_content))
            }
        }
        body.push_str("\n\n");
    }

    let markdown = format!(
        "---\n{}---\n\n# Chapter {}: {}\n\n{}",
        yaml, chapter.chapter_index, chapter.title, body
    );
    crate::util::atomic_write(&markdown_path(root, chapter.chapter_index), markdown.as_bytes())?;

    let ast_json = serde_json::to_vec_pretty(&asts)?;
    crate::util::atomic_write(&ast_json_path(root, chapter.chapter_index), &ast_json)?;

    Ok(serde_json::json!({
        "chapter_id": chapter_id,
        "chapter_index": chapter.chapter_index,
        "markdown": markdown_path(root, chapter.chapter_index).display().to_string(),
        "ast_json": ast_json_path(root, chapter.chapter_index).display().to_string(),
        "segments": asts.len(),
    }))
}

/// Render every chapter; prune shadow files for chapters that no longer exist.
pub fn render_all(conn: &Connection, root: &Path) -> Result<u32, AppError> {
    let mut rendered = 0u32;
    let mut stmt = conn.prepare("SELECT id FROM chapters")?;
    let ids: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for id in ids {
        render_chapter(conn, root, id)?;
        rendered += 1;
    }
    prune_orphans(conn, root)?;
    Ok(rendered)
}

fn prune_orphans(conn: &Connection, root: &Path) -> Result<(), AppError> {
    let mut stmt = conn.prepare("SELECT chapter_index FROM chapters")?;
    let live: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let dir = shadow_dir(root);
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let index: Option<i64> = if let Some(stem) = name.strip_suffix(".ast.json") {
            stem.strip_prefix("ch").and_then(|s| s.parse().ok())
        } else if let Some(stem) = name.strip_suffix(".md") {
            stem.strip_prefix("ch").and_then(|s| s.parse().ok())
        } else {
            None
        };
        if let Some(idx) = index {
            if !live.contains(&idx) && !name.ends_with(".tmp") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

/// Remove both shadow files of a chapter (used before/after DB delete).
pub fn remove_chapter_files(root: &Path, chapter_index: i64) -> Result<(), AppError> {
    let _ = std::fs::remove_file(markdown_path(root, chapter_index));
    let _ = std::fs::remove_file(ast_json_path(root, chapter_index));
    Ok(())
}
