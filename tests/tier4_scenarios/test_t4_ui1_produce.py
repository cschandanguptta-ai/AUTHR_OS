"""T4.UI1 — the in-app chapter-audio produce chain (R-01, UX roadmap Wave 1).

Drives the exact call sequence the Audio Studio produce card runs —
audio_plan → audio_synthesize → segment audio paths → audio_master (bare
`out` filename) → audio_validate → publish_captions → publish_m4b — against
the real release binary and asserts the artifacts + the ACX fields the UI
renders. Mirrors the smoke-matrix steps 41–46/57–58.
"""

import shutil

import pytest

from helpers.assertions import assert_ok
from helpers.ipc_contract_harness import assert_envelope_ok

requires_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not available")


@requires_ffmpeg
def test_t4_ui1_produce_chain(bundle, cli):
    b = bundle()

    # Cast + two-segment chapter (the minimal realistic produce target).
    assert cli("character_upsert", bundle=b, args={
        "character_key": "kade", "display_name": "Kade Rill",
        "role": "PROTAGONIST", "voice_engine": "KOKORO_LOCAL", "voice_id": "af_bella",
    }).ok
    assert cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Ember Gate", "act_number": 1,
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The tide turned at the ember gate; the vault exhaled cold dust.",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "DIALOGUE", "speaker_id": "kade",
        "emotion": "tense", "text_content": "We hold the line here, whatever it costs.",
    }).ok

    # Stage 1 — plan: the DAG the UI previews.
    plan = assert_ok(cli("audio_plan", bundle=b, args={"chapter_id": 1}), "audio_plan")
    assert plan["turns"] == 2 and plan["wave_count"] >= 1
    assert {t["speaker"] for t in plan["segment_turns"]} == {"NARRATOR", "kade"}

    # Stage 2 — synthesize: chapter form; per-segment paths live on segments.
    synth = assert_ok(cli("audio_synthesize", bundle=b, args={
        "chapter_id": 1, "engine": "SCRIPTED"}), "audio_synthesize")
    assert synth["chapter_id"] == 1 and synth["engine"] == "SCRIPTED"
    assert len(synth["results"]) == 2 and synth["cache_hits"] == 0

    segs = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": 1}), "segment_list")["segments"]
    assert all(s["audio_hash"] and s["audio_cache_path"] for s in segs), segs
    speech_layers = [{"kind": "SPEECH", "wav": s["audio_cache_path"]} for s in segs]

    # Stage 3 — master: `out` is a BARE filename (backend prepends exports/).
    master = assert_ok(cli("audio_master", bundle=b, args={
        "layers": speech_layers,
        "out": "audiobook_ch01_master.wav",
        "target_rms_db": -20.0,
        "ceiling_db": -3.1,
        "duck": True,
    }), "audio_master")
    assert master["out"] == "exports/audiobook_ch01_master.wav"
    assert master["layers"] == 2
    assert master["metrics"]["rms_db"] <= -18.0, "two-pass normalize must hit the ACX RMS band"
    assert master["metrics"]["peak_db"] <= -3.0, "lookahead limiter must honor the ceiling"
    assert set(master["acx"]["checks"]) == {"rms_ok", "peak_ok", "noise_ok"}
    assert (b.path / master["out"]).is_file()

    # Stage 4 — validate: the ACX panel's numbers, independently reported.
    acx = assert_ok(cli("audio_validate", bundle=b, args={
        "wav": "exports/audiobook_ch01_master.wav"}), "audio_validate")
    for field in ("pass", "rms_db", "peak_db", "noise_floor_db", "checks", "issues"):
        assert field in acx
    assert acx["checks"]["rms_ok"] and acx["checks"]["peak_ok"]
    # Honest numbers only: SCRIPTED tones cannot pass the noise gate — the UI
    # shows the real floor with an explainer instead of faking a pass.
    assert acx["pass"] == (acx["checks"]["noise_ok"] and acx["checks"]["rms_ok"] and acx["checks"]["peak_ok"])

    # Stage 5 — captions: line-synced VTT + LRC land in exports/.
    caps = assert_ok(cli("publish_captions", bundle=b, args={"chapter_id": 1}), "publish_captions")
    assert caps["cues"] == 2 and caps["duration_ms"] > 0
    assert caps["lrc"] and caps["vtt"]
    assert (b.path / caps["lrc"]).is_file() and (b.path / caps["vtt"]).is_file()

    # Stage 6 — optional M4B (checkbox enabled only when all segments are synthesized).
    m4b = assert_ok(cli("publish_m4b", bundle=b, args={"chapter_id": 1}), "publish_m4b")
    assert (b.path / m4b["out"]).is_file() and m4b["bytes"] > 1_000
    assert "aac-lc" in m4b["codec"]

    # Companion reads the UI renders beside the run: cache stats + G2P check.
    stats = assert_ok(cli("audio_cache_stats", bundle=b, args={}), "audio_cache_stats")
    assert stats["chunks"] >= 2 and stats["bytes"] > 0
    pron = assert_ok(cli("audio_pronounce", bundle=b, args={
        "names": ["Kade Rill"]}), "audio_pronounce")
    assert pron["count"] == 1
    assert pron["entries"][0]["source"] in ("lexicon", "rules")
    assert_envelope_ok(cli("audio_cache_stats", bundle=b, args={}).payload)


@requires_ffmpeg
def test_t4_ui1_m4b_disabled_until_synthesized(bundle, cli):
    """The M4B checkbox is honest: an unsynthesized chapter fails closed."""
    b = bundle()
    assert cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Raw", "act_number": 1}).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "Nobody has voiced this chapter yet."}).ok
    r = cli("publish_m4b", bundle=b, args={"chapter_id": 1})
    assert not r.ok, "M4B must refuse a chapter with unsynthesized segments"
    assert r.code == "BUNDLE_ERROR", r.payload
