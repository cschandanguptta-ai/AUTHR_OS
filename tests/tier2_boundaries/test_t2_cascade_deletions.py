"""Tier 2 — BVA: multi-level cascade deletes & ON DELETE SET NULL / ON UPDATE CASCADE.

Test IDs T2.CD1–T2.CD5 (5 tests).
"""

from helpers.assertions import assert_ok
from helpers.assertions import connect


def _setup(b, cli):
    assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Cascades",
    }))
    assert_ok(cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION",
        "speaker_id": "NARRATOR", "text_content": "cascade me",
    }))
    assert_ok(cli("thread_open", bundle=b, args={
        "thread_title": "Cascade thread", "introduced_in_chapter": 1,
        "payoff_requirement": "…",
    }))


def test_cd1_book_delete_cascades_everything(bundle, cli):
    """T2.CD1 — book delete cascades to chapters, segments, threads."""
    b = bundle()
    _setup(b, cli)
    assert_ok(cli("book_delete", bundle=b, args={"id": 1}))
    assert b.query("SELECT COUNT(*) c FROM chapters")[0]["c"] == 0
    assert b.query("SELECT COUNT(*) c FROM chapter_segments")[0]["c"] == 0
    assert b.query("SELECT COUNT(*) c FROM unresolved_threads")[0]["c"] == 0


def test_cd2_chapter_delete_cascades_segments_and_fts(seeded_bundle, cli):
    """T2.CD2 — chapter delete cascades segments; FTS no longer matches."""
    b = seeded_bundle
    assert_ok(cli("chapter_delete", bundle=b, args={"id": 1}))
    assert b.query("SELECT COUNT(*) c FROM chapter_segments")[0]["c"] == 0
    hits = assert_ok(cli("segment_search", bundle=b, args={"query": "dormant"}))["hits"]
    assert hits == []


def test_cd3_chapter_delete_cascades_open_threads(bundle, cli):
    """T2.CD3 — threads introduced in a deleted chapter cascade away."""
    b = bundle()
    _setup(b, cli)
    assert_ok(cli("chapter_delete", bundle=b, args={"id": 1}))
    assert b.query("SELECT COUNT(*) c FROM unresolved_threads")[0]["c"] == 0


def test_cd4_resolved_in_chapter_set_null(bundle, cli):
    """T2.CD4 — deleting the RESOLVING chapter SETs NULL, thread survives."""
    b = bundle()
    ch1 = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Planted",
    }))["id"]
    ch2 = assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 2, "title": "Paid Off",
    }))["id"]
    thread = assert_ok(cli("thread_open", bundle=b, args={
        "thread_title": "The locked case", "introduced_in_chapter": ch1,
        "payoff_requirement": "Open the case",
    }))
    resolved = assert_ok(cli("thread_resolve", bundle=b, args={
        "id": thread["id"], "resolved_in_chapter": ch2,
    }))
    assert resolved["resolved_in_chapter"] == ch2
    assert_ok(cli("chapter_delete", bundle=b, args={"id": ch2}))
    row = b.query("SELECT resolved_in_chapter, status FROM unresolved_threads WHERE id=?",
                  [thread["id"]])[0]
    assert row["resolved_in_chapter"] is None
    assert row["status"] == "RESOLVED"


def test_cd5_speaker_rename_on_update_cascades(seeded_bundle):
    """T2.CD5 — renaming character_key (PK) cascades into chapter_segments."""
    b = seeded_bundle
    conn = b.db()
    try:
        conn.execute(
            "UPDATE character_profiles SET character_key='CHAR_KIRA_VOSS' "
            "WHERE character_key='CHAR_KIRA'"
        )
        conn.commit()
        rows = conn.execute(
            "SELECT DISTINCT speaker_id FROM chapter_segments"
        ).fetchall()
        speakers = {r["speaker_id"] for r in rows}
        assert "CHAR_KIRA_VOSS" in speakers
        assert "CHAR_KIRA" not in speakers
    finally:
        conn.close()
