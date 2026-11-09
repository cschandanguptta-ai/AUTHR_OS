// AUTHR_OS — IPC dispatcher: the single command surface shared by the
// Tauri shell, the CLI binary and the browser dev bridge (docs/CLI_CONTRACT.md).

pub mod args;
pub mod handlers;
pub mod studio;

use crate::bundle::lock;
use crate::bundle::manifest::Manifest;
use crate::error::{AppError, IpcResult};
use rusqlite::Connection;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Per-invocation execution context.
pub struct Ctx {
    pub root: PathBuf,
    pub conn: Connection,
    pub readonly: bool,
    pub cloud_guard: bool,
}

impl Ctx {
    /// Validate the bundle, enforce write access if required, open project.db.
    pub fn open(root: &Path, needs_write: bool) -> Result<Ctx, AppError> {
        Manifest::read(root)?;
        let readonly = matches!(lock::classify(root)?, lock::LockState::ForeignActive(_));
        if needs_write {
            lock::ensure_writable(root)?;
        }
        let conn = crate::storage::db::open_db(root)?;
        Ok(Ctx {
            root: root.to_path_buf(),
            conn,
            readonly,
            cloud_guard: crate::security::sandbox::is_cloud_sync_path(root),
        })
    }

    /// Bookkeeping after a write commit: manifest timestamp, then the
    /// cloud-sync guard checkpoint (WAL → 0 bytes). Lesson from Phase 1:
    /// manifest touch happens BEFORE the shadow/git sync, never after.
    pub fn after_commit(&self) -> Result<(), AppError> {
        Manifest::touch(&self.root)?;
        if self.cloud_guard {
            crate::storage::db::wal_truncate(&self.conn)?;
        }
        Ok(())
    }
}

/// Commands that mutate bundle state (enforce lease writability).
pub const WRITER_COMMANDS: &[&str] = &[
    "book_create",
    "book_update",
    "book_delete",
    "chapter_create",
    "chapter_update",
    "chapter_delete",
    "character_upsert",
    "character_delete",
    "lore_upsert",
    "lore_delete",
    "thread_open",
    "thread_resolve",
    "segment_append",
    "sync_lexical_ast_mutation",
    "git_snapshot",
    "clean_sweep",
    "audio_synthesize",
    "audio_master",
    "rag_index",
    "publish_epub",
    "publish_docx",
    "publish_typst",
    "publish_captions",
    "publish_m4b",
    "craft_apply",
    "craft_scaffold",
    "kdp_package",
    "craft_retention_check",
    "video_render",
    "video_assemble",
];

