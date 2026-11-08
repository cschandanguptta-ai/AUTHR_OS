"""Tier 2 — publishing boundary conditions (Phase 6): sandboxed export names,
readonly enforcement, entity lookup, malformed scope."""

from helpers.assertions import assert_error
from helpers.bundle_harness import fresh_expiry


def test_pb1_hostile_book_title_stays_in_exports_sandbox(bundle, cli):
    b = bundle()
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "../../etc/passwd",
    })
    assert r.ok, r.payload
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The hostile-title slug must be neutralized.",
    }).ok
    r = cli("publish_docx", bundle=b, args={"book_id": 1})
    assert r.ok, r.payload
    out = r.payload["data"]["out"]
    assert out.startswith("exports/") and "/" not in out[len("exports/"):]
    assert ".." not in out
    # The file physically lives inside the bundle.
    assert (b.path / out).exists()


def test_pb2_unknown_book_is_entity_not_found(bundle, cli):
    b = bundle()
    r = cli("publish_epub", bundle=b, args={"book_id": 999})
    assert_error(r, code="ENTITY_NOT_FOUND")


def test_pb3_book_without_chapters_is_bundle_error(bundle, cli):
    b = bundle()
    assert cli("book_create", bundle=b, args={"title": "Empty Shell"}).ok
    r = cli("publish_epub", bundle=b, args={"book_id": 2})
    assert_error(r, code="BUNDLE_ERROR")
    assert "no chapters" in r.payload["error"]["message"]


def test_pb4_captions_unknown_format_is_bundle_error(seeded_bundle, cli):
    b = seeded_bundle  # chapter 1 + three segments
    r = cli("publish_captions", bundle=b, args={"chapter_id": 1, "formats": ["srt"]})
    assert_error(r, code="BUNDLE_ERROR")


def test_pb5_captions_on_empty_chapter_is_bundle_error(bundle, cli):
    b = bundle()
    r = cli("chapter_create", bundle=b, args={"book_id": 1, "chapter_number": 9, "title": "Blank"})
    assert r.ok, r.payload
    cid = r.payload["data"]["id"]
    r = cli("publish_captions", bundle=b, args={"chapter_id": cid})
    assert_error(r, code="BUNDLE_ERROR")


def test_pb6_publishing_is_rejected_on_readonly_bundle(seeded_bundle, cli):
    """A fresh cross-host lease forces every publish command read-only."""
    b = seeded_bundle  # chapter 1 + three segments
    b.write_lock(hostname="REMOTE-HOST", lease_expires_at=fresh_expiry(120))
    r = cli("publish_epub", bundle=b, args={"book_id": 1})
    assert_error(r, code="READ_ONLY_PROJECT", category="bundle")
    r = cli("publish_m4b", bundle=b, args={"book_id": 1})
    assert_error(r, code="READ_ONLY_PROJECT", category="bundle")
