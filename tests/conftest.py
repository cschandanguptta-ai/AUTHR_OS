"""AUTHR_OS E2E suite — pytest configuration and shared fixtures.

Isolation model (TEST_INFRA §1): every test runs in its own tmp sandbox
directory with a dedicated mock-keyring file and a fresh .authr bundle.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

# `tests.helpers.*` is the canonical import root (no sys.path hacks).
sys.path.insert(0, str(Path(__file__).resolve().parent))

REPO_ROOT = Path(__file__).resolve().parents[1]

from helpers.assertions import CLIResult  # noqa: E402
from helpers.bundle_harness import Bundle  # noqa: E402


@pytest.fixture(scope="session")
def authr_bin():
    """Locate the compiled core binary."""
    cand = os.environ.get("AUTHR_BIN")
    if cand:
        p = Path(cand)
        if not p.exists():
            pytest.fail(f"AUTHR_BIN={cand} does not exist")
        return str(p)
    p = REPO_ROOT / "src-tauri" / "target" / "release" / "authr"
    if not p.exists():
        pytest.fail(
            "core binary missing — build with: "
            "(cd src-tauri && cargo build --release)"
        )
    return str(p)


@pytest.fixture()
def env(tmp_path):
    """Headless environment: mock keychain with a per-test keyring file."""
    e = os.environ.copy()
    e["AUTHR_MOCK_KEYCHAIN"] = "1"
    e["AUTHR_MOCK_KEYRING_FILE"] = str(tmp_path / "mock_keyring.json")
    return e


class Cli:
    def __init__(self, bin_path, base_env):
        self.bin = bin_path
        self.base_env = base_env

    def __call__(self, command, bundle=None, args=None, env=None, timeout=60):
        cmd = [self.bin, command]
        if bundle is not None:
            # Accept str, Path, or a helpers Bundle object.
            b = getattr(bundle, "path", bundle)
            cmd += ["--bundle", str(b)]
        if args is not None:
            cmd += ["--args", json.dumps(args)]
        p = subprocess.run(
            cmd, capture_output=True, text=True,
            env=env or self.base_env, timeout=timeout,
        )
        try:
            payload = json.loads(p.stdout)
        except json.JSONDecodeError:
            payload = {
                "ok": False,
                "error": {
                    "code": "STORAGE_ERROR",
                    "category": "storage",
                    "message": f"malformed CLI output: {p.stdout[:200]!r} / {p.stderr[:200]!r}",
                },
            }
        return CLIResult(p.returncode, payload, p.stdout, p.stderr)


@pytest.fixture()
def cli(authr_bin, env):
    return Cli(authr_bin, env)


@pytest.fixture()
def bundle(cli, tmp_path):
    """Factory fixture: `bundle = make_bundle()` inits a fresh .authr project."""
    created = []

    def _make(name="Novel.authr", title="The Ember Gate", author="K. Voss", args=None):
        path = tmp_path / name
        payload = {"path": str(path), "title": title, "author": author}
        if args:
            payload.update(args)
        r = cli("project_init", args=payload)
        assert r.ok, f"project_init failed: {r.payload}"
        created.append(path)
        return Bundle(path)

    return _make


@pytest.fixture()
def seeded_bundle(bundle, cli):
    """A bundle with one chapter and three segments (narration/dialogue/monologue)."""
    b = bundle()
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Cold Orbit", "act_number": 1,
    })
    assert r.ok, r.payload
    cli("character_upsert", bundle=b, args={
        "character_key": "CHAR_KIRA", "display_name": "Kira Voss",
        "role": "PROTAGONIST", "ipa_phonetic": "\u02c8k\u026a\u0259r\u0259",
        "voice_engine": "EDGE_TTS", "voice_id": "en-US-JennyNeural",
    })
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "NARRATION", "speaker_id": "NARRATOR",
        "text_content": "The station hummed with dormant power.",
        "ambient_cue": "cyberpunk_rain_heavy",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "DIALOGUE", "speaker_id": "CHAR_KIRA",
        "emotion": "tense", "sfx_cue": "airlock_cycle",
        "text_content": "We have maybe three minutes before it wakes up.",
    }).ok
    assert cli("segment_append", bundle=b, args={
        "chapter_id": 1, "segment_type": "INTERNAL_MONOLOGUE", "speaker_id": "NARRATOR",
        "text_content": "Three minutes was never going to be enough.",
    }).ok
    return b
