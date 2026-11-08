"""Tier 1 — release manifest signing/verification + benchmark thresholds (Phase 6, Sprints 15–16)."""

import json
import os
import subprocess


def _run(binary, cmd, args):
    p = subprocess.run([binary, cmd, "--args", json.dumps(args)],
                       capture_output=True, text=True)
    return json.loads(p.stdout)


def test_r1_release_sign_verify_roundtrip(bundle, cli, tmp_path):
    binary = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
    artifact = tmp_path / "AUTHR_OS-3.0.0-PROD.tar.gz"
    payload = os.urandom(2048)
    artifact.write_bytes(payload)
    r = _run(binary, "release_manifest", {
        "artifact": str(artifact), "version": "3.0.0-PROD",
        "url": "https://releases.example.com/AUTHR_OS-3.0.0-PROD.tar.gz",
    })
    assert r["ok"], r
    d = r["data"]
    mf = d["manifest_data"]
    assert mf["version"] == "3.0.0-PROD"
    assert len(mf["public_key"]) == 64  # ed25519 pubkey = 32 bytes hex
    assert len(mf["signature"]) == 128  # ed25519 sig = 64 bytes hex
    assert d["generated_key_hex"] and len(d["generated_key_hex"]) == 64
    assert (tmp_path / "latest.json").exists()
    # Independent verification roundtrip.
    v = _run(binary, "release_verify", {"manifest": str(tmp_path / "latest.json"),
                                        "artifact": str(artifact)})
    assert v["ok"], v
    assert v["data"]["valid"] is True
    assert v["data"]["checks"] == {"sha256": True, "signature": True}


def test_r2_tampered_artifact_fails_verification(bundle, cli, tmp_path):
    binary = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
    artifact = tmp_path / "authr.bin"
    artifact.write_bytes(b"AUTHR release payload")
    r = _run(binary, "release_manifest", {"artifact": str(artifact), "key_hex": "11" * 32})
    assert r["ok"], r
    artifact.write_bytes(b"AUTHR release payloaD")  # flip one byte
    v = _run(binary, "release_verify", {"manifest": r["data"]["manifest"], "artifact": str(artifact)})
    assert v["ok"], v
    assert v["data"]["valid"] is False
    assert v["data"]["checks"]["sha256"] is False
    assert v["data"]["checks"]["signature"] is False


def test_r3_release_manifest_rejects_bad_key(bundle, cli, tmp_path):
    binary = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
    artifact = tmp_path / "x.bin"
    artifact.write_bytes(b"x")
    r = _run(binary, "release_manifest", {"artifact": str(artifact), "key_hex": "abcd"})
    assert not r["ok"]
    assert r["error"]["code"] == "BUNDLE_ERROR"
    r = _run(binary, "release_manifest", {"artifact": str(tmp_path / "missing.bin")})
    assert not r["ok"]


def test_b1_benchmark_meets_hardening_thresholds(bundle, cli):
    binary = os.environ.get("AUTHR_BIN") or "src-tauri/target/release/authr"
    r = _run(binary, "benchmark_run", {"iterations": 7})
    assert r["ok"], r
    d = r["data"]
    # Build-plan Sprint 16 thresholds: cold <1.5 s, idle RAM <50 MB, installer <25 MB.
    assert d["checks"]["cold_start"] is True, d["cold_start_ms"]
    assert d["checks"]["rss"] is True, d["rss"]
    assert d["checks"]["binary_size"] is True, d["binary"]
    assert d["pass"] is True
    assert d["cold_start_ms"]["median"] < 1500.0
    assert d["rss"]["peak_mb"] < 50.0
    assert d["binary"]["mb"] < 25.0
