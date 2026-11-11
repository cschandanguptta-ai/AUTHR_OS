"""T4.UI2 — the in-app publish & export chain (R-04, UX roadmap Wave 1).

Drives the same call order as the Publish & Export surface (and smoke-matrix
steps 54–65): publish_epub (overlays from synthesized audio) → publish_docx →
publish_typst → publish_captions → publish_m4b → release_manifest →
release_verify, including the honesty check that a tampered artifact
verifies invalid while still answering with a well-formed envelope.
"""

import shutil
from pathlib import Path

import pytest

from helpers.assertions import assert_ok

requires_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not available")


def _seed_authored_book(bundle, cli):
    """Book + one synthesized chapter — the minimum publishable state."""
    b = bundle()
    assert cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "The Ember Gate",
        "act_number": 1, "synopsis": "A vault exhales cold dust.",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The tide turned at the ember gate; Kade never saw it coming.",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "DIALOGUE", "speaker_id": "NARRATOR",
        "text_content": "Then we burn the ledger.",
    }).ok
    d = assert_ok(cli("audio_synthesize", bundle=b, args={
        "chapter_id": 1, "engine": "SCRIPTED"}), "audio_synthesize")
    assert len(d["results"]) == 2
    return b


@requires_ffmpeg
def test_t4_ui2_publish_chain(bundle, cli, tmp_path):
    b = _seed_authored_book(bundle, cli)

    # EPUB 3 — synthesized chapters embed audio + SMIL overlays.
    epub = assert_ok(cli("publish_epub", bundle=b, args={"book_id": 1}), "publish_epub")
    assert epub["format"] == "EPUB 3" and epub["chapters"] == 1
    assert epub["overlays"] >= 1, "synthesized chapter must carry media overlays"
    epub_path = b.path / epub["out"]
    assert epub_path.is_file() and epub_path.stat().st_size > 5_000

    # DOCX manuscript.
    docx = assert_ok(cli("publish_docx", bundle=b, args={"book_id": 1}), "publish_docx")
    assert docx["chapters"] == 1 and docx["words"] > 0
    assert (b.path / docx["out"]).is_file()

    # Typst print source (PDF compiles only when a typst binary exists).
    typ = assert_ok(cli("publish_typst", bundle=b, args={"book_id": 1}), "publish_typst")
    assert (b.path / typ["typ"]).is_file()
    if not typ["compiled"]:
        assert typ.get("reason"), "degraded compile must explain itself"

    # Captions for the chapter picker row.
    caps = assert_ok(cli("publish_captions", bundle=b, args={"chapter_id": 1}), "publish_captions")
    assert (b.path / caps["vtt"]).is_file() and (b.path / caps["lrc"]).is_file()

    # M4B audiobook — whole-book scope now that every segment is synthesized.
    m4b = assert_ok(cli("publish_m4b", bundle=b, args={"book_id": 1}), "publish_m4b")
    assert (b.path / m4b["out"]).is_file() and m4b["bytes"] > 10_000

    # Release manifest (Advanced disclosure): release_* are bundle-free —
    # the artifact must be an ABSOLUTE path.
    manifest_out = tmp_path / "latest.json"
    rm = assert_ok(cli("release_manifest", args={
        "artifact": str(epub_path),
        "out": str(manifest_out),
    }), "release_manifest")
    md = rm["manifest_data"]
    assert md["artifact_sha256"] == rm["artifact_sha256"]
    assert md["artifact_bytes"] == epub_path.stat().st_size
    assert md["signature"] and md["public_key"]
    assert manifest_out.is_file()

    # Verify — independent check passes on the untouched artifact.
    rv = assert_ok(cli("release_verify", args={
        "manifest": str(manifest_out),
        "artifact": str(epub_path),
    }), "release_verify")
    assert rv["valid"] is True
    assert rv["checks"]["sha256"] is True and rv["checks"]["signature"] is True

    # Honesty: a tampered artifact reports valid:false, still ok:true.
    tampered = tmp_path / "tampered.epub"
    tampered.write_bytes(epub_path.read_bytes() + b"\x00tamper")
    rv2 = cli("release_verify", args={
        "manifest": str(manifest_out),
        "artifact": str(tampered),
    })
    assert rv2.ok, "verification of a tampered artifact is itself a valid check"
    assert rv2.data["valid"] is False
    assert rv2.data["checks"]["sha256"] is False


@requires_ffmpeg
def test_t4_ui2_epub_overlays_absent_when_silent(bundle, cli):
    """A never-synthesized book publishes text-only EPUB — no silent-film lie."""
    b = bundle()
    assert cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Silent Draft", "act_number": 1}).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "Text only — no audio exists for this chapter."}).ok
    epub = assert_ok(cli("publish_epub", bundle=b, args={"book_id": 1}), "publish_epub")
    assert epub["overlays"] == 0
    assert (b.path / epub["out"]).is_file()


def test_t4_ui2_release_manifest_requires_absolute_path(bundle, cli, tmp_path):
    """The release-manifest row pre-fills an absolute path — bare names fail."""
    b = bundle()
    assert cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "X", "act_number": 1}).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "placeholder"}).ok
    r = cli("release_manifest", args={"artifact": "relative/thing.epub"})
    assert not r.ok, "relative artifact paths are rejected (release_* are bundle-free)"
    assert r.code in ("PATH_TRAVERSAL_VIOLATION", "BUNDLE_ERROR"), r.payload
