"""Tier 4 — full pipeline & live provider ladder (contract v1.6).

The v1.6 regression for the whole author→publish chain plus every live AI
seam, each exercised against the real release binary:

T4.FP1  one continuous pipeline: offline-LLM novel generation → multi-voice
        audio novel (SCRIPTED cast) → M4B audiobook → video novel assembled
        with the pipeline audio auto-chained as narration (novelcast leg
        order, end to end).
T4.FP2  live llama-server ladder: OFFLINE_GRAMMAR fail-closed default →
        `AUTHR_LLAMA_SERVER` env → provider settings beat env → explicit args
        beat settings; a real localhost HTTP stub asserts the llama.cpp wire
        flags (`cache_prompt:true`, `keep_alive:-1`, `n_predict`).
T4.FP3  live R2I image ladder: fail-closed without endpoint →
        `AUTHR_IMG_ENDPOINT` env probe (provider_test) → settings beat env →
        `video_render engine=R2I_ENDPOINT` registers real PNG scene art →
        assembler consumes it; an always-500 endpoint exhausts retries and
        fails closed with the canonical `AI_ENGINE_UNAVAILABLE`.
T4.FP4  command surface sweep: every contract command answers with a
        well-formed envelope (ok, or a frozen error code) and never panics
        on empty args.

The stubs are plain `http.server` threads on 127.0.0.1 ephemeral ports —
no external dependency, no internet, fully deterministic.
"""

import base64
import contextlib
import json
import os
import shutil
import struct
import threading
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from helpers.ipc_contract_harness import CONTRACT_COMMANDS, CANONICAL_CODES
from helpers.ipc_contract_harness import assert_envelope_ok

LLAMA_TEXT = "The tide turned at the ember gate; Kade never saw it coming."


# ── tiny stdlib PNG writer (magic-byte real, IHDR-parseable) ─────────────────
def _png_chunk(tag: bytes, data: bytes) -> bytes:
    return (struct.pack(">I", len(data)) + tag + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))


def _png_bytes(w: int = 64, h: int = 64) -> bytes:
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)  # 8-bit truecolor
    row = b"\x00" + b"\x5a\x8c\xdc" * w
    return (b"\x89PNG\r\n\x1a\n" + _png_chunk(b"IHDR", ihdr)
            + _png_chunk(b"IDAT", zlib.compress(row * h)) + _png_chunk(b"IEND", b""))


# ── localhost stub servers ───────────────────────────────────────────────────
class _LlamaHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        try:
            req = json.loads(self.rfile.read(n))
        except Exception:
            req = {}
        self.server.state["requests"].append(req)
        resp = json.dumps({"content": LLAMA_TEXT,
                           "timings": {"predicted_ms": 12.5}}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)


class _ImageHandler(BaseHTTPRequestHandler):
    """OpenAI-compatible /images/generations stub with forced-failure mode."""

    def log_message(self, *args):
        pass

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        self.rfile.read(n)
        st = self.server.state
        st["requests"] += 1
        if st["requests"] <= st["fail_first"]:
            body = b'{"error":{"message":"stub outage"}}'
            self.send_response(500)
        else:
            body = json.dumps({"data": [{"b64_json": st["png_b64"]}]}).encode()
            self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


@contextlib.contextmanager
def _http_stub(handler, state):
    srv = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    srv.state = state
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    try:
        yield srv
    finally:
        srv.shutdown()
        srv.server_close()


def _env(tmp_path, **extra):
    e = dict(os.environ)
    e["AUTHR_MOCK_KEYCHAIN"] = "1"
    e["AUTHR_MOCK_KEYRING_FILE"] = str(tmp_path / "kr.json")
    e.update(extra)
    return e


