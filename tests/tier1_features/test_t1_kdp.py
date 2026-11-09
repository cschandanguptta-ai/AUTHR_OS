"""Tier 1 — KDP storefront readiness & packaging (contract v1.3).

Test IDs T1.KD1–T1.KD5.
"""

import json
import sqlite3
import xml.etree.ElementTree as ET

from helpers.ipc_contract_harness import assert_envelope_error, assert_envelope_ok
from helpers.audio_fixtures import seed_audio_chapter

DESCRIPTION = (
    "A signal officer races a dying orbit to decrypt the last transmission "
    "from a station that officially never existed."
)

KEYWORDS = ["lighthouse mystery", "coastal thriller", "slow burn suspense", "island secrets"]
CATEGORIES = ["Fiction / Mystery & Detective / Cozy", "Fiction / Thriller / Domestic"]


def _slug():
    return "the-ember-gate"


def test_kd1_check_flags_gaps_on_fresh_bundle(bundle, cli):
    """T1.KD1 — kdp_check on a seeded bundle fails honestly: no EPUB, no
    cover, no metadata; readiness is scored, never invented."""
    b = bundle()
    seed_audio_chapter(b, cli)
    r = cli("kdp_check", bundle=b, args={"book_id": 1, "price_usd": 4.99})
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["pass"] is False
    assert d["readiness_score"] < 70
    areas = {i["area"]: i["severity"] for i in d["issues"]}
    assert areas.get("epub") == "FAIL"
    assert areas.get("cover") == "FAIL"
    meta_issues = [i for i in d["issues"] if i["area"] == "metadata"]
    assert any(i["severity"] == "FAIL" for i in meta_issues)
    assert d["checks"]["manuscript"]["words"] > 0
    assert d["pricing_hint"]["ebook"]["tier_70"]["rate"] == 0.70


def test_kd2_check_sees_epub_after_publish(bundle, cli):
    """T1.KD2 — after publish_epub the epub check flips to present and the
    score rises; cover stays a FAIL (nothing to hide behind)."""
    b = bundle()
    seed_audio_chapter(b, cli)
    assert cli("publish_epub", bundle=b, args={"book_id": 1}).ok
    r = cli("kdp_check", bundle=b, args={"book_id": 1})
    d = r.payload["data"]
    assert d["checks"]["epub"]["present"] is True
    assert (b.path / "exports" / f"{_slug()}.epub").is_file()
    cover_issue = next(i for i in d["issues"] if i["area"] == "cover")
    assert cover_issue["severity"] == "FAIL"


