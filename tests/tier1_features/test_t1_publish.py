"""Tier 1 — publishing pipelines: EPUB 3, DOCX, Typst, captions, M4B (Phase 6, Sprint 14)."""

import shutil
import subprocess
import xml.etree.ElementTree as ET
import zipfile

from helpers.audio_fixtures import seed_audio_chapter


def _seed(bundle, cli):
    return seed_audio_chapter(bundle, cli)


def test_p1_epub3_structure_and_overlays(bundle, cli):
    b = bundle()
    _seed(b, cli)
    assert cli("audio_synthesize", bundle=b, args={"chapter_id": 1, "engine": "SCRIPTED"}).ok
    r = cli("publish_epub", bundle=b, args={"book_id": 1})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["format"] == "EPUB 3"
    assert d["chapters"] == 1 and d["overlays"] == 1
    assert d["duration_ms"] > 0
    dc = d["dc"]
    assert isinstance(dc["title"], str) and dc["title"]
    assert dc["identifier"].startswith("urn:")
    path = b.path / d["out"]
    assert path.exists()
    z = zipfile.ZipFile(path)
    first = z.infolist()[0]
    assert first.filename == "mimetype"
    assert first.compress_type == zipfile.ZIP_STORED
    assert z.read("mimetype") == b"application/epub+zip"
    names = z.namelist()
    assert "META-INF/container.xml" in names
    assert "OEBPS/content.opf" in names and "OEBPS/nav.xhtml" in names
    # Every XML document must be well-formed (the EPUBCheck bar we can run offline).
    for n in names:
        if n.endswith((".xhtml", ".opf", ".smil", ".xml")):
            ET.fromstring(z.read(n))
    opf = z.read("OEBPS/content.opf").decode()
    assert 'unique-identifier="bookid"' in opf
    assert "dcterms:modified" in opf
    assert 'properties="nav"' in opf
    assert "media-overlay=" in opf
    smil = z.read("OEBPS/overlay/ch01.smil").decode()
    assert "clipBegin" in smil and "clipEnd" in smil
    assert smil.count("<par") == 5
    assert any(n.startswith("OEBPS/audio/") and n.endswith(".wav") for n in names)


def test_p2_epub_without_audio_has_no_overlays(bundle, cli):
    b = bundle()
    _seed(b, cli)
    r = cli("publish_epub", bundle=b, args={"book_id": 1})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["overlays"] == 0
    z = zipfile.ZipFile(b.path / d["out"])
    assert not any(n.endswith(".smil") for n in z.namelist())


def test_p3_docx_manuscript(bundle, cli):
    b = bundle()
    _seed(b, cli)
    r = cli("publish_docx", bundle=b, args={"book_id": 1})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["chapters"] == 1 and d["paragraphs"] >= 7  # title + author + heading + 5 segments
    assert d["words"] > 10
    path = b.path / d["out"]
    z = zipfile.ZipFile(path)
    doc = z.read("word/document.xml").decode()
    ET.fromstring(doc)
    assert "Signal Bloom" in doc
    assert "Kira Voss" in doc  # cast display name resolved
    assert "<w:b/>" in doc  # bold speaker run
    assert "<w:i/>" in doc  # italic monologue run
    styles = z.read("word/styles.xml").decode()
    ET.fromstring(styles)
    assert "Heading1" in styles


def test_p4_typst_source_and_optional_pdf(bundle, cli):
    b = bundle()
    _seed(b, cli)
    r = cli("publish_typst", bundle=b, args={"book_id": 1})
    assert r.ok, r.payload
    d = r.payload["data"]
    typ_path = b.path / d["typ"]
    assert typ_path.exists()
    src = typ_path.read_text()
    assert "= Chapter 1: Signal Bloom" in src
    assert "#set page" in src
    has_typst = shutil.which("typst") is not None
    if has_typst:
        assert d["compiled"] is True and d["pdf"], d
        assert (b.path / d["pdf"]).exists()
    else:
        assert d["compiled"] is False
        assert "typst" in d["reason"].lower()


def test_p5_captions_lrc_vtt(bundle, cli):
    b = bundle()
    _, chapter_id, _ = _seed(b, cli)
    assert cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"}).ok
    r = cli("publish_captions", bundle=b, args={"chapter_id": chapter_id})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["cues"] == 5 and d["duration_ms"] > 0
    lrc = (b.path / d["lrc"]).read_text()
    assert lrc.startswith("[re:AUTHR_OS]")
    assert "[00:00.00]" in lrc
    vtt = (b.path / d["vtt"]).read_text()
    assert vtt.startswith("WEBVTT")
    assert " --> " in vtt
    # Cue timings must be monotonic and cover every segment.
    stamps = [ln.split(" --> ")[0] for ln in vtt.splitlines() if " --> " in ln]
    assert len(stamps) == 5
    assert stamps == sorted(stamps)


def test_p6_m4b_audiobook_with_chapter_atoms(bundle, cli):
    b = bundle()
    _, chapter_id, _ = _seed(b, cli)
    assert cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"}).ok
    r = cli("publish_m4b", bundle=b, args={"chapter_id": chapter_id})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["chapters"] == 1
    assert d["duration_ms"] > 0 and d["bytes"] > 1000
    out = b.path / d["out"]
    assert out.exists() and out.suffix == ".m4b"
    # ffprobe must see AAC + the chapter atom.
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=format_name",
         "-show_chapters", "-of", "json", str(out)],
        capture_output=True, text=True,
    )
    assert probe.returncode == 0, probe.stderr
    meta = __import__("json").loads(probe.stdout)
    assert "mov" in meta["format"]["format_name"] or "mp4" in meta["format"]["format_name"]
    assert len(meta.get("chapters", [])) == 1
    assert meta["chapters"][0]["tags"]["title"].startswith("Ch01")


def test_p7_m4b_book_level_rejects_unsynthesized(bundle, cli):
    b = bundle()
    _seed(b, cli)  # segments exist but are never synthesized
    r = cli("publish_m4b", bundle=b, args={"book_id": 1})
    assert not r.ok
    assert r.payload["error"]["code"] == "BUNDLE_ERROR"
    assert "audio_synthesize" in r.payload["error"]["message"]
