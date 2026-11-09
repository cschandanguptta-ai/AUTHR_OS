"""Tier 4 — One-person authoring & publishing house (contract v1.3).

The full business pipeline as one scenario: apply craft presets, scaffold an
arc, write prose, index RAG, synthesize audio, publish, then package for
storefront sale. Test IDs T4.OP1.
"""

import json
from pathlib import Path

from helpers.ipc_contract_harness import assert_envelope_ok


def test_op1_one_person_publisher_pipeline(bundle, cli):
    """T4.OP1 — craft → scaffold → write → rag → audio → publish → package."""
    b = bundle(args={"title": "The Lighthouse Ledger", "author": "M. Calder"})

    # 1. Craft presets: memoir shape, memoir arc, intimate memoir voice.
    r = cli("craft_apply", bundle=b, args={
        "book_id": 1, "format_id": "MEMOIR", "plot_arc_id": "MEMOIR_ARC",
        "style_persona_id": "MEMOIR_INTIMATE", "adopt_defaults": True,
    })
    assert_envelope_ok(r.payload)
    assert r.payload["data"]["format"]["default_word_count"] == 85000
    book = cli("book_get", bundle=b, args={"id": 1}).payload["data"]
    assert book["persona_id"] == "MEMOIR_INTIMATE"
    assert book["target_word_count"] == 85000
    assert book["target_chapters"] == 20

    # 2. Beat scaffold across 8 chapters of the memoir arc.
    r = cli("craft_scaffold", bundle=b, args={"book_id": 1, "chapter_count": 8})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["created"] == 8 and d["beats_total"] == 8
    chapters = cli("chapter_list", bundle=b, args={"book_id": 1}).payload["data"]["chapters"]
    assert chapters[0]["title"] == "Snapshot of Then"
    assert chapters[4]["title"] == "Reckoning"

    # 3. Write the opening (two segments in ch1, one in ch2).
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The lighthouse ledger began the year my mother stopped speaking "
                        "to the sea, and it ended the year the sea answered back.",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "INTERNAL_MONOLOGUE", "speaker_id": "NARRATOR",
        "text_content": "I kept the light because she would not.",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 2, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The wound arrived quietly, the way weather arrives on islands.",
    }).ok

    # 4. Continuity index.
    r = cli("rag_index", bundle=b, args={"sources": ["chapters", "characters"]})
    assert_envelope_ok(r.payload)
    assert r.payload["data"]["total_chunks"] >= 3

    # 5. Offline audio for chapter 1 (SCRIPTED engine, zero network).
    r = cli("audio_synthesize", bundle=b, args={"chapter_id": 1, "engine": "SCRIPTED"})
    assert_envelope_ok(r.payload)

    # 6. Publish: ebook + manuscript + captions.
    r = cli("publish_epub", bundle=b, args={"book_id": 1})
    assert_envelope_ok(r.payload)
    epub_rel = r.payload["data"]["out"]
    assert (b.path / epub_rel).is_file()
    assert cli("publish_docx", bundle=b, args={"book_id": 1}).ok
    r = cli("publish_captions", bundle=b, args={"chapter_id": 1})
    assert_envelope_ok(r.payload)

    # 7. Storefront package: metadata, 5 keywords, 2 categories, price.
    description = (
        "A daughter keeps her mother's lighthouse ledgers and finds the year "
        "the sea came between them. A quiet, luminous memoir of coast, silence "
        "and the words that finally arrived."
    )
    r = cli("kdp_package", bundle=b, args={
        "book_id": 1,
        "price_usd": 5.99,
        "keywords": ["memoir", "lighthouse keeper", "coastal family story",
                     "grief and reconciliation", "island life"],
        "categories": ["Biography & Autobiography / Personal Memoirs",
                       "Fiction / Literary"],
        "description": description,
        "ai_disclosure": True,
    })
    assert_envelope_ok(r.payload)
    pkg = r.payload["data"]
    pkg_dir = b.path / pkg["package_dir"]
    assert pkg_dir.is_dir()
    assert {f["name"] for f in pkg["files"]} == {
        "metadata.json", "description.html", "copyright-page.xhtml", "checklist.md",
    }
    meta = json.loads((pkg_dir / "metadata.json").read_text())
    assert meta["title"] == "The Lighthouse Ledger"
    assert meta["author"] == "M. Calder"
    assert meta["style_persona_id"] == "MEMOIR_INTIMATE"

    # 8. Readiness check: everything but the cover is now in place.
    r = cli("kdp_check", bundle=b, args={"book_id": 1, "price_usd": 5.99})
    assert_envelope_ok(r.payload)
    chk = r.payload["data"]
    assert chk["checks"]["epub"]["present"] is True
    assert chk["checks"]["metadata"]["score"] == 15
    failing = [i for i in chk["issues"] if i["severity"] == "FAIL"]
    assert failing and all(i["area"] == "cover" for i in failing), (
        "the only honest FAIL left is the missing cover"
    )
    assert chk["readiness_score"] >= 60
    assert chk["pricing_hint"]["ebook"]["tier_70"]["eligible"] is True

    # 9. Craft progress closes the loop: 8/8 beats mapped, prose flowing.
    r = cli("craft_progress", bundle=b, args={"book_id": 1})
    assert_envelope_ok(r.payload)
    prog = r.payload["data"]
    assert prog["chapters"] == 8 and prog["total_beats"] == 8
    assert prog["total_words"] > 0
    assert prog["unresolved_threads"] == 0
