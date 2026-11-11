"""T4.UI3 — the Project Dashboard NBA data contract (RA-01, UX re-assessment D6).

The maturity-aware next-best-action ladder is browser-side logic
(src/lib/nba.ts — pure, ordered: first-chapter → write → cast → tag →
produce → scene → kdp). Its inputs come exclusively from frozen commands,
so this tier locks the exact fields the ladder consumes at each maturity
state:

  0 chapters              → ladder must open with chapter creation
                            (chapter_list == [], kdp readiness reported)
  1 chapter, 0 words      → ladder must open with "start writing"
                            (actual_word_count == 0, target_word_count present)
  1 chapter, words > 0    → storefront tier unlocked (kdp_check answers with a
                            readiness score; words gate it in the ladder)

The KDP-before-content regression (RA-01: an empty project being pointed at
"Complete your KDP package") is structurally impossible while these fields
hold: the ladder ranks KDP below word-count progress and only surfaces it
when words > 0.
"""

import pytest

from helpers.assertions import assert_ok


def test_t4_ui3_dashboard_nba_data_contract(bundle, cli):
    b = bundle()

    # ── Maturity state 1: fresh bundle, no chapters ──────────────────────
    books = assert_ok(cli("book_list", bundle=b), "book_list")
    assert len(books["books"]) == 1, "fresh bootstrap seeds exactly one scaffold book"

    chapters = assert_ok(cli("chapter_list", bundle=b), "chapter_list")
    assert chapters["chapters"] == [], "fresh bundle has no chapters → ladder state: first-chapter"

    kdp = assert_ok(
        cli("kdp_check", bundle=b, args={"book_id": 1, "price_usd": 4.99}), "kdp_check"
    )
    assert isinstance(kdp["readiness_score"], int)
    assert kdp["readiness_score"] == 0, "empty book must not present as storefront-ready"

    # ── Maturity state 2: one chapter, zero words ─────────────────────────
    created = assert_ok(
        cli("chapter_create", bundle=b, args={
            "book_id": 1, "chapter_number": 1, "title": "The Ember Gate",
            "act_number": 1, "synopsis": "A vault exhales cold dust.",
            "target_word_count": 2200,
        }),
        "chapter_create",
    )
    assert created["actual_word_count"] == 0
    assert created["target_word_count"] == 2200
    assert created["status"] == "PLANNED"

    chapters = assert_ok(cli("chapter_list", bundle=b), "chapter_list")
    assert len(chapters["chapters"]) == 1
    assert chapters["chapters"][0]["actual_word_count"] == 0, (
        "0 words → ladder state: write (never KDP)"
    )

    # ── Maturity state 3: content exists → storefront tier may surface ────
    seg = assert_ok(
        cli("segment_append", bundle=b, args={
            "chapter_id": created["id"], "segment_type": "NARRATION",
            "speaker_id": "NARRATOR",
            "text_content": (
                "The tide turned at the ember gate and Kade never saw the ledger "
                "burning behind him; ash rode the wind all the way to the sea."
            ),
        }),
        "segment_append",
    )
    assert seg["segment_id"] or seg.get("id"), "segment row identified"

    chapter = assert_ok(cli("chapter_get", bundle=b, args={"id": created["id"]}), "chapter_get")
    words = chapter["actual_word_count"]
    assert isinstance(words, int) and words >= 0
    if words == 0:
        pytest.fail(
            "actual_word_count must reflect appended segment text — the KDP "
            "tier of the NBA ladder unlocks on words > 0"
        )

    kdp = assert_ok(
        cli("kdp_check", bundle=b, args={"book_id": 1, "price_usd": 4.99}), "kdp_check"
    )
    assert isinstance(kdp["readiness_score"], int)
    assert isinstance(kdp["pass"], bool)

    # ── Coverage probe fields (produce/scene rules) ────────────────────────
    segs = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": created["id"]}), "segment_list")
    assert len(segs["segments"]) == 1
    assert "audio_hash" in segs["segments"][0], (
        "segment_list carries audio_hash — the per-chapter coverage probe keys on it"
    )
