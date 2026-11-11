"""Tier 1 — Audio Studio surface (Phase 4): cache stats, pronounce, plan.

Contract §4.11 commands exercised through the CLI transport.
"""

import pytest

from helpers.audio_fixtures import seed_audio_chapter


def test_a1_cache_stats_empty(bundle, cli):
    r = cli("audio_cache_stats", bundle=bundle())
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["chunks"] == 0 and d["bytes"] == 0
    assert d["hits"] == 0 and d["misses"] == 0


def test_a2_pronounce_lexicon_and_rules(bundle, cli):
    b = bundle()
    r = cli("audio_pronounce", bundle=b, args={"names": ["Kira", "Thax", "narrator"]})
    assert r.ok, r.payload
    entries = {e["name"]: e for e in r.payload["data"]["entries"]}
    assert entries["Kira"]["ipa"] == "ˈkɪərə"
    assert entries["Kira"]["source"] == "lexicon"
    assert entries["narrator"]["source"] == "lexicon"
    assert entries["Thax"]["source"] == "rules"
    assert entries["Thax"]["ipa"].startswith("θ")


def test_a3_plan_waves_and_continuity(bundle, cli):
    b = bundle()
    _, chapter_id, _ = seed_audio_chapter(b, cli)
    r = cli("audio_plan", bundle=b, args={"chapter_id": chapter_id})
    assert r.ok, r.payload
    d = r.payload["data"]
    # Turn layout [NAR, KIRA, NAR, ORIN, KIRA]: chains NAR 0→2 and KIRA 1→4.
    # Wave 1 = {NAR-0, KIRA-1, ORIN-3} (all indegree-0), wave 2 = {2, 4}.
    assert d["turns"] == 5
    assert d["speakers"] == 3
    assert d["wave_count"] == 2
    assert len(d["waves"][0]) == 3
    assert len(d["waves"][1]) == 2
    # Continuity: each same-speaker turn points at its predecessor.
    continuity = {(c["turn"], c["previous_request_from"]) for c in d["continuity"]}
    assert (2, 0) in continuity and (4, 1) in continuity
    assert len(d["deps"]) == 2


def test_a4_plan_unknown_chapter_is_entity_not_found(bundle, cli):
    r = cli("audio_plan", bundle=bundle(), args={"chapter_id": 999})
    assert not r.ok
    assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND"


def test_a5_pronounce_rejects_non_list(bundle, cli):
    r = cli("audio_pronounce", bundle=bundle(), args={"names": "Kira"})
    assert not r.ok
    assert r.payload["error"]["code"] == "AUDIO_ERROR"


def test_a6_cache_stats_reports_stored_chunks(bundle, cli):
    b = bundle()
    r1 = cli("audio_synthesize", bundle=b, args={
        "text": "The ember gate hums.", "engine": "SCRIPTED", "voice": "af_bella",
    })
    assert r1.ok, r1.payload
    assert r1.payload["data"]["cached"] is False
    r2 = cli("audio_cache_stats", bundle=b)
    assert r2.ok
    assert r2.payload["data"]["chunks"] == 1
    assert r2.payload["data"]["misses"] >= 1


@pytest.mark.parametrize("names", [["Kira", "Mira"], []])
def test_a7_pronounce_batch_shapes(bundle, cli, names):
    r = cli("audio_pronounce", bundle=bundle(), args={"names": names})
    assert r.ok
    assert r.payload["data"]["count"] == len(names)
