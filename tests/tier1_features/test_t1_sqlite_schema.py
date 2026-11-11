"""Tier 1 — F7: 6-table relational schema creation & DDL constraints.

Test IDs T1.SC1–T1.SC6 (6 tests).
"""

import sqlite3

from helpers.assertions import assert_tables, assert_columns, connect
from helpers import schema_reference as sr


def test_sc1_all_six_tables_created(bundle):
    """T1.SC1 — books/chapters/character_profiles/lorebook/unresolved_threads/chapter_segments."""
    names = assert_tables(bundle().db_path)
    assert "chapter_segments_fts" in names


def test_sc2_books_table_columns(bundle):
    """T1.SC2 — books table has the full projected column set."""
    assert_columns(bundle().db_path, "books")


def test_sc3_chapters_cascade_fk_and_unique(bundle):
    """T1.SC3 — chapters FK is ON DELETE CASCADE and (book_id, chapter_number) unique."""
    b = bundle()
    conn = connect(b.db_path)
    try:
        sql = conn.execute(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='chapters'"
        ).fetchone()["sql"]
    finally:
        conn.close()
    assert "REFERENCES books(id) ON DELETE CASCADE" in sql
    assert "UNIQUE(book_id, chapter_number)" in sql


def test_sc4_fts5_virtual_table_porter(bundle):
    """T1.SC4 — chapter_segments_fts uses fts5 with the porter unicode61 tokenizer."""
    b = bundle()
    conn = connect(b.db_path)
    try:
        row = conn.execute(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='chapter_segments_fts'"
        ).fetchone()
        assert row is not None, "chapter_segments_fts missing"
        sql = row["sql"]
    finally:
        conn.close()
    assert "fts5" in sql.lower()
    assert sr.FTS_TOKENIZER in sql
    assert "segment_id UNINDEXED" in sql


def test_sc5_three_sync_triggers(bundle):
    """T1.SC5 — trg_segments_ai / trg_segments_ad / trg_segments_au exist verbatim."""
    from helpers.assertions import assert_triggers

    assert_triggers(bundle().db_path)


def test_sc6_primary_keys_and_defaults(bundle):
    """T1.SC6 — PKs on every table + default values honored (NARRATOR seeded, defaults)."""
    b = bundle()
    conn = connect(b.db_path)
    try:
        # NARRATOR seeded at schema creation.
        row = conn.execute(
            "SELECT character_key, role, voice_engine FROM character_profiles "
            "WHERE character_key='NARRATOR'"
        ).fetchone()
        assert row is not None, "NARRATOR not seeded"
        assert row["role"] == "NARRATOR"
        assert row["voice_engine"] == "KOKORO_LOCAL"

        # Defaults honored on a books insert (title only).
        conn.execute("INSERT INTO books(title) VALUES ('Default Probe')")
        row = conn.execute(
            "SELECT author, genre, narrative_structure, target_word_count "
            "FROM books WHERE title='Default Probe'"
        ).fetchone()
        assert row["author"] == "Anonymous"
        assert row["genre"] == "Sci-Fi"
        assert row["narrative_structure"] == "SAVE_THE_CAT"
        assert row["target_word_count"] == 80000

        # PK map.
        pks = {}
        for t in sr.EXPECTED_TABLES:
            info = conn.execute(f"PRAGMA table_info({t})").fetchall()
            pks[t] = [r["name"] for r in info if r["pk"]]
        assert pks["books"] == ["id"]
        assert pks["chapters"] == ["id"]
        assert pks["character_profiles"] == ["character_key"]
        assert pks["lorebook"] == ["id"]
        assert pks["unresolved_threads"] == ["id"]
        assert pks["chapter_segments"] == ["segment_id"]
    finally:
        conn.close()
