"""Tier 2 — BVA: schema CHECK constraints & enum validation.

Test IDs T2.SC1–T2.SC5 (5 tests).
"""

from helpers.assertions import assert_error, assert_ok


def test_sc1_invalid_narrative_structure(bundle, cli):
    b = bundle()
    r = cli("book_create", bundle=b, args={
        "title": "Bad Structure", "narrative_structure": "SAVE_THE_DOG",
    })
    assert_error(r, category="storage", what="invalid narrative_structure")


def test_sc2_invalid_chapter_status(bundle, cli):
    b = bundle()
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Bad Status", "status": "IMAGINARY",
    })
    assert_error(r, category="storage", what="invalid chapter status")


def test_sc3_invalid_character_role(bundle, cli):
    b = bundle()
    r = cli("character_upsert", bundle=b, args={
        "character_key": "CHAR_X", "display_name": "X", "role": "EXTRA",
    })
    assert_error(r, category="storage", what="invalid role")


def test_sc4_invalid_voice_engine(bundle, cli):
    b = bundle()
    r = cli("character_upsert", bundle=b, args={
        "character_key": "CHAR_Y", "display_name": "Y", "voice_engine": "MAGIC_MIC",
    })
    assert_error(r, category="storage", what="invalid voice_engine")


def test_sc5_invalid_lore_category_and_duplicate_unique(bundle, cli):
    b = bundle()
    r = cli("lore_upsert", bundle=b, args={
        "key": "L1", "category": "CUISINE", "content": "…",
    })
    assert_error(r, category="storage", what="invalid lore category")

    # Duplicate unique key: two chapters with the same number in one book.
    assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "First",
    }))
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Duplicate",
    })
    assert_error(r, category="storage", what="duplicate (book_id, chapter_number)")