# ── T4.FP1 — the whole business chain in one continuous scenario ─────────────
def test_fp1_novel_to_audio_to_video_pipeline(bundle, cli):
    """T4.FP1 — generate the novel, voice it, film it. Same book throughout."""
    if shutil.which("ffmpeg") is None:
        import pytest
        pytest.skip("ffmpeg not available")

    b = bundle()
    r = cli("craft_apply", bundle=b, args={
        "book_id": 1, "format_id": "NOVEL", "plot_arc_id": "THREE_ACT",
        "style_persona_id": "EDGE_THRILLER", "adopt_defaults": True})
    assert_envelope_ok(r.payload)
    assert cli("craft_scaffold", bundle=b,
               args={"book_id": 1, "chapter_count": 2}).payload["data"]["created"] == 2
    cli("character_upsert", bundle=b, args={
        "character_key": "mira", "display_name": "Mira Voss",
        "voice_id": "af_nicole", "base_rate": 1.05})

    # LEG 1 — novel generation (offline deterministic LLM) → manuscript.
    for cid, seed in ((1, 42), (2, 1337)):
        d = cli("local_llm_generate", bundle=b, args={
            "prompt": f"Chapter {cid} — cold orbit thriller scene:",
            "max_tokens": 48, "seed": seed,
            "style_persona": "EDGE_THRILLER"}).payload["data"]
        assert d["backend"] == "OFFLINE_GRAMMAR"
        text = d["text"]
        assert len(text.split()) >= 20
        cli("segment_append", bundle=b, args={
            "chapter_id": cid, "segment_type": "NARRATION",
            "speaker_id": "NARRATOR", "text_content": text})
        cli("segment_append", bundle=b, args={
            "chapter_id": cid, "segment_type": "DIALOGUE", "speaker_id": "mira",
            "text_content": "Then we burn the ledger."})
    epub = cli("publish_epub", bundle=b, args={"book_id": 1}).payload["data"]["out"]
    assert (b.path / epub).is_file(), "novel deliverable missing"

    # LEG 2 — audio novel: multi-voice cast synthesis → M4B audiobook.
    for cid in (1, 2):
        d = cli("audio_synthesize", bundle=b,
                args={"chapter_id": cid, "engine": "SCRIPTED"}).payload["data"]
        assert len(d["results"]) == 2
        assert d["cache_hits"] == (0 if cid == 1 else 1), \
            "ch1 fresh; ch2 reuses the identical dialogue line (content-addressed cache)"
    d = cli("publish_m4b", bundle=b, args={"book_id": 1}).payload["data"]
    assert d["chapters"] == 2
    m4b = b.path / d["out"]
    assert m4b.is_file() and m4b.stat().st_size > 10_000

    # LEG 3 — video novel: plan → SCRIPTED render → assemble. The narration
    # track must auto-chain the LEG 2 synthesized audio (novelcast parity).
    d = cli("video_plan", bundle=b, args={"chapter_id": 1}).payload["data"]
    assert d["scene_count"] >= 2 and d["narration_ready"] is True
    d = cli("video_render", bundle=b,
            args={"chapter_id": 1, "engine": "SCRIPTED"}).payload["data"]
    assert d["rendered"], "no frames rendered"
    d = cli("video_assemble", bundle=b, args={"chapter_id": 1}).payload["data"]
    assert d["narration"] == "auto:pipeline-audio", d
    mp4 = b.path / d["out"]
    assert mp4.is_file() and mp4.stat().st_size > 50_000
    d = cli("video_validate", bundle=b, args={"chapter_id": 1}).payload["data"]
    assert d["ok"] is True and not [c for c in d["checks"] if c["status"] == "FAIL"]