/// Full contract dispatch (§4, 69 commands). `root` is the `--bundle` value.
pub fn dispatch(root: Option<&Path>, command: &str, args: &Value) -> IpcResult {
    // ── Bundle-free commands ────────────────────────────────────────────────
    match command {
        "project_init" => return crate::bundle::lifecycle::project_init(args),
        "project_import" => {
            let zip = args::req_str(args, "zip")?;
            let dest = args::req_str(args, "dest")?;
            return crate::bundle::project_import(&zip, &dest);
        }
        "keychain_store" => return handlers::keychain_store(args),
        "keychain_get" => return handlers::keychain_get(args),
        "keychain_delete" => return handlers::keychain_delete(args),
        "sandbox_normalize" => return handlers::sandbox_normalize(args),
        "release_manifest" => return crate::publish::release::release_manifest(args),
        "release_verify" => return crate::publish::release::release_verify(args),
        "benchmark_run" => return crate::publish::bench::benchmark_run(args),
        "craft_catalog" => {
            let kind = args::get_str(args, "kind").unwrap_or_else(|| "all".into());
            return crate::craft::catalog::catalog_json(&kind).map_err(crate::error::AppError::craft);
        }
        _ => {}
    }

    // ── project_open resolves the bundle from args.path or --bundle ────────
    if command == "project_open" {
        return crate::bundle::lifecycle::project_open(args, root);
    }

    // ── Every remaining command is bundle-scoped ───────────────────────────
    let root = root.ok_or_else(|| {
        AppError::bundle(format!("command `{command}` requires --bundle <BUNDLE_DIR>"))
    })?;
    let needs_write = WRITER_COMMANDS.contains(&command);
    let mut ctx = Ctx::open(root, needs_write)?;

    match command {
        "project_close" => crate::bundle::lifecycle::project_close(&ctx.root, &ctx.conn),
        "project_lock_status" => crate::bundle::lifecycle::project_lock_status(&ctx.root),
        "project_export" => {
            let out = args::req_str(args, "out")?;
            crate::bundle::project_export(&ctx.root, &out)
        }

        "book_create" => crate::storage::books::create(&ctx.conn, args),
        "book_get" => {
            let id = args::req_i64(args, "id")?;
            crate::storage::books::get(&ctx.conn, id)
        }
        "book_list" => crate::storage::books::list(&ctx.conn),
        "book_update" => crate::storage::books::update(&ctx.conn, args),
        "book_delete" => {
            let id = args::req_i64(args, "id")?;
            crate::storage::books::delete(&ctx.conn, id)
        }

        "chapter_create" => crate::storage::chapters::create(&mut ctx, args),
        "chapter_get" => {
            let id = args::req_i64(args, "id")?;
            crate::storage::chapters::get(&ctx.conn, id)
        }
        "chapter_list" => crate::storage::chapters::list(&ctx.conn, args::get_i64(args, "book_id")),
        "chapter_update" => crate::storage::chapters::update(&mut ctx, args),
        "chapter_delete" => {
            let id = args::req_i64(args, "id")?;
            crate::storage::chapters::delete(&mut ctx, id)
        }

        "character_list" => crate::storage::characters::list(&ctx.conn),
        "character_upsert" => crate::storage::characters::upsert(&mut ctx, args),
        "character_delete" => {
            let key = args::req_str(args, "character_key")?;
            crate::storage::characters::delete(&ctx.conn, &key)
        }

        "lore_list" => crate::storage::lorebook::list(&ctx.conn),
        "lore_upsert" => crate::storage::lorebook::upsert(&ctx.conn, args),
        "lore_delete" => {
            let key = args::req_str(args, "key")?;
            crate::storage::lorebook::delete(&ctx.conn, &key)
        }

        "thread_open" => crate::storage::threads::open(&ctx.conn, args),
        "thread_list" => crate::storage::threads::list(&ctx.conn, args::get_str(args, "status").as_deref()),
        "thread_resolve" => crate::storage::threads::resolve(&ctx.conn, args),

        "segment_append" => crate::storage::segments::append(&mut ctx, args),
        "segment_list" => {
            let chapter_id = args::req_i64(args, "chapter_id")?;
            crate::storage::segments::list(&ctx.conn, chapter_id)
        }
        "segment_search" => {
            let query = args::req_str(args, "query")?;
            let limit = args::get_i64(args, "limit").unwrap_or(20);
            crate::storage::segments::search(&ctx.conn, &query, limit)
        }

        "sync_lexical_ast_mutation" => crate::storage::segments::apply_mutation(&mut ctx, args),

        "git_snapshot" => {
            let msg = args::get_str(args, "message");
            crate::storage::git::manual_snapshot(&ctx, msg.as_deref())
                .map(|oid| serde_json::json!({ "commit": oid }))
        }
        "clean_sweep" => crate::storage::cleaner::sweep(&ctx.conn, &ctx.root),

        // ── 4.11 audio studio (Phase 4) ───────────────────────────────────
        "audio_cache_stats" => crate::ipc::studio::audio_cache_stats(&ctx, args),
        "audio_pronounce" => crate::ipc::studio::audio_pronounce(&ctx, args),
        "audio_plan" => crate::ipc::studio::audio_plan(&ctx, args),
        "audio_synthesize" => crate::ipc::studio::audio_synthesize(&mut ctx, args),
        "audio_master" => crate::ipc::studio::audio_master(&mut ctx, args),
        "audio_validate" => crate::ipc::studio::audio_validate(&ctx, args),

        // ── 4.12 local-first AI & vector RAG (Phase 5) ────────────────────
        "rag_index" => crate::ipc::studio::rag_index(&mut ctx, args),
        "rag_search" => crate::ipc::studio::rag_search(&ctx, args),
        "rag_status" => crate::ipc::studio::rag_status(&ctx, args),
        "local_llm_generate" => crate::ipc::studio::local_llm_generate(&ctx, args),
        "local_llm_status" => crate::ipc::studio::local_llm_status(&ctx, args),
        "local_tts_generate" => crate::ipc::studio::local_tts_generate(&mut ctx, args),
        "local_ai_status" => crate::ipc::studio::local_ai_status(&ctx, args),

        // ── 4.13 publishing & release (Phase 6) ──────────────────────────
        "publish_epub" => crate::publish::epub::publish_epub(&mut ctx, args),
        "publish_docx" => crate::publish::docx::publish_docx(&mut ctx, args),
        "publish_typst" => crate::publish::typst::publish_typst(&mut ctx, args),
        "publish_captions" => crate::publish::captions::publish_captions(&mut ctx, args),
        "publish_m4b" => crate::publish::m4b::publish_m4b(&mut ctx, args),

        // ── 4.14 craft catalog & storefront packaging (v1.3) ─────────
        "craft_apply" => crate::craft::engine::craft_apply(&mut ctx, args),
        "craft_scaffold" => crate::craft::engine::craft_scaffold(&mut ctx, args),
        "craft_progress" => crate::craft::engine::craft_progress(&mut ctx, args),
        "kdp_check" => crate::craft::kdp::kdp_check(&mut ctx, args),
        "kdp_package" => crate::craft::kdp::kdp_package(&mut ctx, args),

        // ── 4.15 retention & serialization kit (v1.4) ────────────────
        "craft_retention_check" => crate::craft::retention::craft_retention_check(&mut ctx, args),
        "craft_retention_report" => crate::craft::retention::craft_retention_report(&mut ctx, args),
        "craft_retention_worksheet" => crate::craft::retention::craft_retention_worksheet(&mut ctx, args),

        // ── 4.16 video novel studio (v1.5) ─────────────────────────
        "video_plan" => crate::video::plan::video_plan(&ctx, args),
        "video_render" => crate::video::render::video_render(&mut ctx, args),
        "video_assemble" => crate::video::assemble::video_assemble(&mut ctx, args),
        "video_validate" => crate::video::validate::video_validate(&ctx, args),

        other => Err(AppError::bundle(format!(
            "unknown command `{other}` — see docs/CLI_CONTRACT.md §4"
        ))),
    }
}
