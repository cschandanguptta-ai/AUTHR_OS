"""Tier 1 — F13: Embedded Git init, .gitignore, scaffold & auto commits.

Test IDs T1.GT1–T1.GT5 (5 tests).
"""

from helpers.assertions import assert_ok
from helpers import git_harness as gh


def test_gt1_git_initialized_in_bundle(bundle):
    """T1.GT1 — .git/ exists inside the bundle after project_init."""
    b = bundle()
    assert gh.repo_initialized(b.path)


def test_gt2_gitignore_excludes_db_and_volatile(bundle):
    """T1.GT2 — .gitignore excludes project.db*, lock file, audio/, exports/."""
    text = gh.gitignore_text(bundle().path)
    for line in gh.GITIGNORE_LINES:
        assert line in text, f".gitignore missing `{line}`"


def test_gt3_initial_scaffold_commit(bundle):
    """T1.GT3 — initial scaffold commit exists with the contract message."""
    b = bundle()
    assert gh.commit_count(b.path) >= 1
    assert gh.commit_messages(b.path)[0] == gh.SCAFFOLD_MESSAGE


def test_gt4_chapter_sync_auto_commit(seeded_bundle):
    """T1.GT4 — segment ops create `Auto-sync chNN` commits."""
    msgs = gh.commit_messages(seeded_bundle)
    autos = [m for m in msgs if m.startswith("Auto-sync ch01")]
    assert len(autos) >= 1, f"no Auto-sync commits found in {msgs}"


def test_gt5_database_never_tracked(seeded_bundle):
    """T1.GT5 — git ls-files never contains project.db / WAL / SHM / lock."""
    tracked = gh.tracked_files(seeded_bundle)
    for banned in ["project.db", "project.db-wal", "project.db-shm", ".authr_lock"]:
        assert banned not in tracked, f"{banned} unexpectedly tracked"
    assert "manuscript/ch01.md" in tracked
