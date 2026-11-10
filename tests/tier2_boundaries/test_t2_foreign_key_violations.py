"""Tier 2 — BVA: foreign key enforcement.

Test IDs T2.FK1–T2.FK5 (5 tests).
"""

from helpers.assertions import assert_error, assert_ok


def test_fk1_orphan_chapter_rejected(bundle, cli):
    b = bundle()
    r = cli("chapter_create", bundle=b, args={
        "book_id": 999, "chapter_number": 1, "title": "Orphan",
    })
    assert_error(r, code="FOREIGN_KEY_VIOLATION", category="storage")


def test_fk2_orphan_segment_chapter_missing(bundle, cli):
    b = bundle()
    r = cli("segment_append", bundle=b, args={
        "chapter_id": 777, "segment_type": "NARRATION",
        "speaker_id": "NARRATOR", "text_content": "…",
    })
    assert_error(r, code="FOREIGN_KEY_VIOLATION", category="storage")


def test_fk3_segment_unknown_speaker_rejected(seeded_bundle, cli):
    r = cli("segment_append", bundle=seeded_bundle, args={
        "chapter_id": 1, "segment_type": "DIALOGUE",
        "speaker_id": "CHAR_GHOST", "text_content": "Boo.",
    })
    assert_error(r, code="FOREIGN_KEY_VIOLATION", category="storage")


def test_fk4_thread_missing_chapter_rejected(bundle, cli):
    b = bundle()
    r = cli("thread_open", bundle=b, args={
        "thread_title": "Dangling", "introduced_in_chapter": 4242,
        "payoff_requirement": "…",
    })
    assert_error(r, code="FOREIGN_KEY_VIOLATION", category="storage")


def test_fk5_resolve_into_missing_chapter_rejected(bundle, cli):
    """FK5 — resolving a thread into a nonexistent chapter violates the FK.
    (Lorebook `key` is UNIQUE per §6.1 but upsert-by-key per contract §4.5;
    the duplicate-key rejection evidence lives in test_t2_schema_constraints.)"""
    b = bundle()
    thread = assert_ok(cli("thread_open", bundle=b, args={
        "thread_title": "The buried ledger", "introduced_in_chapter": None or _mk_chapter(b, cli),
        "payoff_requirement": "Ledger surfaces in Act III",
    }), "thread_open")
    r = cli("thread_resolve", bundle=b, args={
        "id": thread["id"], "resolved_in_chapter": 98765,
    })
    assert_error(r, code="FOREIGN_KEY_VIOLATION", category="storage")


def _mk_chapter(b, cli):
    row = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Seed",
    }))
    return row["id"]
