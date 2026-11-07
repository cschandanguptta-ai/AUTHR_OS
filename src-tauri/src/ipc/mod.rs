// AUTHR_OS — IPC dispatcher: the single command surface shared by the
// Tauri shell, the CLI binary and the browser dev bridge (docs/CLI_CONTRACT.md).

pub mod args;
pub mod handlers;

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
];

/// Full contract dispatch (§4, 34 commands). `root` is the `--bundle` value.
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

        other => Err(AppError::bundle(format!(
            "unknown command `{other}` — see docs/CLI_CONTRACT.md §4"
        ))),
    }
}
