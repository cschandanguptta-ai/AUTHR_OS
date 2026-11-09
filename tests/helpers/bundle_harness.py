"""AUTHR_OS — .authr bundle container harness: creation, inspection, tampering."""

import json
import os
import shutil
import socket
import sqlite3
import subprocess
import zipfile
from pathlib import Path

from . import schema_reference as sr


def _as_path(p) -> Path:
    """Accept str, Path, or Bundle."""
    return Path(getattr(p, "path", p))


class Bundle:
    """Wrapper around an on-disk .authr bundle produced by project_init."""

    def __init__(self, path: Path):
        self.path = Path(path)

    # ── layout ────────────────────────────────────────────────────────────────
    def exists(self, rel: str) -> bool:
        return (self.path / rel).exists()

    def dirs_exist(self):
        return all(
            self.exists(d)
            for d in ["manuscript", "audio/segments", "audio/sfx_custom", "assets", "exports"]
        )

    def list_files(self):
        out = []
        for p in sorted(self.path.rglob("*")):
            if ".git" in p.parts:
                continue
            if p.is_file():
                out.append(str(p.relative_to(self.path)))
        return out

    # ── manifest / lock ───────────────────────────────────────────────────────
    def manifest(self):
        return json.loads((self.path / "manifest.json").read_text())

    def lock(self):
        p = self.path / ".authr_lock"
        if not p.exists():
            return None
        return json.loads(p.read_text())

    def write_lock(self, pid=999_999, hostname="OTHER-WORKSTATION", user="other",
                   locked_at="2026-11-06T08:00:00Z", lease_expires_at="2026-11-06T08:02:00Z",
                   session_id="fixed-session"):
        lease = {
            "pid": pid,
            "hostname": hostname,
            "user": user,
            "locked_at": locked_at,
            "lease_expires_at": lease_expires_at,
            "session_id": session_id,
        }
        (self.path / ".authr_lock").write_text(json.dumps(lease))
        return lease

    def write_live_local_lock(self, ttl_seconds=60):
        """A lease that classifies as ACTIVE: live pid on this host, fresh TTL."""
        return self.write_lock(
            pid=os.getpid(),
            hostname=socket.gethostname(),
            user=os.environ.get("USER", "pytest"),
            lease_expires_at=fresh_expiry(ttl_seconds),
        )

    def write_dead_local_lock(self):
        return self.write_lock(
            pid=999_999,
            hostname=socket.gethostname(),
        )

    def clear_lock(self):
        p = self.path / ".authr_lock"
        if p.exists():
            p.unlink()

    def corrupt_lock(self):
        (self.path / ".authr_lock").write_text("{ this is not json !!!")

    # ── database ──────────────────────────────────────────────────────────────
    @property
    def db_path(self):
        return self.path / "project.db"

    def db(self):
        conn = sqlite3.connect(str(self.db_path))
        conn.row_factory = sqlite3.Row
        # Mirror the engine's connection settings (PRAGMAs, TEST_INFRA §5.1).
        conn.execute("PRAGMA foreign_keys = ON")
        return conn

    def query(self, sql, params=()):
        conn = self.db()
        try:
            return [dict(r) for r in conn.execute(sql, params).fetchall()]
        finally:
            conn.close()

    def wal_bytes(self):
        p = self.path / "project.db-wal"
        return p.stat().st_size if p.exists() else 0

    # ── shadow layer ──────────────────────────────────────────────────────────
    def shadow_md(self, nn: int):
        return (self.path / "manuscript" / f"ch{nn:02d}.md").read_text()

    def shadow_ast(self, nn: int):
        return json.loads((self.path / "manuscript" / f"ch{nn:02d}.ast.json").read_text())

    def shadow_tmp_leftovers(self):
        return [f for f in self.list_files() if f.endswith(".tmp")]

    # ── zip helpers ───────────────────────────────────────────────────────────
    def export_zip(self, out: Path):
        out = Path(out)
        if out.exists():
            out.unlink()
        return out

    def tampered_zip(self, out: Path, evil_names, valid_entries=None):
        """Craft an archive mixing valid entries with Zip-Slip payload names."""
        out = Path(out)
        valid_entries = valid_entries or {"manifest.json": json.dumps(self.manifest())}
        with zipfile.ZipFile(out, "w") as zf:
            for name, data in valid_entries.items():
                zf.writestr(name, data)
            for evil in evil_names:
                zf.writestr(evil, "pwned")
        return out


def fresh_expiry(seconds=sr.LEASE_TTL_SECONDS):
    import datetime

    return (datetime.datetime.now(datetime.timezone.utc)
            + datetime.timedelta(seconds=seconds)).strftime("%Y-%m-%dT%H:%M:%SZ")


def stale_expiry(seconds_before=300):
    import datetime

    return (datetime.datetime.now(datetime.timezone.utc)
            - datetime.timedelta(seconds=seconds_before)).strftime("%Y-%m-%dT%H:%M:%SZ")


def make_zip(out: Path, entries: dict):
    """entries: {name: bytes|str}"""
    out = Path(out)
    with zipfile.ZipFile(out, "w") as zf:
        for name, data in entries.items():
            zf.writestr(name, data)
    return out


def zip_from_bundle(bundle: Bundle, out: Path):
    return shutil.make_archive(str(out.with_suffix("")), "zip", root_dir=bundle.path)
