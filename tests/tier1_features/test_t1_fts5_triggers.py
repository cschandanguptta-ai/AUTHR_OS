"""Tier 1 — F9: FTS5 virtual table & 3 synchronization triggers.

Test IDs T1.FT1–T1.FT6 (6 tests).
"""

from helpers.assertions import assert_ok


def _search(cli, bundle, query, limit=20):
    return assert_ok(cli("segment_search", bundle=bundle, args={
        "query": query, "limit": limit,
    }), "segment_search")["hits"]


def test_ft1_bm25_search_matches_inserted_text(seeded_bundle, cli):
    """T1.FT1 — insert trigger populates FTS; BM25 query finds the segment."""
    hits = _search(cli, seeded_bundle, "dormant power")
    assert len(hits) == 1
    assert "dormant power" in hits[0]["text_content"]
    assert hits[0]["chapter_id"] == 1


def test_ft2_porter_stemming_matches_variants(seeded_bundle, cli):
    """T1.FT2 — porter stemmer: query `wakes` matches `wakes up` text."""
    hits = _search(cli, seeded_bundle, "wakes")
    assert len(hits) >= 1
    assert any("wakes up" in h["text_content"] for h in hits)


def test_ft3_update_trigger_resyncs_fts(seeded_bundle, cli):
    """T1.FT3 — UPDATE OF text_content trigger re-syncs the FTS row."""
    segs = assert_ok(cli("segment_list", bundle=seeded_bundle, args={"chapter_id": 1}))["segments"]
    sid = segs[0]["segment_id"]
    new_text = "The reactor was singing an ancient song of ignition and dawn."
    r = cli("sync_lexical_ast_mutation", bundle=seeded_bundle, args={
        "chapter_id": 1,
        "upserts": [dict(segs[0], text_content=new_text)],
        "deletes": [],
    })
    assert_ok(r, "sync update")
    hits = _search(cli, seeded_bundle, "ignition")
    assert any(h["segment_id"] == sid for h in hits)
    hits_old = _search(cli, seeded_bundle, "dormant")
    assert not any(h["segment_id"] == sid for h in hits_old)


def test_ft4_delete_trigger_removes_fts_row(seeded_bundle, cli):
    """T1.FT4 — AFTER DELETE trigger removes the FTS row."""
    segs = assert_ok(cli("segment_list", bundle=seeded_bundle, args={"chapter_id": 1}))["segments"]
    sid = segs[0]["segment_id"]
    assert_ok(cli("sync_lexical_ast_mutation", bundle=seeded_bundle, args={
        "chapter_id": 1, "upserts": [], "deletes": [sid],
    }), "sync delete")
    hits = _search(cli, seeded_bundle, "dormant")
    assert not any(h["segment_id"] == sid for h in hits)


def test_ft5_unicode61_accented_content(seeded_bundle, cli):
    """T1.FT5 — unicode61 tokenizer handles accented / non-ASCII prose."""
    assert_ok(cli("segment_append", bundle=seeded_bundle, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "Le café de la marina brillait sous les étoiles éteintes.",
    }))
    hits = _search(cli, seeded_bundle, "café")
    assert len(hits) >= 1 and any("café" in h["text_content"] for h in hits)


def test_ft6_search_scoped_across_chapters(seeded_bundle, cli):
    """T1.FT6 — FTS covers all chapters; second chapter content is searchable."""
    assert_ok(cli("chapter_create", bundle=seeded_bundle, args={
        "book_id": 1, "chapter_number": 2, "title": "Signal Decay", "act_number": 1,
    }))
    assert_ok(cli("segment_append", bundle=seeded_bundle, args={
        "chapter_id": 2, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "Quantum weather battered the array all night.",
    }))
    hits = _search(cli, seeded_bundle, "quantum")
    assert len(hits) == 1 and hits[0]["chapter_id"] == 2
    # Unrelated query returns nothing.
    assert _search(cli, seeded_bundle, "zebra") == []


def test_ft7_hostile_queries_are_sanitized_not_errors(seeded_bundle, cli):
    """T1.FT7 — T-2 regression: FTS5 syntax in segment_search is searched
    literally (shared rag sanitizer + bound parameter), never a 500-level
    MATCH syntax error. Mirrors the rag fts hostile-input test."""
    for hostile in ['"unclosed', "dormant OR (", "dormant AND power NOT x",
                    "*^;--", "NEAR(dormant, power)", '"phrases with quotes"']:
        hits = _search(cli, seeded_bundle, hostile)
        assert isinstance(hits, list), f"hostile `{hostile}` errored"
    # Literal terms inside hostile-looking input still match.
    hits = _search(cli, seeded_bundle, "dormant AND power")
    assert len(hits) >= 1 and "dormant power" in hits[0]["text_content"]
