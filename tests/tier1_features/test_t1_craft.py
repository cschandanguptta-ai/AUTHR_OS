"""Tier 1 — Craft catalog (contract v1.3): formats, personas, plot arcs,
beat scaffolding, persona-driven LLM prompts.

Test IDs T1.CR1–T1.CR8.
"""

import json
import sqlite3

from helpers.ipc_contract_harness import assert_envelope_error, assert_envelope_ok


def test_cr1_catalog_is_bundlefree_and_complete(cli):
    """T1.CR1 — craft_catalog runs without a bundle; counts match the frozen
    catalog sizes (25 formats / 37 personas / 18 arcs)."""
    r = cli("craft_catalog")
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["offline"] is True
    assert d["counts"] == {"formats": 25, "personas": 37, "arcs": 18}
    ids_f = {e["id"] for e in d["formats"]}
    ids_p = {e["id"] for e in d["personas"]}
    ids_a = {e["id"] for e in d["arcs"]}
    assert len(ids_f) == 25 and len(ids_p) == 37 and len(ids_a) == 18
    for arc in d["arcs"]:
        assert len(arc["beats"]) >= 4
        assert all("guidance" in b and "label" in b for b in arc["beats"])
    for p in d["personas"]:
        assert len(p["directive"]) > 40
    assert d["pricing"]["ebook"]["delivery_usd_per_mb"] == 0.15


def test_cr2_catalog_slices_and_unknown_kind(cli):
    """T1.CR2 — kind slices return entries; unknown kind is a CRAFT_ERROR."""
    for kind, count in (("formats", 25), ("personas", 37), ("arcs", 18)):
        r = cli("craft_catalog", args={"kind": kind})
        assert_envelope_ok(r.payload)
        assert r.payload["data"]["count"] == count
    r = cli("craft_catalog", args={"kind": "astrology"})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "CRAFT_ERROR"
    assert r.payload["error"]["category"] == "craft"


def test_cr3_apply_validates_and_syncs_legacy(bundle, cli):
    """T1.CR3 — craft_apply persists state, syncs persona_id and (only for
    legacy arcs) narrative_structure; adopt_defaults retargets the book."""
    b = bundle()
    r = cli("craft_apply", bundle=b, args={
        "book_id": 1, "format_id": "MEMOIR", "plot_arc_id": "MEMOIR_ARC",
        "style_persona_id": "MEMOIR_INTIMATE", "adopt_defaults": True,
    })
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["format"]["id"] == "MEMOIR"
    assert d["plot_arc"]["id"] == "MEMOIR_ARC"
    assert d["style_persona"]["id"] == "MEMOIR_INTIMATE"
    # MEMOIR_ARC has no legacy mapping → narrative_structure untouched.
    assert d["legacy_sync"]["persona_id"] == "MEMOIR_INTIMATE"
    assert "narrative_structure" not in d["legacy_sync"]
    book = cli("book_get", bundle=b, args={"id": 1}).payload["data"]
    assert book["persona_id"] == "MEMOIR_INTIMATE"
    assert book["narrative_structure"] == "SAVE_THE_CAT"
    assert book["target_word_count"] == 85000  # adopt_defaults from MEMOIR

    # A legacy arc DOES move narrative_structure (one of the 4 CHECK values).
    r2 = cli("craft_apply", bundle=b, args={"book_id": 1, "plot_arc_id": "STORY_CIRCLE"})
    assert_envelope_ok(r2.payload)
    book2 = cli("book_get", bundle=b, args={"id": 1}).payload["data"]
    assert book2["narrative_structure"] == "SAVE_THE_CAT"  # STORY_CIRCLE: no legacy
    r3 = cli("craft_apply", bundle=b, args={"book_id": 1, "plot_arc_id": "HEROS_JOURNEY"})
    book3 = cli("book_get", bundle=b, args={"id": 1}).payload["data"]
    assert book3["narrative_structure"] == "HEROS_JOURNEY"


def test_cr4_apply_rejects_unknown_ids(bundle, cli):
    """T1.CR4 — unknown format/arc/persona ids raise canonical CRAFT_ERROR."""
    b = bundle()
    for args in (
        {"book_id": 1, "format_id": "HAIKU_CYCLE"},
        {"book_id": 1, "plot_arc_id": "SALVADOR DalI"},
        {"book_id": 1, "style_persona_id": "GHOSTWRITER_X"},
    ):
        r = cli("craft_apply", bundle=b, args=args)
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "CRAFT_ERROR"
    r = cli("craft_apply", bundle=b, args={"book_id": 999})
    assert_envelope_error(r.payload)
    assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND"


