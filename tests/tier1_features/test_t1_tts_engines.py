"""Tier 1 — TTS engines, synthesis caching and chapter batch (Phase 4, Sprints 9–10)."""

from helpers.audio_fixtures import seed_audio_chapter


def test_s1_scripted_synthesis_and_cache_hit(bundle, cli):
    b = bundle()
    args = {"text": "The lighthouse keeper wakes.", "engine": "SCRIPTED", "voice": "af_bella"}
    r1 = cli("audio_synthesize", bundle=b, args=args)
    assert r1.ok, r1.payload
    d1 = r1.payload["data"]
    assert d1["cached"] is False
    assert d1["engine"] == "SCRIPTED"
    assert d1["duration_ms"] > 0
    assert d1["hash"] and len(d1["hash"]) == 64
    assert d1["path"].startswith("audio/segments/")
    # Identical request → sub-second cache hit.
    r2 = cli("audio_synthesize", bundle=b, args=args)
    assert r2.ok
    d2 = r2.payload["data"]
    assert d2["cached"] is True
    assert d2["hash"] == d1["hash"]
    stats = cli("audio_cache_stats", bundle=b).payload["data"]
    assert stats["chunks"] == 1 and stats["hits"] >= 1


def test_s2_different_voice_is_different_chunk(bundle, cli):
    b = bundle()
    a1 = {"text": "same words", "engine": "SCRIPTED", "voice": "af_bella"}
    a2 = {"text": "same words", "engine": "SCRIPTED", "voice": "am_adam"}
    h1 = cli("audio_synthesize", bundle=b, args=a1).payload["data"]["hash"]
    h2 = cli("audio_synthesize", bundle=b, args=a2).payload["data"]["hash"]
    assert h1 != h2
    assert cli("audio_cache_stats", bundle=b).payload["data"]["chunks"] == 2


def test_s3_chapter_batch_uses_cast_voices_and_updates_segments(bundle, cli):
    b = bundle()
    _, chapter_id, seg_ids = seed_audio_chapter(b, cli)
    r = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["turns"] == 5
    assert d["cache_hits"] == 0
    by_seg = {x["segment_id"]: x for x in d["results"]}
    # Kira's turn used the cast voice → different hash than Orin's turn.
    assert by_seg[seg_ids[1]]["speaker"] == "CHAR_KIRA"
    assert by_seg[seg_ids[1]]["hash"] != by_seg[seg_ids[3]]["hash"]
    # NARRATOR segments chain: segment 3's previous_request_id comes from segment 1.
    assert by_seg[seg_ids[2]]["previous_request_id"] == by_seg[seg_ids[0]]["request_id"]
    assert by_seg[seg_ids[0]]["previous_request_id"] is None
    # Segment rows updated with cache pointers.
    listing = cli("segment_list", bundle=b, args={"chapter_id": chapter_id}).payload["data"]["segments"]
    by_id = {s["segment_id"]: s for s in listing}
    assert by_id[seg_ids[0]]["audio_hash"] == by_seg[seg_ids[0]]["hash"]
    assert by_id[seg_ids[0]]["audio_cache_path"] == by_seg[seg_ids[0]]["path"]
    assert by_id[seg_ids[0]]["audio_duration_ms"] > 0
    assert by_id[seg_ids[1]]["prev_request_id"] is None  # KIRA first turn
    # KIRA's second turn (segment index 4) threads her first turn's request id.
    assert by_id[seg_ids[4]]["prev_request_id"] == by_seg[seg_ids[1]]["request_id"]
    assert by_id[seg_ids[3]]["prev_request_id"] is None  # ORIN first turn


def test_s4_second_chapter_pass_is_full_cache_hit(bundle, cli):
    b = bundle()
    _, chapter_id, _ = seed_audio_chapter(b, cli)
    r1 = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r1.ok, r1.payload
    r2 = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r2.ok, r2.payload
    assert r2.payload["data"]["cache_hits"] == 5
    assert r2.payload["data"]["turns"] == 5


def test_s4b_duplicate_text_cache_hit_stamps_segment_row(bundle, cli):
    # Two byte-identical segments: the second resolves to a cache hit, and its
    # row must still receive audio_hash / audio_cache_path / duration.
    b = bundle()
    _, chapter_id, _ = seed_audio_chapter(b, cli)
    dup = "Echo turn for the duplicate-text regression."
    for _ in range(2):
        assert cli("segment_append", bundle=b, args={
            "chapter_id": chapter_id, "segment_type": "NARRATION",
            "speaker_id": "NARRATOR", "text_content": dup,
        }).ok
    r = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r.ok, r.payload
    by_seg = {x["segment_id"]: x for x in r.payload["data"]["results"]}
    listing = cli("segment_list", bundle=b, args={"chapter_id": chapter_id}).payload["data"]["segments"]
    dups = [s for s in listing if s["text_content"] == dup]
    assert len(dups) == 2
    assert dups[0]["audio_hash"] == dups[1]["audio_hash"]
    for s in dups:
        assert s["audio_cache_path"], s
        assert s["audio_duration_ms"] > 0
        assert by_seg[s["segment_id"]]["cached"] in (True, False)


def test_s5_export_includes_cached_audio(bundle, cli, tmp_path):
    import zipfile
    b = bundle()
    r = cli("audio_synthesize", bundle=b, args={"text": "portability check", "engine": "SCRIPTED"})
    assert r.ok
    out = tmp_path / "portable.zip"
    r = cli("project_export", bundle=b, args={"out": str(out)})
    assert r.ok, r.payload
    with zipfile.ZipFile(out) as z:
        entries = z.namelist()
    assert any(n.startswith("audio/segments/") and n.endswith(".wav") for n in entries)


def test_s6_engine_unavailable_when_offline_guard(bundle, cli):
    # AUTHR_TTS_OFFLINE=1 blocks network engines deterministically.
    import os
    import subprocess
    import json
    b = bundle()
    e = os.environ.copy()
    e["AUTHR_MOCK_KEYCHAIN"] = "1"
    e["AUTHR_TTS_OFFLINE"] = "1"
    bin_path = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
    p = subprocess.run(
        [bin_path, "audio_synthesize", "--bundle", str(b.path),
         "--args", json.dumps({"text": "hello", "engine": "EDGE_TTS"})],
        capture_output=True, text=True, env=e,
    )
    payload = json.loads(p.stdout)
    assert payload["ok"] is False
    assert payload["error"]["code"] == "AUDIO_ENGINE_UNAVAILABLE"
    assert payload["error"]["category"] == "audio"