def test_kd3_package_writes_and_persists(bundle, cli):
    """T1.KD3 — kdp_package writes metadata.json / description.html /
    copyright-page.xhtml / checklist.md into exports/kdp/<slug>/, persists the
    storefront payload, and a following kdp_check scores metadata 15/15."""
    b = bundle()
    seed_audio_chapter(b, cli)
    assert cli("publish_epub", bundle=b, args={"book_id": 1}).ok
    r = cli("kdp_package", bundle=b, args={
        "book_id": 1,
        "price_usd": 4.99,
        "keywords": KEYWORDS,
        "categories": CATEGORIES,
        "description": DESCRIPTION,
        "subtitle": "A Station Signal Novel",
        "ai_disclosure": True,
    })
    assert_envelope_ok(r.payload)
    d = r.payload["data"]
    assert d["package_dir"] == f"exports/kdp/{_slug()}"
    names = {f["name"] for f in d["files"]}
    assert names == {"metadata.json", "description.html", "copyright-page.xhtml", "checklist.md"}
    pkg = b.path / "exports" / "kdp" / _slug()

    meta = json.loads((pkg / "metadata.json").read_text())
    assert meta["title"] == "The Ember Gate"
    assert meta["keywords"] == KEYWORDS
    assert meta["categories"] == CATEGORIES
    assert meta["ai_disclosure"] is True
    assert meta["pricing"]["list_price_usd"] == 4.99

    html = (pkg / "description.html").read_text()
    assert "<script" not in html and "onerror" not in html
    assert html.startswith("<p>")
    assert "decrypt" in html

    cpage = (pkg / "copyright-page.xhtml").read_text()
    ET.fromstring(f"<root>{cpage}</root>")  # well-formed fragment
    assert "Copyright" in cpage and "AI-assisted tooling" in cpage

    checklist = (pkg / "checklist.md").read_text()
    assert "Amazon KDP" in checklist
    for kw in KEYWORDS:
        assert kw in checklist
    assert "Draft2Digital" in checklist

    # Storefront payload persisted in craft_state.
    conn = sqlite3.connect(b.path / "project.db")
    try:
        sf = json.loads(
            conn.execute("SELECT storefront_json FROM craft_state WHERE book_id=1").fetchone()[0]
        )
    finally:
        conn.close()
    assert sf["keywords"] == KEYWORDS and sf["price_usd"] == 4.99

    # kdp_check now sees full metadata (6+5+4 = 15).
    chk = cli("kdp_check", bundle=b, args={"book_id": 1}).payload["data"]
    assert chk["checks"]["metadata"]["score"] == 15
    assert chk["checks"]["metadata"]["keywords"] == len(KEYWORDS)
    assert chk["readiness_score"] >= 60  # cover FAIL is the only remaining hole
    royalties = d["royalties"]
    assert royalties["ebook"]["tier_70"]["rate"] == 0.70
    assert royalties["ebook"]["tier_70"]["royalty"] > 3.0
    assert royalties["paperback"]["min_list_price"] > 0


def test_kd4_package_validations_are_canonical(bundle, cli):
    """T1.KD4 — KDP hard limits raise CRAFT_ERROR, never silent truncation."""
    b = bundle()
    seed_audio_chapter(b, cli)
    base = {"book_id": 1, "price_usd": 4.99, "description": DESCRIPTION}
    bad_sets = [
        {**base, "keywords": [f"kw{i}" for i in range(8)]},          # 8 > 7 slots
        {**base, "keywords": ["x" * 51]},                            # > 50 chars
        {**base, "categories": ["a", "b", "c"]},                     # 3 > 2
        {**base, "description": "too short"},                        # < 20 chars
        {**base, "price_usd": 0.50},                                 # out of band
    ]
    for args in bad_sets:
        r = cli("kdp_package", bundle=b, args=args)
        assert_envelope_error(r.payload)
        assert r.payload["error"]["code"] == "CRAFT_ERROR"
    # Nothing was persisted for a failed package run.
    conn = sqlite3.connect(b.path / "project.db")
    try:
        row = conn.execute("SELECT storefront_json FROM craft_state WHERE book_id=1").fetchone()
        assert row is None or json.loads(row[0]) == {}
    finally:
        conn.close()


def test_kd5_royalty_tiers_match_kdp_model(bundle, cli):
    """T1.KD5 — pricing hint math: 70% tier inside $2.99-$9.99 minus delivery;
    35% tier outside with zero delivery fee."""
    b = bundle()
    seed_audio_chapter(b, cli)
    assert cli("publish_epub", bundle=b, args={"book_id": 1}).ok
    r = cli("kdp_check", bundle=b, args={"book_id": 1, "price_usd": 12.0})
    d = r.payload["data"]
    assert d["pricing_hint"]["ebook"]["tier_35"]["royalty"] == 4.20
    assert d["pricing_hint"]["ebook"]["tier_35"]["rate"] == 0.35
    assert d["pricing_hint"]["ebook"]["tier_70"]["eligible"] is False
    r2 = cli("kdp_check", bundle=b, args={"book_id": 1, "price_usd": 2.99})
    d2 = r2.payload["data"]
    assert d2["pricing_hint"]["ebook"]["tier_70"]["eligible"] is True
    assert d2["pricing_hint"]["ebook"]["tier_70"]["delivery_fee"] >= 0.0
