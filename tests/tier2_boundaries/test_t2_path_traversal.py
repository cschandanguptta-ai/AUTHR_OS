"""Tier 2 — BVA: path traversal attack defense.

Test IDs T2.PT1–T2.PT5 (5 tests).
"""

from helpers.assertions import assert_error, assert_ok
from helpers.sandbox_harness import assert_normalized, make_evil_zip, try_import


def test_pt1_classic_dotdot_rejected(cli):
    r = cli("sandbox_normalize", args={"path": "../../etc/passwd"})
    assert_error(r, code="PATH_TRAVERSAL_VIOLATION", category="security")


def test_pt2_encoded_traversal_rejected(cli):
    r = cli("sandbox_normalize", args={"path": "manuscript/%2e%2e/%2e%2e/etc"})
    assert_error(r, code="PATH_TRAVERSAL_VIOLATION", category="security")


def test_pt3_deep_traversal_rejected(cli):
    r = cli("sandbox_normalize", args={"path": "a/b/c/d/../../../../../../../../root"})
    assert_error(r, code="PATH_TRAVERSAL_VIOLATION", category="security")


def test_pt4_null_byte_injection_rejected(cli):
    r = cli("sandbox_normalize", args={"path": "safe\x00../../evil"})
    assert_error(r, code="PATH_TRAVERSAL_VIOLATION", category="security")


def test_pt5_absolute_root_escape_in_archive_rejected(cli, tmp_path):
    """Root-escape archive entries (`/etc/passwd`) are rejected on import."""
    evil = make_evil_zip(tmp_path / "root_escape.zip", ["/etc/passwd"])
    r = try_import(cli, evil, tmp_path / "dest.authr")
    assert_error(r, code="ZIP_SLIP_VIOLATION", category="security")
