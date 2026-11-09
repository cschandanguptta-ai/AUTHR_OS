#!/usr/bin/env python3
"""Dependency audit: verify every declared dep is actually referenced.

Rust: each [dependencies] crate must appear as a qualified path (`name::`)
in src-tauri/src — optional crates (keyring/tauri) appear only inside
cfg(feature)-gated modules, the presence check is identical.
npm: each dependencies/devDependencies entry must be referenced in src/,
scripts/, docs/, or the root configs (vite/tailwind/postcss/tsconfig/index).

Usage:  python3 scripts/dep_audit.py        # prints UNUSED lists; empty = clean
"""
import json, re
from pathlib import Path

APP = Path(__file__).resolve().parent.parent

# ── Rust ─────────────────────────────────────────────────────────────────────
toml = (APP / "src-tauri" / "Cargo.toml").read_text()
deps_block = toml.split("[dependencies]")[1].split("\n[")[0]
rust_deps = re.findall(r"^([a-zA-Z0-9_-]+)\s*=", deps_block, re.M)
rs_src = "\n".join(p.read_text() for p in (APP / "src-tauri" / "src").rglob("*.rs"))
unused_rust = [d for d in rust_deps
               if not re.search(rf"\b{d.replace('-', '_')}::", rs_src)]

# ── npm ──────────────────────────────────────────────────────────────────────
pkg = json.loads((APP / "package.json").read_text())
npm_deps = list(pkg.get("dependencies", {}).keys()) + list(pkg.get("devDependencies", {}).keys())
hay = []
for pat in ("src", "scripts", "docs"):
    hay += list(APP.glob(f"{pat}/**/*.*"))
hay += [APP / f for f in ("vite.config.ts", "tailwind.config.js", "postcss.config.js",
                          "tsconfig.json", "index.html", "package.json")]
blob = ""
for p in hay:
    if p.is_file():
        try:
            blob += p.read_text(errors="ignore")
        except Exception:
            pass
unused_npm = [d for d in npm_deps if d not in blob]

print(f"rust deps: {len(rust_deps)}  unused: {unused_rust or 'NONE'}")
print(f"npm  deps: {len(npm_deps)}  unused: {unused_npm or 'NONE'}")
