"""AUTHR_OS — DTO schemas, serialization and error-code taxonomy (contract §3/§5).

Contract v1.2: the 9 frozen v1 codes remain untouched; 4 additive codes in
the `audio` and `ai` categories cover the Phase 4/5 studio surface. Phase 6
(publishing & release) reuses the frozen taxonomy — no new codes.
Contract v1.3 (craft & KDP): +1 additive code `CRAFT_ERROR` in the `craft`
category; 6 new commands (§4.14).
Contract v1.4 (retention & serialization): no new codes; 3 new commands
(§4.15) and a `retention` catalog kind. Storage stays additive
(`chapter_retention`).
"""

CANONICAL_CODES = {
    # v1 (frozen)
    "ENTITY_NOT_FOUND",
    "FOREIGN_KEY_VIOLATION",
    "STORAGE_ERROR",
    "LEASE_LOCK_ACTIVE",
    "READ_ONLY_PROJECT",
    "BUNDLE_ERROR",
    "PATH_TRAVERSAL_VIOLATION",
    "ZIP_SLIP_VIOLATION",
    "SECURITY_ERROR",
    # v1.1 additive (Phases 4–5)
    "AUDIO_ERROR",
    "AUDIO_ENGINE_UNAVAILABLE",
    "AI_ERROR",
    "AI_ENGINE_UNAVAILABLE",
    # v1.3 additive (Craft & KDP)
    "CRAFT_ERROR",
}

CATEGORIES = {"storage", "bundle", "security", "audio", "ai", "craft"}

CODE_CATEGORY = {
    "ENTITY_NOT_FOUND": "storage",
    "FOREIGN_KEY_VIOLATION": "storage",
    "STORAGE_ERROR": "storage",
    "LEASE_LOCK_ACTIVE": "bundle",
    "READ_ONLY_PROJECT": "bundle",
    "BUNDLE_ERROR": "bundle",
    "PATH_TRAVERSAL_VIOLATION": "security",
    "ZIP_SLIP_VIOLATION": "security",
    "SECURITY_ERROR": "security",
    "AUDIO_ERROR": "audio",
    "AUDIO_ENGINE_UNAVAILABLE": "audio",
    "AI_ERROR": "ai",
    "AI_ENGINE_UNAVAILABLE": "ai",
    "CRAFT_ERROR": "craft",
}

# Contract §4 command surface (v1.3 — 62 commands).
CONTRACT_COMMANDS = [
    "project_init", "project_open", "project_close", "project_lock_status",
    "project_export", "project_import",
    "book_create", "book_get", "book_list", "book_update", "book_delete",
    "chapter_create", "chapter_get", "chapter_list", "chapter_update", "chapter_delete",
    "character_list", "character_upsert", "character_delete",
    "lore_list", "lore_upsert", "lore_delete",
    "thread_open", "thread_list", "thread_resolve",
    "segment_append", "segment_list", "segment_search",
    "sync_lexical_ast_mutation",
    "keychain_store", "keychain_get", "keychain_delete",
    "git_snapshot", "clean_sweep", "sandbox_normalize",
    # v1.1 — audio studio (Phase 4)
    "audio_cache_stats", "audio_pronounce", "audio_plan",
    "audio_synthesize", "audio_master", "audio_validate",
    # v1.1 — local-first AI & vector RAG (Phase 5)
    "rag_index", "rag_search", "rag_status",
    "local_llm_generate", "local_llm_status", "local_tts_generate",
    "local_ai_status",
    # v1.2 — publishing & release (Phase 6)
    "publish_epub", "publish_docx", "publish_typst", "publish_captions",
    "publish_m4b", "release_manifest", "release_verify", "benchmark_run",
    # v1.3 — craft catalog & storefront packaging
    "craft_catalog", "craft_apply", "craft_scaffold", "craft_progress",
    "kdp_check", "kdp_package",
    # v1.4 — retention & serialization kit (§4.15)
    "craft_retention_check", "craft_retention_report", "craft_retention_worksheet",
]

EXPECTED_COMMAND_COUNT = 65

ROW_FIELDS = {
    "book": ["id", "title", "author", "genre", "premise", "theme", "narrative_structure",
             "target_word_count", "target_chapters", "persona_id", "cover_image_path",
             "cover_prompt", "marketing_copy", "isbn", "readability_score",
             "created_at", "last_modified_at"],
    "chapter": ["id", "book_id", "act_number", "chapter_number", "title", "synopsis",
                "target_word_count", "actual_word_count", "status",
                "created_at", "last_modified_at"],
    "character": ["character_key", "display_name", "role", "bio", "ipa_phonetic",
                  "voice_engine", "voice_id", "base_rate", "base_pitch",
                  "cadence_notes", "created_at"],
    "lore": ["id", "key", "category", "content", "ipa_phonetic",
             "created_at", "last_modified_at"],
    "thread": ["id", "thread_title", "thread_type", "introduced_in_chapter",
               "resolved_in_chapter", "status", "payoff_requirement",
               "created_at", "last_modified_at"],
    "segment": ["segment_id", "chapter_id", "sequence_order", "segment_type", "speaker_id",
                "emotion", "prosody_rate", "prosody_pitch", "text_content", "ambient_cue",
                "sfx_cue", "prev_request_id", "audio_hash", "audio_cache_path",
                "audio_start_ms", "audio_duration_ms", "created_at", "last_modified_at"],
}


def assert_row_shape(row, kind, what="row"):
    assert isinstance(row, dict), f"{what} not a dict"
    for f in ROW_FIELDS[kind]:
        assert f in row, f"{what} of kind {kind} missing field `{f}`"


def assert_envelope_ok(payload):
    assert isinstance(payload, dict)
    assert payload.get("ok") is True
    assert "data" in payload
    assert "error" not in payload


def assert_envelope_error(payload):
    assert isinstance(payload, dict)
    assert payload.get("ok") is False
    err = payload.get("error")
    assert isinstance(err, dict)
    assert set(err.keys()) >= {"code", "message", "category"}
    assert err["code"] in CANONICAL_CODES, f"non-canonical code {err['code']}"
    assert err["category"] in CATEGORIES
    assert err["category"] == CODE_CATEGORY[err["code"]], (
        f"code/category mismatch: {err['code']} vs {err['category']}"
    )
    assert isinstance(err["message"], str) and err["message"]
