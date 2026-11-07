"""Tier 1 — F11: .authr bundle layout, manifest v1, lease locking.

Test IDs T1.BL1–T1.BL6 (6 tests).
"""

import json

from helpers.assertions import assert_error, assert_ok
from helpers import schema_reference as sr


def test_bl1_container_layout(bundle):
    """T1.BL1 — full directory layout exists after project_init."""
    b = bundle()
    assert b.exists("manifest.json")
    assert b.exists("project.db")
    assert b.dirs_exist(), "manuscript/audio/assets/exports subdirs missing"


def test_bl2_manifest_v1_fields(bundle):
    """T1.BL2 — manifest.json carries all v1 fields with schema_version 1."""
    m = bundle().manifest()
    for f in sr.MANIFEST_FIELDS:
        assert f in m, f"manifest missing `{f}`"
    assert m["schema_version"] == 1
    assert m["authr_version"] == "3.0.0-PROD"
    assert isinstance(m["project_id"], str) and len(m["project_id"]) == 36


def test_bl3_open_acquires_lease(cli, bundle):
    """T1.BL3 — project_open writes .authr_lock with pid/host/expiry/session."""
    b = bundle()
    r = cli("project_open", bundle=b)
    assert_ok(r, "project_open")
    assert r.data["readonly"] is False
    lock = b.lock()
    assert lock is not None
    for f in sr.LOCK_FIELDS:
        assert f in lock, f"lock missing `{f}`"
    assert lock["pid"] > 0
    status = assert_ok(cli("project_lock_status", bundle=b), "lock status")
    assert status["locked"] is True


def test_bl4_readonly_on_same_host_conflict(cli, bundle):
    """T1.BL4 — a live same-host lease forces read-only mode."""
    b = bundle()
    b.write_live_local_lock(ttl_seconds=60)
    r = cli("project_open", bundle=b)
    assert_ok(r, "project_open")
    assert r.data["readonly"] is True
    r = cli("book_create", bundle=b, args={"title": "Blocked"})
    assert_error(r, code="READ_ONLY_PROJECT", category="bundle")


def test_bl5_close_releases_cleanly(cli, bundle):
    """T1.BL5 — project_close releases the lease and removes .authr_lock."""
    b = bundle()
    assert_ok(cli("project_open", bundle=b), "open")
    assert b.exists(".authr_lock")
    r = cli("project_close", bundle=b)
    data = assert_ok(r, "project_close")
    assert data["released"] is True
    assert not b.exists(".authr_lock")


def test_bl6_foreign_host_fresh_lease_isolated(cli, bundle):
    """T1.BL6 — fresh cross-host lease is honored (isolated), not reclaimed."""
    b = bundle()
    b.write_lock(hostname="OTHER-WORKSTATION", lease_expires_at="2099-01-01T00:00:00Z")
    r = cli("project_lock_status", bundle=b)
    status = assert_ok(r, "lock status")
    assert status["locked"] is True
    assert status["readonly"] is True
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 9, "title": "Blocked",
    })
    assert_error(r, code="READ_ONLY_PROJECT", category="bundle")
