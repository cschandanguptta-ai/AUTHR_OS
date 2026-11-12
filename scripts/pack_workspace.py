#!/usr/bin/env python3
"""pack_workspace.py — rebuild download/AUTHR_OS_workspace.zip from git history.

The workspace materials (both UX assessment rounds + the agent execution
workflow diagram) are NOT loose files in the working tree anymore — they were
consolidated into this zip at 9473f9f and removed from the tree. Their content
is preserved in git history; this script extracts it via `git archive` and
repacks, so the workspace zip is reproducible from the repository alone.

Usage:
  python3 scripts/pack_workspace.py [REV]     # default REV: 9473f9f^ (1cfee76)

Output: download/AUTHR_OS_workspace.zip  (internal top folder: AUTHR_OS_WORKSPACE/)
"""
import hashlib
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

REV = sys.argv[1] if len(sys.argv) > 1 else "9473f9f^"
APP = Path(__file__).resolve().parent.parent          # AUTHR_OS app root
REPO = APP.parent.parent                              # git repo root (/home/z/my-project)
OUT = REPO / "download" / "AUTHR_OS_workspace.zip"
TOP = "AUTHR_OS_WORKSPACE"

if not (REPO / ".git").is_dir():
    sys.exit("pack_workspace.py must run inside the git repository "
             f"(expected {REPO} to be the git root) — an extracted release zip "
             "has no history; the workspace zip ships prebuilt instead.")

WORKSPACE_README = """# AUTHR_OS_WORKSPACE - UX assessments & diagrams

Companion pack to AUTHR_OS_v3.0.0-PROD.zip / AUTHR_OS_docs.zip.

## assessments/
- round1_UX_Assessment/   - first audit: D1-D7 (friction F-01..F-24,
  roadmap R-01..R-18 - all 18 implemented and gate-verified)
- round2_UX_ReAssessment/ - second comprehensive audit (8 domains A-H):
  D1-D7 + IMPLEMENTATION_REPORT (register RA-01..RA-08 cleared,
  229/229 E2E, live verification shots in evidence/)

## diagrams/
- agent_execution_workflow.mmd / .svg / .png - Mermaid source + renders
  of the agent execution workflow.

The workspace cockpit/user-flow designs (ui_workspace_design, user_work_flow)
ship inside the two AUTHR_OS zips under AUTHR_OS/docs/design/ and are not
duplicated here.
"""

# 1) extract the two source folders from git history into a temp dir
tmp = Path(tempfile.mkdtemp(prefix="authr_wspack_"))
arc = subprocess.run(
    ["git", "archive", REV, "--", "download/assessments", "download/design_assets"],
    cwd=REPO, check=True, stdout=subprocess.PIPE,
)
subprocess.run(["tar", "-x", "-C", str(tmp)], input=arc.stdout, check=True)

# 2) collect entries: assessments (all files) + unique diagrams only
entries: list[tuple[Path, str]] = []
for sub in ("round1_UX_Assessment", "round2_UX_ReAssessment"):
    base = tmp / "download" / "assessments" / sub
    for p in sorted(base.rglob("*")):
        if p.is_file():
            entries.append((p, f"{TOP}/assessments/{sub}/{p.relative_to(base).as_posix()}"))
for name in ("agent_execution_workflow.mmd", "agent_execution_workflow.svg", "agent_execution_workflow.png"):
    p = tmp / "download" / "design_assets" / name
    if p.is_file():
        entries.append((p, f"{TOP}/diagrams/{name}"))

# 3) pack (the 4 ui_workspace_design/user_work_flow files are byte-duplicates
#    of AUTHR_OS/docs/design/ and intentionally excluded)
with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
    zi = zipfile.ZipInfo(f"{TOP}/README.md")
    zi.compress_type = zipfile.ZIP_DEFLATED
    zf.writestr(zi, WORKSPACE_README)
    for src, name in entries:
        zi = zipfile.ZipInfo(name)
        zi.compress_type = zipfile.ZIP_DEFLATED
        zf.writestr(zi, src.read_bytes())

h = hashlib.sha256(OUT.read_bytes()).hexdigest()
print(f"{OUT.name}: {OUT.stat().st_size/1e6:.2f} MB  sha256={h[:16]}…  files={len(entries)+1}  rev={REV}")
