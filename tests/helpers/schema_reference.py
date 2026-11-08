"""AUTHR_OS — verbatim schema reference (build plan §6.1 / TEST_INFRA §5.1).

The E2E suite validates the produced `project.db` against these constants.
"""

EXPECTED_TABLES = [
    "books",
    "chapters",
    "character_profiles",
    "lorebook",
    "unresolved_threads",
    "chapter_segments",
]

EXPECTED_INDEXES = [
    "idx_chapters_book_order",
    "idx_segments_chapter_order",
    "idx_segments_audio_hash",
    "idx_threads_status",
]

EXPECTED_TRIGGERS = [
    "trg_segments_ai",
    "trg_segments_ad",
    "trg_segments_au",
]

FTS_TABLE = "chapter_segments_fts"
FTS_TOKENIZER = "porter unicode61"

TABLE_COLUMNS = {
    "books": [
        "id", "title", "author", "genre", "premise", "theme", "narrative_structure",
        "target_word_count", "target_chapters", "persona_id", "cover_image_path",
        "cover_prompt", "marketing_copy", "isbn", "readability_score",
        "created_at", "last_modified_at",
    ],
    "chapters": [
        "id", "book_id", "act_number", "chapter_number", "title", "synopsis",
        "target_word_count", "actual_word_count", "status",
        "created_at", "last_modified_at",
    ],
    "character_profiles": [
        "character_key", "display_name", "role", "bio", "ipa_phonetic", "voice_engine",
        "voice_id", "base_rate", "base_pitch", "cadence_notes", "created_at",
    ],
    "lorebook": [
        "id", "key", "category", "content", "ipa_phonetic",
        "created_at", "last_modified_at",
    ],
    "unresolved_threads": [
        "id", "thread_title", "thread_type", "introduced_in_chapter",
        "resolved_in_chapter", "status", "payoff_requirement",
        "created_at", "last_modified_at",
    ],
    "chapter_segments": [
        "segment_id", "chapter_id", "sequence_order", "segment_type", "speaker_id",
        "emotion", "prosody_rate", "prosody_pitch", "text_content", "ambient_cue",
        "sfx_cue", "prev_request_id", "audio_hash", "audio_cache_path",
        "audio_start_ms", "audio_duration_ms", "created_at", "last_modified_at",
    ],
}

# Frozen CHECK enumerations (§6.1 DDL).
ENUMS = {
    "books.narrative_structure": ["SAVE_THE_CAT", "THREE_ACT", "FICHTEAN_CURVE", "HEROS_JOURNEY"],
    "chapters.status": ["PLANNED", "DRAFTING", "POLISHED", "AUDIO_READY", "PUBLISHED"],
    "character_profiles.role": ["PROTAGONIST", "ANTAGONIST", "SUPPORTING", "NARRATOR"],
    "character_profiles.voice_engine": [
        "KOKORO_LOCAL", "EDGE_TTS", "ELEVEN_LABS", "CARTESIA", "OPENAI_TTS",
    ],
    "lorebook.category": ["GENERAL", "LOCATION", "FACTION", "MAGIC_TECH", "HISTORICAL"],
    "unresolved_threads.thread_type": ["CHEKHOV_GUN", "MYSTERY", "SUBPLOT", "PROMISE"],
    "unresolved_threads.status": ["INTRODUCED", "ESCALATED", "RESOLVED"],
    "chapter_segments.segment_type": ["NARRATION", "DIALOGUE", "INTERNAL_MONOLOGUE"],
}

# FK edges validated by tier 2.
FOREIGN_KEYS = {
    "chapters.book_id": ("books", "id", "CASCADE"),
    "chapter_segments.chapter_id": ("chapters", "id", "CASCADE"),
    "chapter_segments.speaker_id": ("character_profiles", "character_key", None),  # ON UPDATE CASCADE
    "unresolved_threads.introduced_in_chapter": ("chapters", "id", "CASCADE"),
    "unresolved_threads.resolved_in_chapter": ("chapters", "id", "SET NULL"),
}

PRAGMAS = {
    "foreign_keys": 1,
    "journal_mode": "wal",
    "synchronous": 1,  # NORMAL
}

# Verbatim DDL excerpt kept for byte-comparison of trigger bodies.
TRIGGER_SQL = {
    "trg_segments_ai": "AFTER INSERT ON chapter_segments",
    "trg_segments_ad": "AFTER DELETE ON chapter_segments",
    "trg_segments_au": "AFTER UPDATE OF text_content ON chapter_segments",
}

MANIFEST_FIELDS = [
    "schema_version", "project_id", "title", "author",
    "created_at", "last_modified_at", "authr_version",
]

LOCK_FIELDS = [
    "pid", "hostname", "user", "locked_at", "lease_expires_at", "session_id",
]

LEASE_TTL_SECONDS = 120
GITIGNORE_LINES = ["project.db", "project.db-wal", "project.db-shm", ".authr_lock", "audio/", "exports/"]
