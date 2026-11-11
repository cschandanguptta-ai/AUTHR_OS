"""Tier 2 — BVA: Zip-Slip archive defense (crafted archives, project_import).

Test IDs T2.ZS1–T2.ZS5 (5 tests).
"""

from helpers.assertions import assert_error, assert_ok
from helpers.sandbox_harness import make_evil_zip, try_import


def _import_ok_manifest(tmp_path, name="base.authr"):
    """Craft a minimal-but-valid bundle zip (manifest + project.db absent → still rejected)."""
    import json

    manifest = json.dumps({
        "schema_version": 1, "project_id": "00000000-0000-4000-8000-000000000000",
        "title": "Archive Probe", "author": "T",
        "created_at": 1730880000, "last_modified_at": 1730880000,
        "authr_version": "3.0.0-PROD",
    })
    return {"manifest.json": manifest}


def test_zs1_classic_traversal_entry(cli, tmp_path):
    evil = make_evil_zip(tmp_path / "z1.zip", ["../evil.txt"], _import_ok_manifest(tmp_path))
    assert_error(try_import(cli, evil, tmp_path / "d1.authr"),
                 code="ZIP_SLIP_VIOLATION", category="security")


def test_zs2_deep_nested_escape(cli, tmp_path):
    evil = make_evil_zip(tmp_path / "z2.zip", ["a/b/c/../../../../evil.txt"], _import_ok_manifest(tmp_path))
    assert_error(try_import(cli, evil, tmp_path / "d2.authr"),
                 code="ZIP_SLIP_VIOLATION", category="security")


def test_zs3_root_absolute_entry(cli, tmp_path):
    evil = make_evil_zip(tmp_path / "z3.zip", ["/root/owned.txt"], _import_ok_manifest(tmp_path))
    assert_error(try_import(cli, evil, tmp_path / "d3.authr"),
                 code="ZIP_SLIP_VIOLATION", category="security")


def test_zs4_windows_drive_letter_entry(cli, tmp_path):
    evil = make_evil_zip(tmp_path / "z4.zip", ["C:\\evil.txt"], _import_ok_manifest(tmp_path))
    assert_error(try_import(cli, evil, tmp_path / "d4.authr"),
                 code="ZIP_SLIP_VIOLATION", category="security")


def test_zs5_forward_slash_drive_letter_and_slip_combo(cli, tmp_path):
    evil = make_evil_zip(
        tmp_path / "z5.zip",
        ["C:/evil.txt", "ok/../../evil2.txt"],
        _import_ok_manifest(tmp_path),
    )
    assert_error(try_import(cli, evil, tmp_path / "d5.authr"),
                 code="ZIP_SLIP_VIOLATION", category="security")
