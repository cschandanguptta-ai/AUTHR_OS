"""Tier 2 — BVA: stale lease recovery (dead PID / expired / cross-host / corrupt).

Test IDs T2.LR1–T2.LR5 (5 tests).
"""

from helpers.assertions import assert_error, assert_ok
from helpers.bundle_harness import fresh_expiry, stale_expiry


def test_lr1_dead_local_pid_recovered(cli, bundle):
    """T2.LR1 — a lock whose local PID is dead is reclaimed on next write."""
    b = bundle()
    b.write_dead_local_lock()
    r = cli("book_create", bundle=b, args={"title": "Recovered"})
    assert_ok(r, "write after dead-pid recovery")
    # Lock file cleaned up by the recovery path.
    assert b.lock() is None


def test_lr2_expired_remote_lease_recovered(cli, bundle):
    """T2.LR2 — a foreign-host lease past the 120 s TTL is reclaimed."""
    b = bundle()
    b.write_lock(hostname="REMOTE-HOST", lease_expires_at=stale_expiry(300))
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "After Expiry",
    })
    assert_ok(r, "write after expired remote lease")


def test_lr3_cross_host_fresh_lease_isolated(cli, bundle):
    """T2.LR3 — fresh cross-host lease is honored: writes rejected read-only."""
    b = bundle()
    b.write_lock(hostname="REMOTE-HOST", lease_expires_at=fresh_expiry(120))
    r = cli("book_create", bundle=b, args={"title": "Blocked"})
    assert_error(r, code="READ_ONLY_PROJECT", category="bundle")


def test_lr4_live_local_pid_readonly_open(cli, bundle):
    """T2.LR4 — live local PID lease: project_open mounts read-only."""
    b = bundle()
    b.write_live_local_lock(ttl_seconds=60)
    r = cli("project_open", bundle=b)
    assert_ok(r, "project_open")
    assert r.data["readonly"] is True


def test_lr5_corrupt_lock_recovered(cli, bundle):
    """T2.LR5 — a corrupt .authr_lock is detected and reclaimed."""
    b = bundle()
    b.corrupt_lock()
    r = cli("book_create", bundle=b, args={"title": "After Corruption"})
    assert_ok(r, "write after corrupt lock recovery")
    assert b.lock() is None
