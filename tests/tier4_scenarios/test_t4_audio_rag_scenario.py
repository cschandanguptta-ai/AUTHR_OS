"""Tier 4 — author scenario: full audio pipeline + RAG loop (Phases 4–5).

The deliverable walk-through: seed a chapter → lookahead schedule → synthesize
with continuity chaining → master 3-layer audio → ACX validate → index the
book into vector RAG → hybrid retrieval feeds the writing loop.
"""

import time

from helpers.audio_fixtures import make_wav, seed_audio_chapter


def test_full_audio_and_rag_scenario(bundle, cli):
    b = bundle()
    # ── 1. Author the cast + chapter ────────────────────────────────────────
    cli("lore_upsert", bundle=b, args={
        "key": "relay_array",
        "content": "The relay array sings at dawn; its hum drowns grief.",
    })
    book_id, chapter_id, seg_ids = seed_audio_chapter(b, cli)

    # ── 2. Lookahead schedule: chains NAR 0→2 / KIRA 1→4, ORIN free ────────
    plan = cli("audio_plan", bundle=b, args={"chapter_id": chapter_id}).payload["data"]
    assert plan["wave_count"] == 2 and plan["turns"] == 5
    assert len(plan["waves"][0]) == 3 and len(plan["waves"][1]) == 2

    # ── 3. Synthesize the whole chapter (SCRIPTED, cast voices) ─────────────
    r = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    assert r.ok, r.payload
    synth = r.payload["data"]
    assert synth["turns"] == 5 and synth["cache_hits"] == 0

    # ── 4. Cache hit on re-run (sub-second) ─────────────────────────────────
    t0 = time.monotonic()
    r2 = cli("audio_synthesize", bundle=b, args={"chapter_id": chapter_id, "engine": "SCRIPTED"})
    elapsed = time.monotonic() - t0
    assert r2.ok and r2.payload["data"]["cache_hits"] == 5
    assert elapsed < 5.0, f"cache re-pass took {elapsed:.2f}s"

    # ── 5. Master two deliverables: ACX speech master + scene mix ──────────
    listing = cli("segment_list", bundle=b, args={"chapter_id": chapter_id}).payload["data"]["segments"]
    paths = [s["audio_cache_path"] for s in listing]
    make_wav(b.path / "audio" / "sfx_custom" / "dawn_bed.wav", 0.3, 90, 0.3)
    make_wav(b.path / "audio" / "sfx_custom" / "relay_ping.wav", 0.04, 1200, 0.5)
    speech_layers = [{"kind": "SPEECH", "wav": p} for p in paths]
    r = cli("audio_master", bundle=b, args={
        "layers": speech_layers + [
            {"kind": "SFX", "wav": "audio/sfx_custom/relay_ping.wav", "gain_db": -4, "offset_ms": 50},
        ],
        "out": "ch1_scenario.wav",
        "sample_rate": 24000,
    })
    assert r.ok, r.payload
    master = r.payload["data"]
    assert master["metrics"]["peak_db"] <= -3.0

    # Scene mix: ambient bed rides under the speech chain (ducking check).
    r = cli("audio_master", bundle=b, args={
        "layers": speech_layers + [
            {"kind": "AMBIENT", "wav": "audio/sfx_custom/dawn_bed.wav", "gain_db": -9},
        ],
        "out": "ch1_scene_mix.wav",
        "sample_rate": 24000,
    })
    assert r.ok, r.payload
    assert r.payload["data"]["duck"]["ducked_ms"] > 0

    # ── 6. ACX validation of the delivered speech master ────────────────────
    # rms_ok + peak_ok are mastering-chain guarantees; the noise-floor leg
    # depends on the source material's room tone (the SCRIPTED device engine
    # outputs continuous tone), so the full `pass` gate is asserted in the
    # dedicated mastering suite with silent-tail fixtures.
    rep = cli("audio_validate", bundle=b, args={"wav": "exports/ch1_scenario.wav"}).payload["data"]
    assert rep["checks"]["rms_ok"] is True, rep
    assert rep["checks"]["peak_ok"] is True, rep

    # ── 7. Vector RAG: index + hybrid retrieval ─────────────────────────────
    r = cli("rag_index", bundle=b, args={})
    assert r.ok, r.payload
    assert r.payload["data"]["total_chunks"] == 1 + 3 + 5  # lore + cast + segments

    r = cli("rag_search", bundle=b, args={
        "query": "relay array dawn hum", "k": 3, "mode": "hybrid",
    })
    assert r.ok, r.payload
    hits = r.payload["data"]["hits"]
    assert hits, "hybrid search must return the lore + segment matches"
    assert any(h["ref_id"] == "relay_array" for h in hits)
    assert any(h["source"] == "chapters" for h in hits)

    # ── 8. Offline LLM beat-expansion round trip ────────────────────────────
    r = cli("local_llm_generate", bundle=b, args={
        "prompt": "The relay array sings at dawn; Kira answers before it stops asking.",
        "max_tokens": 24, "seed": 11,
    })
    assert r.ok, r.payload
    assert r.payload["data"]["backend"] == "OFFLINE_GRAMMAR"

    # ── 9. Bundle exports stay portable: audio + exports + manifest ─────────
    import zipfile
    import tempfile
    import subprocess
    import json as _json
    with tempfile.TemporaryDirectory() as td:
        out = f"{td}/scenario.zip"
        assert cli("project_export", bundle=b, args={"out": out}).ok
        with zipfile.ZipFile(out) as z:
            names = z.namelist()
        assert any(n.startswith("audio/segments/") and n.endswith(".wav") for n in names)
        assert "exports/ch1_scenario.wav" in names
        assert "manifest.json" in names
