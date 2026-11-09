"""Tier 1 — F8: 4 secondary B-tree indexes & query-plan verification.

Test IDs T1.IX1–T1.IX5 (5 tests).
"""

from helpers.assertions import assert_indexes, connect


def _index_names(db_path):
    conn = connect(db_path)
    try:
        return {r["name"] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='index'")}
    finally:
        conn.close()


def test_ix1_idx_chapters_book_order(bundle):
    assert "idx_chapters_book_order" in _index_names(bundle().db_path)


def test_ix2_idx_segments_chapter_order(bundle):
    assert "idx_segments_chapter_order" in _index_names(bundle().db_path)


def test_ix3_idx_segments_audio_hash(bundle):
    assert "idx_segments_audio_hash" in _index_names(bundle().db_path)


def test_ix4_idx_threads_status(bundle):
    assert "idx_threads_status" in _index_names(bundle().db_path)


def test_ix5_query_plans_use_indexes(seeded_bundle):
    """T1.IX5 — EXPLAIN QUERY PLAN verifies index usage for the hot paths."""
    b = seeded_bundle
    plans = {}
    conn = b.db()
    try:
        for label, sql in [
            ("book_order", "SELECT * FROM chapters WHERE book_id=1 ORDER BY chapter_number"),
            ("chapter_order", "SELECT * FROM chapter_segments WHERE chapter_id=1 ORDER BY sequence_order"),
            ("audio_hash", "SELECT * FROM chapter_segments WHERE audio_hash='abc'"),
            ("status", "SELECT * FROM unresolved_threads WHERE status='INTRODUCED'"),
        ]:
            rows = conn.execute(f"EXPLAIN QUERY PLAN {sql}").fetchall()
            plans[label] = " ".join(r["detail"] for r in rows)
    finally:
        conn.close()

    assert "idx_chapters_book_order" in plans["book_order"], plans["book_order"]
    assert "idx_segments_chapter_order" in plans["chapter_order"], plans["chapter_order"]
    assert "idx_segments_audio_hash" in plans["audio_hash"], plans["audio_hash"]
    assert "idx_threads_status" in plans["status"], plans["status"]