# ── T4.FP2 — live llama-server ladder (LLM tier) ─────────────────────────────
def test_fp2_llama_server_ladder(bundle, cli, tmp_path):
    """T4.FP2 — offline fail-closed default; env < settings < args on live LLM."""
    b = bundle()
    prompt = {"prompt": "Chapter 1:", "max_tokens": 64, "seed": 42}

    # Baseline: nothing configured → deterministic offline engine.
    d = cli("local_llm_generate", bundle=b, args=prompt).payload["data"]
    assert d["backend"] == "OFFLINE_GRAMMAR"

    with _http_stub(_LlamaHandler, {"requests": []}) as stub_env, \
         _http_stub(_LlamaHandler, {"requests": []}) as stub_arg:
        url_env = f"http://127.0.0.1:{stub_env.server_port}"
        url_arg = f"http://127.0.0.1:{stub_arg.server_port}"

        # Tier env: AUTHR_LLAMA_SERVER routes to llama-server; the stub sees
        # the llama.cpp wire flags.
        e = _env(tmp_path, AUTHR_LLAMA_SERVER=url_env)
        d = cli("local_llm_generate", bundle=b, args=prompt, env=e).payload["data"]
        assert d["backend"] == "LLAMA_SERVER" and d["text"] == LLAMA_TEXT
        assert d["server"] == url_env and d["prompt_cache_pinned"] is True
        wire = stub_env.state["requests"][-1]
        assert wire["cache_prompt"] is True and wire["keep_alive"] == -1
        assert wire["n_predict"] == 64

        # Tier settings: provider_set beats the env stub.
        r = cli("provider_set", bundle=b, args={"server_url": url_arg})
        assert_envelope_ok(r.payload)
        e = _env(tmp_path, AUTHR_LLAMA_SERVER=url_env)
        d = cli("local_llm_generate", bundle=b, args=prompt, env=e).payload["data"]
        assert d["backend"] == "LLAMA_SERVER" and d["server"] == url_arg
        assert len(stub_arg.state["requests"]) == 1
        assert len(stub_env.state["requests"]) == 1, "env stub must be shadowed"

        # Resolution surface: provider_get reports the ladder redacted.
        d = cli("provider_get", bundle=b, args={}).payload["data"]
        assert d["llm"]["resolved"]["source"] == "settings"
        assert d["llm"]["resolved"]["server_url"] == url_arg
        assert d["ladder"] == "args > settings > env > fail-closed"

        # Tier args: explicit server_url outranks stored settings.
        with _http_stub(_LlamaHandler, {"requests": []}) as stub_top:
            d = cli("local_llm_generate", bundle=b, args={
                **prompt, "server_url": f"http://127.0.0.1:{stub_top.server_port}"},
            ).payload["data"]
            assert d["backend"] == "LLAMA_SERVER" and d["text"] == LLAMA_TEXT
            assert len(stub_top.state["requests"]) == 1

        # Clearing the setting drops back through env (still live), then
        # fully offline when the env var is absent too.
        assert cli("provider_set", bundle=b, args={"server_url": None}).ok
        e = _env(tmp_path, AUTHR_LLAMA_SERVER=url_env)
        d = cli("local_llm_generate", bundle=b, args=prompt, env=e).payload["data"]
        assert d["backend"] == "LLAMA_SERVER" and d["server"] == url_env
        d = cli("local_llm_generate", bundle=b, args=prompt).payload["data"]
        assert d["backend"] == "OFFLINE_GRAMMAR"


