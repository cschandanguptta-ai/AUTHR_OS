"""AUTHR_OS — DTO schemas, serialization and error-code taxonomy (contract §3/§5)."""

CANONICAL_CODES = {
    "ENTITY_NOT_FOUND",
    "FOREIGN_KEY_VIOLATION",
    "STORAGE_ERROR",
    "LEASE_LOCK_ACTIVE",
    "READ_ONLY_PROJECT",
    "BUNDLE_ERROR",
    "PATH_TRAVERSAL_VIOLATION",
    "ZIP_SLIP_VIOLATION",
    "SECURITY_ERROR",
}

CATEGORIES = {"storage", "bundle", "security"}

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
}

# Contract §4 command surface (34 commands).
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
]

EXPECTED_COMMAND_COUNT = 35

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
