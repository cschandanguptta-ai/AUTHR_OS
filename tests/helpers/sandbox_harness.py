"""AUTHR_OS — path sandboxing & zip-slip verifier drivers."""

import json
import subprocess
from pathlib import Path


def normalize(cli, raw_path: str):
    """sandbox_normalize (contract #34): pure verdict for a raw path string."""
    return cli("sandbox_normalize", args={"path": raw_path})


def assert_normalized(result, expected: str):
    assert result.ok, f"normalize failed: {result.payload}"
    assert result.data["normalized"] == expected, (
        f"expected {expected!r}, got {result.data['normalized']!r}"
    )
    return result.data


def make_evil_zip(out: Path, evil_names, base_entries=None):
    """Craft an archive containing payload entry names (Zip-Slip vectors)."""
    import zipfile

    base_entries = base_entries or {"manifest.json": '{"schema_version": 1}'}
    with zipfile.ZipFile(out, "w") as zf:
        for name, data in base_entries.items():
            zf.writestr(name, data)
        for evil in evil_names:
            zf.writestr(evil, "pwned-by-zip-slip")
    return out


def try_import(cli, zip_path, dest):
    return cli("project_import", args={"zip": str(zip_path), "dest": str(dest)})
