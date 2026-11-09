"""Tier 2 — BVA: cloud-sync directory detection & WAL truncation.

Test IDs T2.CW1–T2.CW5 (5 tests).
"""

import shutil

from helpers.assertions import assert_ok
from helpers.bundle_harness import Bundle


def _cloud_bundle(cli, tmp_path, dirname, title):
    root = tmp_path / dirname
    root.mkdir()
    path = root / "Novel.authr"
    r = cli("project_init", args={"path": str(path), "title": title})
    assert_ok(r, f"project_init under {dirname}")
    return Bundle(path)


def test_cw1_onedrive_detected(cli, tmp_path):
    b = _cloud_bundle(cli, tmp_path, "OneDrive", "Cloud One")
    r = cli("project_open", bundle=b)
    assert_ok(r).get("cloud_sync_guard") is True


def test_cw2_dropbox_detected(cli, tmp_path):
    b = _cloud_bundle(cli, tmp_path, "Dropbox", "Cloud Two")
    r = cli("project_open", bundle=b)
    assert_ok(r).get("cloud_sync_guard") is True


def test_cw3_google_drive_detected(cli, tmp_path):
    b = _cloud_bundle(cli, tmp_path, "Google Drive", "Cloud Three")
    r = cli("project_open", bundle=b)
    assert_ok(r).get("cloud_sync_guard") is True


def test_cw4_local_path_not_flagged(cli, bundle):
    b = bundle()
    r = cli("project_open", bundle=b)
    assert_ok(r).get("cloud_sync_guard") is False


def test_cw5_wal_truncated_to_zero_after_writes(cli, tmp_path):
    """T2.CW5 — under a cloud dir, writes end with a 0-byte WAL file."""
    b = _cloud_bundle(cli, tmp_path, "OneDrive", "Cloud Five")
    assert_ok(cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Cloud Writes",
    }))
    assert_ok(cli("project_close", bundle=b))
    assert b.wal_bytes() == 0, f"WAL not truncated: {b.wal_bytes()} bytes"
