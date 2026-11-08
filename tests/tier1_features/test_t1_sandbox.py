"""Tier 1 — F4: Path sandboxing & Zip-Slip guard.

Test IDs T1.S1–T1.S5 (5 tests).
"""

import json

from helpers.assertions import assert_error, assert_ok
from helpers.sandbox_harness import assert_normalized, make_evil_zip, try_import


def test_s1_resolves_safe_relative_paths(cli):
    """T1.S1 — safe bundle-relative paths normalize without error."""
    r = cli("sandbox_normalize", args={"path": "manuscript/ch01.md"})
    assert_ok(r, "sandbox_normalize safe")
    assert r.data["normalized"] == "manuscript/ch01.md"
    r2 = cli("sandbox_normalize", args={"path": "audio/segments/"})
    assert_ok(r2, "sandbox_normalize audio dir")


def test_s2_strips_verbatim_prefix(cli):
    """T1.S2 — `\\\\?\\` verbatim prefix is stripped, drive case normalized."""
    assert_normalized(
        cli("sandbox_normalize", args={"path": "\\\\?\\C:\\tmp\\novel"}),
        "C:\\tmp\\novel",
    )


def test_s3_rejects_dotdot_traversal(cli):
    """T1.S3 — `../` escapes are rejected with PATH_TRAVERSAL_VIOLATION."""
    r = cli("sandbox_normalize", args={"path": "manuscript/../../etc"})
    assert_error(r, code="PATH_TRAVERSAL_VIOLATION", category="security")


def test_s4_rejects_zip_slip_archive(cli, tmp_path):
    """T1.S4 — archive entries with `../` are rejected on import."""
    evil = make_evil_zip(tmp_path / "evil.zip", ["../escaped.txt"])
    r = try_import(cli, evil, tmp_path / "dest.authr")
    assert_error(r, code="ZIP_SLIP_VIOLATION", category="security")


def test_s5_rejects_null_bytes(cli):
    """T1.S5 — null-byte path injections are rejected."""
    r = cli("sandbox_normalize", args={"path": "manuscript/ch0\x001.md"})
    assert_error(r, code="PATH_TRAVERSAL_VIOLATION", category="security")
