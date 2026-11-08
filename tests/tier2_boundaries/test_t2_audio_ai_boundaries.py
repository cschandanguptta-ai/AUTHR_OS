"""Tier 2 — audio & AI boundary conditions (contract v1.1 codes)."""

import json
import os
import subprocess

from helpers.audio_fixtures import make_wav, seed_audio_chapter


def _raw_call(bin_path, env, command, bundle, args_dict):
    cmd = [bin_path, command]
    if bundle is not None:
        cmd += ["--bundle", str(bundle)]
    if args_dict is not None:
        cmd += ["--args", json.dumps(args_dict)]
    p = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=60)
    return json.loads(p.stdout)


def test_b1_unknown_engine_is_audio_error(bundle, cli):
    r = cli("audio_synthesize", bundle=bundle(), args={"text": "x", "engine": "WATSON"})
    assert not r.ok
    err = r.payload["error"]
    assert err["code"] == "AUDIO_ERROR" and err["category"] == "audio"


def test_b2_empty_text_rejected(bundle, cli):
    r = cli("audio_synthesize", bundle=bundle(), args={"text": "   "})
    assert not r.ok
    assert r.payload["error"]["code"] == "AUDIO_ERROR"


def test_b3_sample_rate_bounds(bundle, cli):
    b = bundle()
    for bad in (4000, 192000):
        r = cli("audio_synthesize", bundle=b, args={"text": "x", "sample_rate": bad})
        assert not r.ok
        assert r.payload["error"]["code"] == "AUDIO_ERROR", f"rate {bad}"


def test_b4_missing_text_and_chapter_is_storage_error(bundle, cli):
    r = cli("audio_synthesize", bundle=bundle(), args={})
    assert not r.ok
    assert r.payload["error"]["code"] == "STORAGE_ERROR"


def test_b5_layer_kind_validated(bundle, cli):
    b = bundle()
    make_wav(b.path / "x.wav", 0.1, 220, 0.4)
    r = cli("audio_master", bundle=b, args={"layers": [{"kind": "NOPE", "wav": "x.wav"}]})
    assert not r.ok
    assert r.payload["error"]["code"] == "AUDIO_ERROR"


def test_b6_layer_traversal_is_path_violation(bundle, cli):
    b = bundle()
    r = cli("audio_master", bundle=b, args={"layers": [{"kind": "SPEECH", "wav": "../escape.wav"}]})
    assert not r.ok
    assert r.payload["error"]["code"] == "PATH_TRAVERSAL_VIOLATION"


def test_b7_out_name_forced_into_exports(bundle, cli):
    b = bundle()
    make_wav(b.path / "x.wav", 0.1, 220, 0.4)
    r = cli("audio_master", bundle=b, args={
        "layers": [{"kind": "SPEECH", "wav": "x.wav"}], "out": "../evil.wav",
    })
    assert not r.ok
    assert r.payload["error"]["code"] == "PATH_TRAVERSAL_VIOLATION"
    r = cli("audio_master", bundle=b, args={
        "layers": [{"kind": "SPEECH", "wav": "x.wav"}], "out": "sub/evil.wav",
    })
    assert not r.ok
    assert r.payload["error"]["code"] == "PATH_TRAVERSAL_VIOLATION"
    r = cli("audio_master", bundle=b, args={
        "layers": [{"kind": "SPEECH", "wav": "x.wav"}], "out": "not_wav.txt",
    })
    assert not r.ok
    assert r.payload["error"]["code"] == "AUDIO_ERROR"
    assert not (b.path.parent / "evil.wav").exists()


def test_b8_validate_missing_wav_is_not_found(bundle, cli):
    r = cli("audio_validate", bundle=bundle(), args={"wav": "exports/ghost.wav"})
    assert not r.ok
    # The sandbox resolver reports missing files as ENTITY_NOT_FOUND.
    assert r.payload["error"]["code"] == "ENTITY_NOT_FOUND"


def test_b9_rag_search_guards(bundle, cli):
    b = bundle()
    assert cli("rag_index", bundle=b, args={}).ok
    r = cli("rag_search", bundle=b, args={"query": "", "mode": "dense"})
    assert not r.ok and r.payload["error"]["code"] == "AI_ERROR"
    r = cli("rag_search", bundle=b, args={"query": "q", "mode": "magic"})
    assert not r.ok and r.payload["error"]["code"] == "AI_ERROR"


def test_b10_rag_unknown_source_and_embedder(bundle, cli):
    b = bundle()
    r = cli("rag_index", bundle=b, args={"sources": ["spellbook"]})
    assert not r.ok and r.payload["error"]["code"] == "AI_ERROR"
    r = cli("rag_index", bundle=b, args={"embedder": "gpt-4"})
    assert not r.ok and r.payload["error"]["code"] == "AI_ERROR"


def test_b11_minilm_embedder_without_model(bundle, cli):
    r = cli("rag_index", bundle=bundle(), args={"embedder": "minilm-onnx"})
    assert not r.ok
    assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"


def test_b12_unknown_voice_kokoro(bundle, cli):
    r = cli("local_tts_generate", bundle=bundle(), args={"text": "hi", "voice": "siri"})
    assert not r.ok
    assert r.payload["error"]["code"] == "AI_ERROR"


def test_b13_llm_empty_prompt(bundle, cli):
    r = cli("local_llm_generate", bundle=bundle(), args={"prompt": "  "})
    assert not r.ok
    assert r.payload["error"]["code"] == "AI_ERROR"


def test_b14_writer_commands_honor_readonly_lease(bundle, cli, tmp_path):
    """audio_synthesize / rag_index must fail READ_ONLY_PROJECT under a foreign lease."""
    b = bundle()
    lock = b.path / ".authr_lock"
    lock.write_text(json.dumps({
        "pid": 999999, "hostname": "other-host", "user": "someone",
        "locked_at": "2026-11-08T00:00:00Z", "lease_expires_at": "2099-01-01T00:00:00Z",
        "session_id": "foreign",
    }))
    for cmd, args in [
        ("audio_synthesize", {"text": "x"}),
        ("rag_index", {}),
        ("audio_master", {"layers": [{"kind": "SPEECH", "wav": "x.wav"}]}),
    ]:
        r = cli(cmd, bundle=b, args=args)
        assert not r.ok, f"{cmd} should be read-only blocked"
        assert r.payload["error"]["code"] == "READ_ONLY_PROJECT", f"{cmd}: {r.payload}"


def test_b15_new_commands_envelope_contract(bundle, cli):
    """Every new command must answer with a well-formed envelope (error or ok)."""
    b = bundle()
    probes = [
        ("audio_cache_stats", None), ("audio_pronounce", {"names": ["Kira"]}),
        ("audio_plan", {"chapter_id": 1}), ("audio_synthesize", {"text": "x"}),
        ("audio_master", {"layers": []}), ("audio_validate", {"wav": "x.wav"}),
        ("rag_index", {}), ("rag_search", {"query": "q"}), ("rag_status", None),
        ("local_llm_generate", {"prompt": "p"}), ("local_llm_status", None),
        ("local_tts_generate", {"text": "x"}), ("local_ai_status", None),
    ]
    from helpers.ipc_contract_harness import assert_envelope_ok, assert_envelope_error
    for cmd, args in probes:
        r = cli(cmd, bundle=b, args=args)
        if r.ok:
            assert_envelope_ok(r.payload)
        else:
            assert_envelope_error(r.payload)
