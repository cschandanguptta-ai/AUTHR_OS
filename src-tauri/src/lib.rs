// AUTHR_OS — library crate root. All subsystems are public so the Tauri
// shell, the CLI and integration tests can drive the same dispatcher.

pub mod audio;
pub mod bundle;
pub mod craft;
pub mod error;
pub mod ipc;
pub mod localai;
pub mod media;
pub mod providers;
pub mod publish;
pub mod rag;
pub mod security;
pub mod shadow;
pub mod storage;
pub mod video;

#[cfg(feature = "gui")]
pub mod shell;

pub const CONTRACT_VERSION: &str = "1.6";
pub const COMMAND_COUNT: usize = 74;

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
    // 4.11 audio studio (Phase 4)
    "audio_cache_stats",
    "audio_pronounce",
    "audio_plan",
    "audio_synthesize",
    "audio_master",
    "audio_validate",
    // 4.12 local-first AI & vector RAG (Phase 5)
    "rag_index",
    "rag_search",
    "rag_status",
    "local_llm_generate",
    "local_llm_status",
    "local_tts_generate",
    "local_ai_status",
    // 4.13 publishing & release (Phase 6)
    "publish_epub",
    "publish_docx",
    "publish_typst",
    "publish_captions",
    "publish_m4b",
    "release_manifest",
    "release_verify",
    "benchmark_run",
    // 4.14 craft catalog & storefront packaging (v1.3)
    "craft_catalog",
    "craft_apply",
    "craft_scaffold",
    "craft_progress",
    "kdp_check",
    "kdp_package",
    // 4.15 retention & serialization kit (v1.4)
    "craft_retention_check",
    "craft_retention_report",
    "craft_retention_worksheet",
    // 4.16 video novel studio (v1.5)
    "video_plan",
    "video_render",
    "video_assemble",
    "video_validate",
    // 4.17 provider settings ladder (v1.6, ADR-0012)
    "provider_get",
    "provider_set",
    "provider_test",
    // 4.18 art style catalog + per-book selection (v1.6, novelcast parity)
    "video_style",
    // 4.19 media read access — bundle-scoped, read-only (v1.6)
    "media_read",
];
