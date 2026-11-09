#!/usr/bin/env python3
"""Build AUTHR_OS deliverable zips from the current git tree.

Two artifacts (written next to the repo in download/):
  AUTHR_OS_v3.0.0-PROD.zip : full tracked tree + dist/ frontend build + prebuilt
                             release binary — unpack and run; nothing else needed.
  AUTHR_OS_docs.zip        : full source + complete docs (decisions/ included),
                             no build artifacts.

Both are rebuilt from `git ls-files` (so git is the single source of truth) and
carry directory placeholder entries like the previous bundles.
"""
import subprocess
import sys
import zipfile
from pathlib import Path

# APP may be passed explicitly; default = <script_dir>/../download/AUTHR_OS
_here = Path(__file__).resolve().parent
APP = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else (_here.parent / "download" / "AUTHR_OS").resolve()
OUT = APP.parent
assert (APP / "src-tauri" / "Cargo.toml").exists(), f"not an AUTHR_OS app dir: {APP}"

tracked = subprocess.run(
    ["git", "ls-files"], capture_output=True, text=True, check=True, cwd=APP
).stdout.splitlines()
tracked = [t for t in tracked if t.strip()]

binary = "src-tauri/target/release/authr"
assert binary in tracked, "release binary must stay force-tracked"

dist_files = sorted(p for p in (APP / "dist").rglob("*") if p.is_file())
assert dist_files, "dist/ must be built before packaging"

prod_name = "AUTHR_OS_v3.0.0-PROD.zip"
docs_name = "AUTHR_OS_docs.zip"

dir_placeholders = sorted({f.parent for f in (APP).rglob("*") if f.is_dir()})


def add_dir_entries(zf: zipfile.ZipFile, rel_dirs):
    for d in sorted(rel_dirs):
        name = f"AUTHR_OS/{d}/" if d else "AUTHR_OS/"
        zi = zipfile.ZipInfo(name)
        zi.external_attr = 0o755 << 16 | 0x10
        zf.writestr(zi, b"")


def build(name: str, rel_files, extra_rel):
    dest = OUT / name
    with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
        dirs = {"", *(Path(f).parent.as_posix() for f in list(rel_files) + extra_rel)}
        add_dir_entries(zf, dirs)
        for rel in list(rel_files) + extra_rel:
            src = APP / rel
            zi = zipfile.ZipInfo(f"AUTHR_OS/{rel}")
            zi.external_attr = (0o755 if rel == binary else 0o644) << 16
            zi.compress_type = zipfile.ZIP_DEFLATED
            zf.writestr(zi, src.read_bytes())
    return dest


prod_extra = [f"dist/{p.relative_to(APP / 'dist').as_posix()}" for p in dist_files]
prod_rel = sorted(set(tracked) | set(prod_extra))
docs_rel = sorted(set(tracked) - {binary} - {f"dist/{f}" for f in prod_extra})
docs_rel = [r for r in docs_rel if not r.startswith("dist/")]

p1 = build(prod_name, prod_rel, [])
p2 = build(docs_name, docs_rel, [])

import hashlib
for f in (p1, p2):
    h = hashlib.sha256(f.read_bytes()).hexdigest()
    print(f"{f.name}: {f.stat().st_size/1e6:.2f} MB  sha256={h[:16]}…")
