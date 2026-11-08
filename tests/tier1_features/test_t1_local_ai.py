"""Tier 1 — local-first AI (Phase 5, Sprint 13): offline LLM, llama-server client, statuses."""

import json
import os
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer


def _cli(bin_path, env, command, bundle, args_dict):
    cmd = [bin_path, command]
    if bundle is not None:
        cmd += ["--bundle", str(bundle)]
    if args_dict is not None:
        cmd += ["--args", json.dumps(args_dict)]
    p = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=60)
    return json.loads(p.stdout)


class _StubLlamaServer(HTTPServer):
    """Records the last /completion request; replies OpenAI-compatible content."""

    def __init__(self):
        super().__init__(("127.0.0.1", 0), _Handler)
        self.last_body = None
        self.port = self.server_address[1]


class _Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length).decode()
        server = self.server
        server.last_body = json.loads(body)
        resp = json.dumps({
            "content": "The lighthouse answered in coldfire.",
            "timings": {"predicted_ms": 3.25},
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)

    def log_message(self, *a):
        pass


def test_l1_offline_llm_deterministic(bundle, cli):
    b = bundle()
    args = {"prompt": "Kira climbed the silent lighthouse", "max_tokens": 30, "seed": 7}
    r1 = cli("local_llm_generate", bundle=b, args=args)
    assert r1.ok, r1.payload
    d1 = r1.payload["data"]
    assert d1["backend"] == "OFFLINE_GRAMMAR"
    assert d1["prompt_cache_pinned"] is False
    r2 = cli("local_llm_generate", bundle=b, args=args)
    assert r2.payload["data"]["text"] == d1["text"]
    r3 = cli("local_llm_generate", bundle=b, args={**args, "seed": 8})
    assert r3.payload["data"]["text"] != d1["text"]


def test_l2_llm_status_default_offline(bundle, cli):
    r = cli("local_llm_status", bundle=bundle())
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["default_backend"] == "OFFLINE_GRAMMAR"
    assert d["capabilities"]["prompt_prefix_kv_cache"] is True
    assert d["server_configured"] is False


def test_l3_llama_server_backend_with_stub(bundle, cli, tmp_path):
    """Real HTTP path against a loopback stub — verifies cache_prompt/keep_alive."""
    import socket
    stub = _StubLlamaServer()
    t = threading.Thread(target=stub.serve_forever, daemon=True)
    t.start()
    try:
        b = bundle()
        env = os.environ.copy()
        env["AUTHR_MOCK_KEYCHAIN"] = "1"
        bin_path = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
        payload = _cli(bin_path, env, "local_llm_generate", b.path, {
            "prompt": "Chapter 3 opening:", "max_tokens": 24, "seed": 5,
            "server_url": f"http://127.0.0.1:{stub.port}",
        })
        assert payload["ok"], payload
        d = payload["data"]
        assert d["backend"] == "LLAMA_SERVER"
        assert d["prompt_cache_pinned"] is True
        assert d["text"] == "The lighthouse answered in coldfire."
        assert d["timings"]["predicted_ms"] == 3.25
        body = stub.last_body
        assert body["cache_prompt"] is True
        assert body["keep_alive"] == -1
        assert body["prompt"] == "Chapter 3 opening:"
    finally:
        stub.shutdown()


def test_l4_unreachable_server_is_engine_unavailable(bundle, cli):
    b = bundle()
    r = cli("local_llm_generate", bundle=b, args={
        "prompt": "hello", "server_url": "http://127.0.0.1:1",
    })
    assert not r.ok
    assert r.payload["error"]["code"] == "AI_ENGINE_UNAVAILABLE"


def test_l5_kokoro_model_missing_is_actionable(bundle, cli):
    b = bundle()
    r = cli("local_tts_generate", bundle=b, args={"text": "hello", "voice": "af_bella"})
    assert not r.ok
    err = r.payload["error"]
    assert err["code"] == "AI_ENGINE_UNAVAILABLE"
    assert "kokoro-v1.0.onnx" in err["message"]


def test_l6_local_ai_status_umbrella(bundle, cli):
    b = bundle()
    r = cli("local_ai_status", bundle=b)
    assert r.ok, r.payload
    d = r.payload["data"]
    assert d["embedder"] == "hashing-384"
    assert d["kokoro"]["model_present"] is False
    assert d["kokoro"]["voices"][0] == "af_bella"
    assert d["fallbacks"]["tts"] == "SCRIPTED"
    assert d["fallbacks"]["llm"] == "OFFLINE_GRAMMAR"


def test_l7_llama_server_env_var_routes_backend(bundle, cli):
    import socket
    stub = _StubLlamaServer()
    threading.Thread(target=stub.serve_forever, daemon=True).start()
    try:
        b = bundle()
        env = os.environ.copy()
        env["AUTHR_MOCK_KEYCHAIN"] = "1"
        env["AUTHR_LLAMA_SERVER"] = f"http://127.0.0.1:{stub.port}"
        bin_path = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
        payload = _cli(bin_path, env, "local_llm_generate", b.path, {"prompt": "go"})
        assert payload["ok"], payload
        assert payload["data"]["backend"] == "LLAMA_SERVER"
    finally:
        stub.shutdown()
