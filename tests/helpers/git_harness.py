"""AUTHR_OS — embedded git repository validator (git CLI over .authr/.git)."""

import subprocess
from pathlib import Path


def _git(bundle, *args):
    b = getattr(bundle, "path", bundle)
    return subprocess.run(
        ["git", "-C", str(b), *args],
        capture_output=True, text=True, timeout=30,
    )


def commit_count(bundle) -> int:
    p = _git(bundle, "rev-list", "--count", "HEAD")
    return int(p.stdout.strip()) if p.returncode == 0 else 0


def commit_messages(bundle):
    p = _git(bundle, "log", "--format=%s")
    return p.stdout.strip().splitlines() if p.returncode == 0 else []


def latest_message(bundle):
    msgs = commit_messages(bundle)
    return msgs[0] if msgs else None


def tracked_files(bundle):
    p = _git(bundle, "ls-files")
    return set(p.stdout.strip().splitlines()) if p.returncode == 0 else set()


def gitignore_text(bundle):
    p = _as_path(bundle) / ".gitignore"
    return p.read_text() if p.exists() else ""


def status_dirty(bundle):
    p = _git(bundle, "status", "--porcelain")
    return bool(p.stdout.strip())


def repo_initialized(bundle) -> bool:
    return (_as_path(bundle) / ".git").exists()


def _as_path(p):
    from pathlib import Path as _P

    return _P(getattr(p, "path", p))


GITIGNORE_LINES = ["project.db", "project.db-wal", "project.db-shm", ".authr_lock", "audio/", "exports/"]
SCAFFOLD_MESSAGE = "chore: initial scaffold"
