"""Tier 1 — 3-layer DSP mixer, ducking, mastering & ACX validation (Phase 4, Sprint 11)."""

from helpers.audio_fixtures import make_spike_wav, make_wav, read_wav_stats, seed_audio_chapter


def _three_layer_args(b):
    # Spike-train speech: 3% density over 0.4 s voiced + 0.5 s digital-silence
    # tail. Post-normalize crest forces limiter engagement while the whole-file
    # RMS lands mid-ACX-band and the tail gives the noise-floor check a clean
    # 500 ms window. Sine ambient ducks under the sidechain when voiced.
    make_spike_wav(b.path / "audio" / "segments" / "speech_fix.wav", 0.4, density=0.03,
                   tail_silence=0.6)
    make_wav(b.path / "audio" / "sfx_custom" / "ambient_fix.wav", 0.8, 110, 0.35)
    make_wav(b.path / "audio" / "sfx_custom" / "spot_fix.wav", 0.05, 880, 0.5)
    return {
        "layers": [
            {"kind": "SPEECH", "wav": "audio/segments/speech_fix.wav"},
            {"kind": "AMBIENT", "wav": "audio/sfx_custom/ambient_fix.wav", "gain_db": -6},
            {"kind": "SFX", "wav": "audio/sfx_custom/spot_fix.wav", "gain_db": -3, "offset_ms": 100},
        ],
        "out": "ch1_mastered.wav",
        "sample_rate": 24000,
    }


def test_m1_master_produces_wav_with_metrics_and_acx(bundle, cli):
    b = bundle()
    args = _three_layer_args(b)
    r = cli("audio_master", bundle=b, args=args)
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["out"] == "exports/ch1_mastered.wav"
    assert d["layers"] == 3
    assert d["sample_rate"] == 24000
    m = d["metrics"]
    assert abs(m["target_rms_db"] - (-20.0)) < 1e-9
    assert -23.0 < m["rms_db"] < -17.0, m
    assert m["peak_db"] <= -3.0, m
    assert m["limited"] is True, "spike-train input must engage the limiter"
    assert d["duck"]["ducked_samples"] > 0, "ambient must duck under speech"
    frames, rate = read_wav_stats(b.path / "exports" / "ch1_mastered.wav")
    assert rate == 24000 and frames > 0


def test_m2_mastered_output_passes_audio_validate(bundle, cli):
    b = bundle()
    args = _three_layer_args(b)
    # Validation deliverable: speech + spot SFX only — the ambient bed would
    # keep the trailing window hot and drown the noise-floor measurement.
    args["layers"] = [l for l in args["layers"] if l["kind"] != "AMBIENT"]
    assert cli("audio_master", bundle=b, args=args).ok
    r = cli("audio_validate", bundle=b, args={"wav": "exports/ch1_mastered.wav"})
    assert r.ok, r.payload
    rep = r.payload["data"]
    assert rep["pass"] is True, rep["issues"]
    assert rep["checks"]["rms_ok"] and rep["checks"]["peak_ok"]
    assert rep["noise_floor_db"] < -60.0


def test_m3_resampling_mixed_rates(bundle, cli):
    # 48 kHz speech + 22.05 kHz ambient → session rate 44.1 kHz.
    b = bundle()
    make_wav(b.path / "a48.wav", 0.3, 220, 0.5, rate=48000)
    make_wav(b.path / "a22.wav", 0.5, 110, 0.35, rate=22050)
    r = cli("audio_master", bundle=b, args={
        "layers": [
            {"kind": "SPEECH", "wav": "a48.wav"},
            {"kind": "AMBIENT", "wav": "a22.wav", "gain_db": -6},
        ],
        "sample_rate": 44100, "out": "resampled.wav",
    })
    assert r.ok, r.payload
    frames, rate = read_wav_stats(b.path / "exports" / "resampled.wav")
    assert rate == 44100
    assert abs(frames - int(0.3 * 44100)) < 441  # ±10 ms


def test_m4_chapter_pipeline_synth_then_master_then_validate(bundle, cli):
    b = bundle()
    _, chapter_id, _ = seed_audio_chapter(b, cli)
    r = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r.ok, r.payload
    listing = cli("segment_list", bundle=b, args={"chapter_id": chapter_id}).payload["data"]["segments"]
    paths = [s["audio_cache_path"] for s in listing]
    make_wav(b.path / "audio" / "sfx_custom" / "bed.wav", 0.3, 90, 0.3)
    r = cli("audio_master", bundle=b, args={
        "layers": [{"kind": "SPEECH", "wav": p} for p in paths] + [
            {"kind": "AMBIENT", "wav": "audio/sfx_custom/bed.wav", "gain_db": -9},
        ],
        "out": "ch1_full.wav",
    })
    assert r.ok, r.payload
    assert r.payload["data"]["layers"] == 6
    rep = cli("audio_validate", bundle=b, args={"wav": "exports/ch1_full.wav"}).payload["data"]
    assert rep["checks"]["peak_ok"], rep
    assert rep["checks"]["rms_ok"], rep
