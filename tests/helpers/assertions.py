"""AUTHR_OS — shared assertion helpers for the E2E suite."""

import json
import sqlite3

from . import schema_reference as sr


class CLIResult:
    """Parsed result of one CLI invocation."""

    def __init__(self, exit_code, payload, stdout, stderr):
        self.exit_code = exit_code
        self.payload = payload
        self.ok = bool(payload.get("ok")) if isinstance(payload, dict) else False
        self.data = payload.get("data") if isinstance(payload, dict) else None
        self.error = payload.get("error") if isinstance(payload, dict) else None
        self.stdout = stdout
        self.stderr = stderr

    @property
    def code(self):
        return (self.error or {}).get("code")

    @property
    def category(self):
        return (self.error or {}).get("category")

    @property
    def message(self):
        return (self.error or {}).get("message", "")


def assert_ok(result, what="command"):
    assert result.ok, f"{what} failed: {result.payload}"
    assert result.exit_code == 0, f"{what} exit code {result.exit_code}"
    assert "data" in result.payload, f"{what} envelope missing `data`"
    return result.data


def assert_error(result, code=None, category=None, what="command"):
    assert not result.ok, f"{what} unexpectedly succeeded: {result.payload}"
    assert result.exit_code == 1, f"{what} exit code {result.exit_code}"
    err = result.error or {}
    assert "code" in err and "message" in err and "category" in err, (
        f"{what} malformed error envelope: {err}"
    )
    assert err["category"] in ("storage", "bundle", "security"), (
        f"{what} invalid category {err['category']}"
    )
    if code is not None:
        assert err["code"] == code, f"{what}: expected {code}, got {err['code']} ({err['message']})"
    if category is not None:
        assert err["category"] == category, (
            f"{what}: expected category {category}, got {err['category']}"
        )
    return result.error


def assert_single_line_json(stdout):
    """Contract §2: exactly one line of JSON on stdout."""
    lines = [ln for ln in stdout.strip().splitlines() if ln.strip()]
    assert len(lines) == 1, f"expected 1 stdout line, got {len(lines)}"
    return json.loads(lines[0])


def connect(db_path):
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    return conn


def assert_tables(db_path, expected=None):
    expected = expected or sr.EXPECTED_TABLES
    conn = connect(db_path)
    try:
        rows = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
        ).fetchall()
        names = {r["name"] for r in rows}
    finally:
        conn.close()
    for t in expected:
        assert t in names, f"missing table `{t}` (have {sorted(names)})"
    return names


def assert_indexes(db_path):
    conn = connect(db_path)
    try:
        rows = conn.execute("SELECT name FROM sqlite_master WHERE type='index'").fetchall()
        names = {r["name"] for r in rows}
    finally:
        conn.close()
    for ix in sr.EXPECTED_INDEXES:
        assert ix in names, f"missing index `{ix}` (have {sorted(names)})"


def assert_triggers(db_path):
    conn = connect(db_path)
    try:
        rows = conn.execute("SELECT name, sql FROM sqlite_master WHERE type='trigger'").fetchall()
        by_name = {r["name"]: r["sql"] or "" for r in rows}
    finally:
        conn.close()
    for trig, fragment in sr.TRIGGER_SQL.items():
        assert trig in by_name, f"missing trigger `{trig}`"
        assert fragment in by_name[trig], (
            f"trigger `{trig}` body mismatch: expected fragment `{fragment}`"
        )


def assert_fts_tokenizer(db_path):
    conn = connect(db_path)
    try:
        sql = conn.execute(
            "SELECT sql FROM sqlite_master WHERE name='chapter_segments_fts'"
        ).fetchone()["sql"]
    finally:
        conn.close()
    assert sr.FTS_TOKENIZER in sql, f"FTS tokenizer mismatch: {sql}"


def assert_columns(db_path, table, columns=None):
    columns = columns or sr.TABLE_COLUMNS[table]
    conn = connect(db_path)
    try:
        info = conn.execute(f"PRAGMA table_info({table})").fetchall()
        names = [r["name"] for r in info]
    finally:
        conn.close()
    for c in columns:
        assert c in names, f"table `{table}` missing column `{c}` (have {names})"


def query(db_path, sql, params=()):
    conn = connect(db_path)
    try:
        return [dict(r) for r in conn.execute(sql, params).fetchall()]
    finally:
        conn.close()
