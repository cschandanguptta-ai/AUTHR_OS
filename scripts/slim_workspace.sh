#!/usr/bin/env bash
# slim_workspace.sh — remove build-time-only state so the task file panel
# indexes the ~350 real project files instead of thousands of dependency and
# artifact entries.
#
# Removes:
#   node_modules/            (11k+ files; reinstalled from warm npm cache in
#                             ~5-15 s by restore_env.sh whenever needed)
#   src-tauri/target/*       (cargo artifacts; live externally via
#                             /home/z/my-project/.cargo/config.toml →
#                             /home/z/.authr_build/target)
# Keeps:
#   src-tauri/target/release/authr   (the force-tracked release binary used
#                                     by pytest and the deliverable packager)
#   dist/                            (26 files; consumed by build_deliverables)
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP="$(dirname "$HERE")"

rm -rf "$APP/node_modules"

if [ -d "$APP/src-tauri/target" ]; then
  find "$APP/src-tauri/target" -mindepth 1 -maxdepth 1 \
    ! -name 'release' -exec rm -rf {} +
  if [ -d "$APP/src-tauri/target/release" ]; then
    find "$APP/src-tauri/target/release" -mindepth 1 -maxdepth 1 \
      ! -name 'authr' -exec rm -rf {} +
  fi
fi

NM=0; [ -d "$APP/node_modules" ] && NM=$(find "$APP/node_modules" -type f | wc -l)
TG=0; [ -d "$APP/src-tauri/target" ] && TG=$(find "$APP/src-tauri/target" -type f | wc -l)
echo "slim_workspace: node_modules files=$NM, target files=$TG (expect 0 and 1)"
