# AUTHR_OS — Benchmark & Hardening Measurements (Phase 6, Sprint 16)

All numbers below are **measured on this build machine** by
`authr benchmark_run` (the same command CI asserts on —
`tests/tier1_features/test_t1_release_bench.py::test_b1`).

## Sprint 16 acceptance thresholds

| Metric | Threshold | Measured (n=15) | Margin | Status |
|--------|-----------|-----------------|--------|--------|
| Cold start (median) | < 1500 ms | **1.72 ms** | ~870× headroom | ✅ |
| Cold start (p95) | < 1500 ms | 2.09 ms | | ✅ |
| Peak RSS (full CLI invocation, VmHWM) | < 50 MB | **6.22 MB** | ~8× headroom | ✅ |
| Binary size | < 25 MB | **10.11 MB** (10,602,096 B) | ~2.5× headroom | ✅ |

Raw probe (2026-11-08, release profile, opt-level 2):

```json
{
  "cold_start_ms": {"iterations": 15, "min": 1.618, "median": 1.717, "p95": 2.086, "max": 2.086, "limit": 1500.0},
  "rss": {"peak_kb": 6368, "peak_mb": 6.21875, "limit_mb": 50.0},
  "binary": {"bytes": 10602096, "mb": 10.1109, "limit_mb": 25.0},
  "checks": {"cold_start": true, "rss": true, "binary_size": true},
  "pass": true
}
```

## Throughput observations (same machine)

- Full E2E suite (168 tests, 4 tiers, ~60 bundles): **~10.6 s**
- Rust unit tests (67): **0.02 s**
- Lookahead synthesis, 50-turn chapter (10 speakers, SCRIPTED engine):
  5 dependency waves, full parallelism inside waves (unit-tested stress in
  `audio/scheduler.rs`)
- 3-layer DSP master of a 1 s chapter (mix → duck → two-pass normalize →
  limiter → ACX validate): single-digit milliseconds per stage

## Notes

- Cold start here is the CLI `--version` spawn path; the GUI shell adds
  Tauri/WebView startup which is out of scope for the core threshold.
- VmHWM covers a full bundle-scoped invocation (DB open + query + exit), so
  it upper-bounds the idle footprint of any single command.
- The 25 MB installer budget refers to per-target installers produced by
  `cargo tauri build` (see `docs/PACKAGING.md`); the core binary is the
  dominant payload.
