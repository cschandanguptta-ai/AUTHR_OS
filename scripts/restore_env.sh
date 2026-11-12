#!/usr/bin/env bash
# restore_env.sh — one-shot sandbox-recycle recovery for the AUTHR_OS workspace.
#
# The host sandbox periodically recycles: tracked files get mode-flipped
# (644<->755), ~/.cargo is emptied, node_modules/ and dist/ are wiped. This
# script restores a fully green build environment in one pass and is what the
# daily gatekeeper runs before the four acceptance gates.
#
# Usage: bash scripts/restore_env.sh          (from the AUTHR_OS app root, or anywhere)
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP="$(dirname "$HERE")"          # .../download/AUTHR_OS
REPO="$(dirname "$APP")"          # repo root (/home/z/my-project)

cd "$REPO" 2>/dev/null || cd "$APP"

# 1) Mode-drift: make git ignore worktree mode bits (recycle artifact class),
#    and normalize any current flips back to the canonical 644.
git config core.fileMode false 2>/dev/null || true
if [ -n "$(git diff --name-only 2>/dev/null | head -1)" ]; then
  git diff --name-only 2>/dev/null | xargs -r chmod 644 2>/dev/null
fi

# 2) Rust toolchain (cargo + clippy)
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  curl -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh \
    && sh /tmp/rustup-init.sh -y --profile minimal --default-toolchain stable >/dev/null 2>&1
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  rustup component add clippy 2>/dev/null || true
fi

# 3) Frontend dependencies
if [ ! -d "$APP/node_modules" ] || [ -z "$(ls -A "$APP/node_modules" 2>/dev/null)" ]; then
  (cd "$APP" && npm install --no-audit --no-fund --prefer-offline) || exit 1
fi

# 4) Frontend build (dist/)
if [ ! -f "$APP/dist/index.html" ]; then
  (cd "$APP" && npm run build) || exit 1
fi

# 5) Release binary (force-tracked; survives recycles, rebuilt only if absent)
if [ ! -x "$APP/src-tauri/target/release/authr" ]; then
  (cd "$APP/src-tauri" && cargo build --release) || exit 1
fi

echo "restore_env: OK (cargo=$(cargo --version 2>/dev/null | awk '{print $2}'), node_modules=$(ls "$APP/node_modules" 2>/dev/null | wc -l) pkgs, dist+binary present)"
