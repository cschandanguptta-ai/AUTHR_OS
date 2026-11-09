"""Tier 3 — Cross-feature pairwise interactions (6 tests).

Covers: lease+shadow, cascade+FTS+git, export+zip-slip, speaker cascade+markdown,
cloud+export, FTS+sync-bridge invalidation.
"""

import json

from helpers.assertions import assert_error, assert_ok
from helpers.bundle_harness import fresh_expiry
from helpers import git_harness as gh
from helpers.sandbox_harness import make_evil_zip


def test_x1_foreign_lease_freezes_shadow(cli, seeded_bundle):
    """T3.X1 — lease+shadow: under a foreign lease, shadow files stay frozen."""
    b = seeded_bundle
    before = (b.path / "manuscript" / "ch01.md").read_text()
    b.write_lock(hostname="OTHER-WS", lease_expires_at=fresh_expiry(120))
    r = cli("chapter_update", bundle=b, args={"id": 1, "title": "Should Not Apply"})
    assert_error(r, code="READ_ONLY_PROJECT", what="chapter_update under foreign lease")
    after = (b.path / "manuscript" / "ch01.md").read_text()
    assert before == after


def test_x2_cascade_fts_git_alignment(cli, seeded_bundle):
    """T3.X2 — cascade+FTS+git: chapter delete aligns DB, FTS and workdir."""
    b = seeded_bundle
    assert_ok(cli("chapter_delete", bundle=b, args={"id": 1}))
    assert b.query("SELECT COUNT(*) c FROM chapter_segments")[0]["c"] == 0
    assert assert_ok(cli("segment_search", bundle=b, args={"query": "dormant"}))["hits"] == []
    assert not (b.path / "manuscript" / "ch01.md").exists()
    assert not (b.path / "manuscript" / "ch01.ast.json").exists()
    # T-4 fix: the removal is committed immediately (contract-locked snapshot
    # message) — the workdir is clean and the commit records the deletion,
    # instead of leaving it pending until the next chapter sync.
    import subprocess
    p = subprocess.run(["git", "-C", str(b.path), "status", "--porcelain"],
                       capture_output=True, text=True, timeout=30)
    assert p.stdout.strip() == "", "workdir must be clean after the delete snapshot"
    log = subprocess.run(["git", "-C", str(b.path), "log", "--oneline", "-1"],
                         capture_output=True, text=True, timeout=30)
    assert "chore: manual snapshot" in log.stdout
    show = subprocess.run(["git", "-C", str(b.path), "show", "--name-status", "--format=", "HEAD"],
                          capture_output=True, text=True, timeout=30)
    assert "manuscript/ch01.md" in show.stdout and "D" in show.stdout


def test_x3_export_then_tamper(cli, seeded_bundle, tmp_path):
    """T3.X3 — export+zip-slip: the real export imports cleanly; a tampered twin doesn't."""
    b = seeded_bundle
    out = tmp_path / "clean.zip"
    assert_ok(cli("project_export", bundle=b, args={"out": str(out)}))
    dest = tmp_path / "clean_copy.authr"
    r = cli("project_import", args={"zip": str(out), "dest": str(dest)})
    assert_ok(r, "import of clean export")

    # Tamper: splice a traversal entry into the clean archive.
    import zipfile

    tampered = tmp_path / "tampered.zip"
    with zipfile.ZipFile(out) as src, zipfile.ZipFile(tampered, "w") as dst:
        for item in src.infolist():
            dst.writestr(item.filename, src.read(item.filename))
        dst.writestr("../../../escaped.txt", "pwned")
    assert_error(cli("project_import", args={"zip": str(tampered), "dest": str(tmp_path / "evil.authr")}),
                 code="ZIP_SLIP_VIOLATION")


def test_x4_speaker_update_rewrites_markdown(cli, seeded_bundle):
    """T3.X4 — speaker cascade+markdown: renaming display_name rewrites chNN.md."""
    b = seeded_bundle
    assert "**Kira Voss**" in (b.path / "manuscript" / "ch01.md").read_text()
    assert_ok(cli("character_upsert", bundle=b, args={
        "character_key": "CHAR_KIRA", "display_name": "Kira Voss-Okafor",
    }))
    md = (b.path / "manuscript" / "ch01.md").read_text()
    assert '**Kira Voss-Okafor**: "We have maybe three minutes' in md


def test_x5_cloud_guard_and_export(cli, tmp_path):
    """T3.X5 — cloud+export: OneDrive bundle exports cleanly, WAL stays 0 bytes."""
    root = tmp_path / "OneDrive"
    root.mkdir()
    b_path = root / "Novel.authr"
    assert_ok(cli("project_init", args={"path": str(b_path), "title": "Cloud Export"}))
    assert_ok(cli("chapter_create", bundle=b_path, args={
        "book_id": 1, "chapter_number": 1, "title": "Rain",
    }))
    out = tmp_path / "cloud_export.zip"
    r = cli("project_export", bundle=b_path, args={"out": str(out)})
    data = assert_ok(r, "export")
    assert data["bytes"] > 0
    wal = b_path / "project.db-wal"
    assert not wal.exists() or wal.stat().st_size == 0
    import zipfile

    names = zipfile.ZipFile(out).namelist()
    assert "project.db" in names
    assert "project.db-wal" not in names
    assert ".authr_lock" not in names


def test_x6_sync_bridge_invalidates_audio_hash(cli, seeded_bundle):
    """T3.X6 — FTS+bridge: text edits propagate to FTS, shadow and audio cache."""
    b = seeded_bundle
    # Simulate a rendered segment: give it an audio hash via direct DB (Phase-4
    # synthesis is out of scope; the cache key column is §6.1 contract surface).
    segs = assert_ok(cli("segment_list", bundle=b, args={"chapter_id": 1}))["segments"]
    target = segs[0]
    conn = b.db()
    try:
        conn.execute("UPDATE chapter_segments SET audio_hash='deadbeef' WHERE segment_id=?",
                     [target["segment_id"]])
        conn.commit()
    finally:
        conn.close()

    new_text = "The station hummed with a dying, dormant chorus."
    assert_ok(cli("sync_lexical_ast_mutation", bundle=b, args={
        "chapter_id": 1,
        "upserts": [dict(target, text_content=new_text)],
        "deletes": [],
    }))

    row = b.query("SELECT text_content, audio_hash FROM chapter_segments WHERE segment_id=?",
                  [target["segment_id"]])[0]
    assert row["text_content"] == new_text
    assert row["audio_hash"] is None, "audio_hash not invalidated on text change"

    hits = assert_ok(cli("segment_search", bundle=b, args={"query": "chorus"}))["hits"]
    assert any(h["segment_id"] == target["segment_id"] for h in hits)

    ast = json.loads((b.path / "manuscript" / "ch01.ast.json").read_text())
    match = [s for s in ast if s["segment_id"] == target["segment_id"]]
    assert match and match[0]["text_content"] == new_text