def test_cr5_scaffold_creates_beat_chapters_and_is_idempotent(bundle, cli):
    """T1.CR5 — scaffold maps beats deterministically across N chapters,
    writes chapter_beats, and re-running maps without recreating."""
    b = bundle()
    assert cli("craft_apply", bundle=b, args={"book_id": 1, "plot_arc_id": "THREE_ACT"}).ok
    r = cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 6, "per_chapter_words": 2500})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["created"] == 6 and d["mapped"] == 6 and d["beats_total"] == 9
    chapters = cli("chapter_list", bundle=b, args={"book_id": 1}).payload["data"]["chapters"]
    assert [c["chapter_number"] for c in chapters] == [1, 2, 3, 4, 5, 6]
    assert all(c["status"] == "PLANNED" for c in chapters)
    assert chapters[0]["synopsis"] == "The event that starts the story clock."
    # act numbers derived from beat position, clamped to 1..3
    assert {c["act_number"] for c in chapters} <= {1, 2, 3}

    # Idempotent second pass: nothing new created, still mapped.
    r2 = cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 6})
    assert_envelope_ok(r2.payload)
    d2 = r2.payload["data"]
    assert d2["created"] == 0 and d2["mapped"] == 6
    chapters2 = cli("chapter_list", bundle=b, args={"book_id": 1}).payload["data"]["chapters"]
    assert len(chapters2) == 6

    # DB: craft_state + chapter_beats exist with FK-legal rows; beat_index
    # mapping is monotonic across chapter order.
    conn = sqlite3.connect(b.path / "project.db")
    try:
        rows = conn.execute(
            "SELECT b.beat_index FROM chapter_beats b JOIN chapters c ON c.id = b.chapter_id "
            "ORDER BY c.chapter_number"
        ).fetchall()
        assert len(rows) == 6
        beats = [r[0] for r in rows]
        assert beats == sorted(beats) and len(set(beats)) >= 4
        state = conn.execute("SELECT plot_arc_id FROM craft_state WHERE book_id = 1").fetchone()
        assert state[0] == "THREE_ACT"
    finally:
        conn.close()


def test_cr6_scaffold_one_call_with_arc_override(bundle, cli):
    """T1.CR6 — plot_arc_id on scaffold applies + scaffolds in one call."""
    b = bundle()
    r = cli("craft_scaffold", bundle=b, args={
        "book_id": 1, "plot_arc_id": "KISHOTENKETSU", "chapter_count": 4,
    })
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["beats_total"] == 4 and d["created"] == 4
    chapters = cli("chapter_list", bundle=b, args={"book_id": 1}).payload["data"]["chapters"]
    assert [c["title"] for c in chapters] == [
        "Ki · Introduction", "Shō · Development", "Ten · Twist", "Ketsu · Reconciliation",
    ]


def test_cr7_progress_reports_coverage_and_threads(bundle, cli):
    """T1.CR7 — craft_progress fuses beat coverage, word counts and the
    Chekhov ledger."""
    b = bundle()
    assert cli("craft_scaffold", bundle=b, args={
        "book_id": 1, "plot_arc_id": "SEVEN_POINT", "chapter_count": 7,
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The hook lands like weather.",
    }).ok
    assert cli("thread_open", bundle=b, args={
        "thread_title": "The unopened letter",
        "introduced_in_chapter": 1,
        "payoff_requirement": "Must be read before the resolution beat.",
    }).ok
    r = cli("craft_progress", bundle=b, args={"book_id": 1})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["total_beats"] == 7 and d["chapters"] == 7
    assert d["covered_beats"] == 1  # only chapter 1 has prose (started)
    assert d["total_words"] > 0
    assert d["unresolved_threads"] == 1
    beat_map = {x["beat_id"]: x for x in d["beats"]}
    assert beat_map["hook"]["started"] is True
    assert beat_map["pinch_2"]["started"] is False
    assert beat_map["hook"]["chapters"][0]["chapter_number"] == 1


def test_cr8_persona_directive_flows_into_llm_and_beat_cascade(cli, bundle):
    """T1.CR8 — local_llm_generate accepts style_persona; unknown persona is a
    CRAFT_ERROR; deleting a chapter cascades its beat stamp."""
    b = bundle()
    r = cli("local_llm_generate", bundle=b, args={
        "prompt": "The lighthouse keeper counted the ships.",
        "max_tokens": 24, "seed": 7, "style_persona": "HARDBOILED_NOIR",
    })
    assert_envelope_ok(r.payload)
    assert r.payload["data"]["style_persona"] == "HARDBOILED_NOIR"
    r2 = cli("local_llm_generate", bundle=b, args={
        "prompt": "x", "style_persona": "NOT_A_VOICE",
    })
    assert_envelope_error(r2.payload)
    assert r2.payload["error"]["code"] == "CRAFT_ERROR"

    # Cascade: scaffold 3 chapters on STORY_CIRCLE, delete one, beat row gone.
    assert cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 3}).ok
    conn = sqlite3.connect(b.path / "project.db")
    try:
        before = conn.execute("SELECT COUNT(*) FROM chapter_beats").fetchone()[0]
        assert before == 3
    finally:
        conn.close()
    assert cli("chapter_delete", bundle=b, args={"id": 1}).ok
    conn = sqlite3.connect(b.path / "project.db")
    try:
        after = conn.execute("SELECT COUNT(*) FROM chapter_beats").fetchone()[0]
        assert after == 2
    finally:
        conn.close()