# ── T4.FP3 — live R2I image ladder (scene-art tier) ──────────────────────────
def test_fp3_r2i_image_ladder(bundle, cli, tmp_path):
    """T4.FP3 — fail-closed → env probe → settings → live PNG scene render."""
    b = bundle()
    cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Cold Orbit"})
    # Cast first — speaker_id carries an FK into character_profiles.
    cli("character_upsert", bundle=b, args={
        "character_key": "mira", "display_name": "Mira Voss", "role": "PROTAGONIST"})
    for seg_type, speaker, text in (
            ("NARRATION", "NARRATOR", "The gate breathed slow ion light over the dock."),
            ("DIALOGUE", "mira", "We burn it tonight.")):
        r = cli("segment_append", bundle=b, args={
            "chapter_id": 1, "segment_type": seg_type, "speaker_id": speaker,
            "text_content": text})
        assert r.ok, r.payload

    png_b64 = base64.b64encode(_png_bytes()).decode()
    state = {"requests": 0, "fail_first": 0, "png_b64": png_b64}

    # Fail-closed: no endpoint anywhere → canonical AI_ENGINE_UNAVAILABLE,
    # no placeholder asset produced.
    r = cli("provider_test", bundle=b, args={})
    assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"
    r = cli("video_render", bundle=b, args={"chapter_id": 1, "engine": "R2I_ENDPOINT"})
    assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"

    with _http_stub(_ImageHandler, state) as stub_env, \
         _http_stub(_ImageHandler, dict(state)) as stub_set, \
         _http_stub(_ImageHandler, dict(state, fail_first=99)) as stub_dead:
        url_env = f"http://127.0.0.1:{stub_env.server_port}"
        url_set = f"http://127.0.0.1:{stub_set.server_port}"
        url_dead = f"http://127.0.0.1:{stub_dead.server_port}"

        # Tier env: provider_test generates one real probe image.
        e = _env(tmp_path, AUTHR_IMG_ENDPOINT=url_env)
        d = cli("provider_test", bundle=b, args={}, env=e).payload["data"]
        assert d["ok"] is True and d["ext"] == "png" and d["bytes"] > 0
        assert d["endpoint_source"] == "env" and d["key_source"] in ("env", "keychain", "none")
        probe_hits = stub_env.state["requests"]
        assert probe_hits == 1

        # Tier settings: provider_set beats the env endpoint for real renders.
        assert cli("provider_set", bundle=b, args={"endpoint_url": url_set}).ok
        d = cli("provider_get", bundle=b, args={}).payload["data"]
        assert d["image"]["resolved"]["endpoint_url"] == url_set
        assert d["image"]["resolved"]["endpoint_source"] == "settings"
        d = cli("video_render", bundle=b,
                args={"chapter_id": 1, "engine": "R2I_ENDPOINT"}, env=e).payload["data"]
        scene_art = [a for a in d["rendered"] if a["kind"] == "SCENE"]
        assert scene_art, "no scene art registered"
        for a in scene_art:
            assert a["path"].endswith(".png"), a
            assert (b.path / a["path"]).is_file() and a["bytes"] > 0
            assert a["width"] == 64 and a["height"] == 64
        env_hits = stub_env.state["requests"]
        assert stub_set.state["requests"] >= len(scene_art), "settings stub must serve renders"
        assert env_hits == probe_hits, "env endpoint must be shadowed once settings exist"

        # The plan ladder now reports live reference art; the assembler
        # consumes the PNG registry entries (silent track — FP3 synthesizes
        # no audio; narration chaining is T4.FP1's concern).
        d = cli("video_plan", bundle=b, args={"chapter_id": 1}).payload["data"]
        assert d["ladder"]["refs_ready"] >= 1
        if shutil.which("ffmpeg") is not None:
            d = cli("video_assemble", bundle=b, args={"chapter_id": 1}).payload["data"]
            assert (b.path / d["out"]).is_file()

        # Fail-closed under outage: always-500 endpoint exhausts retries and
        # surfaces the canonical code (no partial assets).
        assert cli("provider_set", bundle=b, args={
            "endpoint_url": url_dead, "retries": 1}).ok
        r = cli("provider_test", bundle=b, args={})
        assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"
        assert stub_dead.state["requests"] >= 2, "retries must be attempted"
        r = cli("video_render", bundle=b, args={"chapter_id": 1, "engine": "R2I_ENDPOINT"})
        assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"


# ── T4.FP4 — command surface sweep (every tool answers in the app) ───────────
def test_fp4_command_surface_sweep(bundle, cli):
    """T4.FP4 — all 74 contract commands: envelope-shaped, frozen codes, no crash."""
    b = bundle()
    failures = []
    for command in CONTRACT_COMMANDS:
        r = cli(command, bundle=b, args={})
        if not isinstance(r.payload, dict) or not isinstance(r.payload.get("ok"), bool):
            failures.append((command, "malformed envelope", r.stdout[:100]))
            continue
        if not r.payload["ok"]:
            code = (r.payload.get("error") or {}).get("code")
            if code not in CANONICAL_CODES:
                failures.append((command, f"unfrozen error code {code!r}", ""))
        if r.exit_code not in (0, 1) or "panicked" in r.stderr:
            failures.append((command, f"crash rc={r.exit_code}", r.stderr[:140]))
    assert not failures, (
        f"{len(failures)}/{len(CONTRACT_COMMANDS)} command-surface failures: "
        f"{failures[:10]}")
