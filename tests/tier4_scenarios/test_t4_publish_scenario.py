"""Tier 4 — end-to-end publishing scenario (Phase 6).

One author flow: manuscript → voice cast → lookahead synthesis → chapter
master → 1-click export of every deliverable (EPUB 3 + overlays, DOCX,
Typst source, captions, M4B audiobook), then cross-artifact consistency.
"""

import json
import os
import subprocess
import xml.etree.ElementTree as ET
import zipfile

from helpers.audio_fixtures import seed_audio_chapter


def test_full_publish_pipeline_produces_every_deliverable(bundle, cli):
    b = bundle()
    book_id, chapter_id, _ = seed_audio_chapter(b, cli)

    # 1. Lookahead synthesis with the offline deterministic engine.
    r = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r.ok, r.payload
    assert r.payload["data"]["turns"] == 5

    # 2. Chapter master through the 3-layer DSP rack (speech-only is enough).
    listing = cli("segment_list", bundle=b, args={"chapter_id": chapter_id}).payload["data"]["segments"]
    layers = [{"kind": "SPEECH", "wav": s["audio_cache_path"]} for s in listing]
    r = cli("audio_master", bundle=b, args={"layers": layers, "out": "ch01_master.wav"})
    assert r.ok, r.payload

    # 3. 1-click exports.
    epub = cli("publish_epub", bundle=b, args={"book_id": book_id})
    docx = cli("publish_docx", bundle=b, args={"book_id": book_id})
    typ = cli("publish_typst", bundle=b, args={"book_id": book_id})
    caps = cli("publish_captions", bundle=b, args={"chapter_id": chapter_id})
    m4b = cli("publish_m4b", bundle=b, args={"chapter_id": chapter_id})
    for r in (epub, docx, typ, caps, m4b):
        assert r.ok, r.payload

    exports = (b.path / "exports")
    produced = sorted(p.name for p in exports.iterdir() if not p.name.startswith("."))
    assert any(n.endswith(".epub") for n in produced)
    assert any(n.endswith(".docx") for n in produced)
    assert any(n.endswith(".typ") for n in produced)
    assert any(n.endswith(".lrc") for n in produced) and any(n.endswith(".vtt") for n in produced)
    assert any(n.endswith(".m4b") for n in produced)
    assert any(n.endswith(".wav") for n in produced)  # the chapter master

    # 4. Cross-artifact consistency: captions ↔ EPUB SMIL share the timeline.
    z = zipfile.ZipFile(exports / next(n for n in produced if n.endswith(".epub")))
    smil = z.read("OEBPS/overlay/ch01.smil").decode()
    ET.fromstring(smil)
    assert smil.count("<par") == 5
    vtt = (exports / next(n for n in produced if n.endswith(".vtt"))).read_text()
    assert vtt.count(" --> ") == 5

    # 5. The M4B carries the chapter atom and matches the caption duration.
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-show_chapters",
         "-of", "json", str(exports / next(n for n in produced if n.endswith(".m4b")))],
        capture_output=True, text=True,
    )
    meta = json.loads(probe.stdout)
    assert len(meta.get("chapters", [])) == 1
    assert abs(float(meta["format"]["duration"]) * 1000 - caps.payload["data"]["duration_ms"]) < 1500

    # 6. Project export remains portable with all publish artifacts included.
    out = b.path.parent / "portable_publish.zip"
    r = cli("project_export", bundle=b, args={"out": str(out)})
    assert r.ok, r.payload
    with zipfile.ZipFile(out) as zf:
        entries = zf.namelist()
    assert any(n.startswith("exports/") and n.endswith(".epub") for n in entries)
    assert any(n.startswith("exports/") and n.endswith(".m4b") for n in entries)
