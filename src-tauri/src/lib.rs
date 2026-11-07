// AUTHR_OS — library crate root. All subsystems are public so the Tauri
// shell, the CLI and integration tests can drive the same dispatcher.

pub mod bundle;
pub mod error;
pub mod ipc;
pub mod security;
pub mod shadow;
pub mod storage;

#[cfg(feature = "gui")]
pub mod shell;

pub const CONTRACT_VERSION: &str = "1";
pub const COMMAND_COUNT: usize = 35;

/// The frozen command surface (docs/CLI_CONTRACT.md §4).
pub const COMMANDS: &[&str] = &[
    // 4.1 project lifecycle
    "project_init",
    "project_open",
    "project_close",
    "project_lock_status",
    "project_export",
    "project_import",
    // 4.2 books
    "book_create",
    "book_get",
    "book_list",
    "book_update",
    "book_delete",
    // 4.3 chapters
    "chapter_create",
    "chapter_get",
    "chapter_list",
    "chapter_update",
    "chapter_delete",
    // 4.4 voice cast
    "character_list",
    "character_upsert",
    "character_delete",
    // 4.5 lorebook
    "lore_list",
    "lore_upsert",
    "lore_delete",
    // 4.6 chekhov ledger
    "thread_open",
    "thread_list",
    "thread_resolve",
    // 4.7 segments
    "segment_append",
    "segment_list",
    "segment_search",
    // 4.8 editor bridge
    "sync_lexical_ast_mutation",
    // 4.9 keychain
    "keychain_store",
    "keychain_get",
    "keychain_delete",
    // 4.10 maintenance + sandbox self-test
    "git_snapshot",
    "clean_sweep",
    "sandbox_normalize",
];
