#!/usr/bin/env python3
"""AUTHR_OS — Master E2E Test Runner (TEST_INFRA §3).

Standalone CLI over pytest:
    python tests/run_e2e.py                 # full suite (4 tiers)
    python tests/run_e2e.py --tier=1        # one tier
    python tests/run_e2e.py --feature=sqlite_schema
    python tests/run_e2e.py -v              # verbose, per-test durations
    python tests/run_e2e.py --json          # machine-readable summary
    python tests/run_e2e.py --bail          # stop at first failure
"""

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

TESTS_DIR = Path(__file__).resolve().parent

FEATURE_MAP = {
    "keychain": "tier1_features/test_t1_keychain.py",
    "sandbox": "tier1_features/test_t1_sandbox.py",
    "sqlite_schema": "tier1_features/test_t1_sqlite_schema.py",
    "indexes": "tier1_features/test_t1_indexes.py",
    "fts5_triggers": "tier1_features/test_t1_fts5_triggers.py",
    "bundle_lifecycle": "tier1_features/test_t1_bundle_lifecycle.py",
    "git_integration": "tier1_features/test_t1_git_integration.py",
    "shadow_markdown": "tier1_features/test_t1_shadow_markdown.py",
    "ipc_contracts": "tier1_features/test_t1_ipc_contracts.py",
    "audio_surface": "tier1_features/test_t1_audio_surface.py",
    "tts_engines": "tier1_features/test_t1_tts_engines.py",
    "dsp_mastering": "tier1_features/test_t1_dsp_mastering.py",
    "rag": "tier1_features/test_t1_rag.py",
    "local_ai": "tier1_features/test_t1_local_ai.py",
    "publish": "tier1_features/test_t1_publish.py",
    "release_bench": "tier1_features/test_t1_release_bench.py",
    "craft": "tier1_features/test_t1_craft.py",
    "kdp": "tier1_features/test_t1_kdp.py",
    "audio_ai_boundaries": "tier2_boundaries/test_t2_audio_ai_boundaries.py",
    "audio_rag_scenario": "tier4_scenarios/test_t4_audio_rag_scenario.py",
    "path_traversal": "tier2_boundaries/test_t2_path_traversal.py",
    "windows_prefixes": "tier2_boundaries/test_t2_windows_prefixes.py",
    "zip_slip": "tier2_boundaries/test_t2_zip_slip.py",
    "schema_constraints": "tier2_boundaries/test_t2_schema_constraints.py",
    "foreign_key_violations": "tier2_boundaries/test_t2_foreign_key_violations.py",
    "cascade_deletions": "tier2_boundaries/test_t2_cascade_deletions.py",
    "publish_boundaries": "tier2_boundaries/test_t2_publish_boundaries.py",
    "stale_lease_recovery": "tier2_boundaries/test_t2_stale_lease_recovery.py",
    "cloud_wal_truncation": "tier2_boundaries/test_t2_cloud_wal_truncation.py",
    "cross_interactions": "tier3_interactions/test_t3_cross_interactions.py",
    "author_scenarios": "tier4_scenarios/test_t4_author_scenarios.py",
    "one_person_publisher": "tier4_scenarios/test_t4_one_person_publisher.py",
}

TIER_GLOBS = {
    1: "tier1_features/test_t1_*.py",
    2: "tier2_boundaries/test_t2_*.py",
    3: "tier3_interactions/test_t3_*.py",
    4: "tier4_scenarios/test_t4_*.py",
}


def main() -> int:
    ap = argparse.ArgumentParser(description="AUTHR_OS master E2E runner")
    ap.add_argument("--tier", type=int, choices=[1, 2, 3, 4], help="run one tier")
    ap.add_argument("--feature", choices=sorted(FEATURE_MAP), help="run one feature module")
    ap.add_argument("-v", "--verbose", action="store_true", help="per-test verbose output")
    ap.add_argument("--json", action="store_true", help="machine-readable JSON summary")
    ap.add_argument("--bail", action="store_true", help="stop on first failure (-x)")
    opts = ap.parse_args()

    targets: list[str] = []
    if opts.feature:
        targets.append(str(TESTS_DIR / FEATURE_MAP[opts.feature]))
    elif opts.tier:
        # Expand glob explicitly so the JSON summary can count modules.
        import glob as _g

        targets = sorted(
            _g.glob(str(TESTS_DIR / TIER_GLOBS[opts.tier]))
        )
    else:
        targets = [
            str(TESTS_DIR / d)
            for d in ["tier1_features", "tier2_boundaries", "tier3_interactions", "tier4_scenarios"]
        ]

    argv = [sys.executable, "-m", "pytest", "--rootdir", str(TESTS_DIR.parent),
            "-q", "--tb=short", *targets]
    if opts.verbose:
        argv.insert(-len(targets), "-v")
    if opts.bail:
        argv.insert(-len(targets), "-x")

    t0 = time.time()
    proc = subprocess.run(argv, capture_output=True, text=True, env=None)
    duration = time.time() - t0

    if opts.json:
        # Derive per-module counts from pytest's last line, e.g.
        # "101 passed, 3 warnings in 2.7s".
        tail = proc.stdout.strip().splitlines()[-1] if proc.stdout.strip() else ""
        passed = failed = 0
        prev = ""
        for token in tail.replace(",", " ").split():
            if token in ("passed", "failed"):
                digits = "".join(ch for ch in prev if ch.isdigit())
                if digits:
                    if token == "passed":
                        passed += int(digits)
                    else:
                        failed += int(digits)
            prev = token
        print(json.dumps({
            "suite": "AUTHR_OS E2E",
            "exit_code": proc.returncode,
            "passed": passed,
            "failed": failed,
            "duration_s": round(duration, 2),
            "tier": opts.tier,
            "feature": opts.feature,
            "modules": len(targets),
        }))
        # stdout must stay pure JSON in --json mode (capture_output above
        # already keeps pytest output out of our stdout).
        return proc.returncode

    print(proc.stdout)
    if proc.stderr.strip():
        print(proc.stderr, file=sys.stderr)
    return proc.returncode


if __name__ == "__main__":
    sys.exit(main())
