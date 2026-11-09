# FORENSIC_AUDIT.md — Core System Optimization & Forensic Integrity Audit (Phase 7)

**Date**: 2026-11-09 · **Scope**: full production tree (`src-tauri/src`, `src`,
`scripts`, `tests`) · **Method**: exhaustive marker sweep → manual
classification → fix → regression test → full gate re-run. Every claim below
cites a file/line or a command that reproduces it.

## 1. Verdict

**Production-ready: PASS.** No hidden mock, stub, or fake-success path exists
in the shipped binary. Every "mock/stub" hit in the sweep is either (a)
documented test infrastructure confined to `#[cfg(test)]`, (b) a documented,
availability-negotiated engine seam that fails closed with a canonical error,
or (c) an env-gated headless fallback — and finding (c) had a real production
reachability flaw that this audit **fixed** (§3.1). Zero `TODO`/`FIXME`/
`todo!()`/`unimplemented!()` markers exist in the Rust core.

## 2. Marker sweep (production code)

`grep -iE "mock|stub|fake|dummy|placeholder|todo!|unimplemented!|FIXME|XXX|HACK"`
over `src-tauri/src` and `src`:

| Location | Hit | Classification |
|---|---|---|
| `security/keychain.rs` | `MockKeyring`, `AUTHR_MOCK_KEYCHAIN` | **Legitimate seam, hardened** (§3.1) — env-gated file backend for headless CI; production builds now cannot reach it |
| `localai/llm.rs:236+` | `spawn_stub` loopback HTTP server | **Test-only** — `#[cfg(test)]`; exercises the *real* HTTP client (`http_post_json`) against a local socket, no model needed |
| `localai/llm.rs:18-97` | `OFFLINE_GRAMMAR` engine, self-described "placeholder prose" | **Production offline backend, relabeled** (§3.2) — seeded SplitMix64 generator; the real llama-server client (`llama_server_generate`) routes first whenever `AUTHR_LLAMA_SERVER`/`server_url` is set |
| `localai/tts.rs:57-65` | Kokoro ONNX path returns `AI_ENGINE_UNAVAILABLE` | **Honest availability seam** — fails closed with actionable guidance; never fakes output; offline SCRIPTED engine keeps the pipeline functional (ADR-0005) |
| `rag/embed.rs:107-118` | MiniLM ONNX resolver | Same as above — canonical `AI_ENGINE_UNAVAILABLE` without the model/feature |
| `src/**` (frontend) | HTML `placeholder=` attributes only | UI affordances, not code stubs |
| `src-tauri/tauri.conf.json:66` | `"REPLACE_WITH_AUTHR_UPDATER_PUBKEY…"` | **Open release-infra item** — updater is inert until a maintainer-generated ed25519 keypair exists (private key must never live in-repo); tracked in `open-questions.md`, not a code stub |

## 3. Findings fixed in this audit

### 3.1 K-1 (HIGH, security) — mock keychain reachable in production builds
`backend()` checked the env var first, so `AUTHR_MOCK_KEYCHAIN=1` downgraded
even `gui,os-keychain` builds to plaintext JSON credentials on disk.
**Fix**: mock is compiled/reachable only without the `os-keychain` feature
(`keychain.rs::backend`). CI behavior unchanged (tests run on default builds).

### 3.2 K-2 (MEDIUM, integrity) — desktop transport unwired (tech-debt T-1)
`bridge.ts` invoked contract names directly; the shell registers only
`authr_command` — every packaged-GUI call would fail with "unknown command".
**Fix**: `src/ipc/bridge.ts` routes through `authr_command` with
client-side bundle tracking (`project_init`/`project_import`/`project_open`),
mirroring the dev middleware. **Bonus gap fixed**: `scripts/ipc-proxy.mjs` did
not re-target `currentBundle` after `project_open` — opening an existing
bundle then editing would have written into the *default sandbox*.

### 3.3 K-3 (MEDIUM, correctness) — `segment_search` FTS injection (T-2)
Raw user query went into `MATCH ?1` un-sanitized; FTS5 syntax errored with
`STORAGE_ERROR` and relied on SQLite to reject injections. **Fix**: shared
sanitizer `rag::store::fts_sanitize` + parameter binding on both call sites;
`fts_search` also dropped its string-interpolated `MATCH '{q}'` for a bound
param. Regression tests: `test_ft7_hostile_queries_are_sanitized_not_errors`
(E2E) + existing rag hostile-input tests.

### 3.4 K-4 (MEDIUM, data loss) — slug prefix collisions (T-11)
Two titles sharing a 64-char slug prefix overwrote each other's export file.
**Fix** (`publish/mod.rs::slugify`): when truncation actually occurs, append
an 8-hex SHA-256 prefix of the full slug — collisions diverge, republishing
the same title stays idempotent. Unit-tested.

### 3.5 K-5 (LOW, correctness) — `chapter_delete` skipped bookkeeping (T-4)
No `after_commit()`, no git commit — deletions could vanish from git until
the next chapter sync. **Fix**: standard ordering (manifest touch → shadow
removal → `chore: manual snapshot` commit); response now carries `commit`.
Tests: `test_cd6` (E2E) + `test_x2` updated to the new invariant (workdir
clean, removal recorded in HEAD).

### 3.6 K-6 (LOW, robustness) — silent BLOB placeholder (T-5)
`row_json` rendered any BLOB as `"<N bytes>"`. **Fix**: fails loudly with
`InvalidColumnType` (no §5 projection selects a BLOB; binary loaders are
dedicated). Unit test in `db.rs`.

### 3.7 K-7 (LOW, robustness) — bulk-mutation argv ceiling
`--args` travels in argv; the OS caps a single argument (~128 KB on Linux),
so a large Lexical chapter mutation crashed the dev bridge with E2BIG —
reproduced: a 302 KB payload was impossible to deliver. **Fix**: additive
`--args-stdin` CLI transport (contract §1) + automatic proxy switch above
96 KB. The new binary ingests the 302 KB payload in ~7 ms; the baseline
binary cannot accept it at all.

### 3.8 K-8 (LOW, maintainability) — duplicated duration estimator (T-3)
`chars×55+400` existed in three places (`studio.rs:62,200`,
`publish/mod.rs:84`). Desync would silently diverge `audio_plan` timelines
from captions/EPUB SMIL/M4B marks. **Fix**: single canonical
`audio::scheduler::estimate_duration_ms`; all callers delegate. Locked-formula
unit test added.

### 3.9 K-9 (LOW, UX) — runner aliases (T-8) + missing CSP (T-6)
`run_e2e.py --feature` gained `publish`, `release_bench`,
`publish_boundaries`. `tauri.conf.json` CSP went from `null` to a restrictive
policy (`default-src 'self'; connect-src 'self' ipc: http://ipc.localhost; …`)
— verify Lexical/vite assets on a desktop target before public release
(manual step, GUI not exercisable headlessly).

## 4. Workerisation & latency (ADR-0008)

Profiling harness: `scripts/latency_profile.py` (deterministic workload,
per-command min/p50/p95/max; JSON out; `--compare` against a baseline JSON).
Paired run, identical workload/corpus (~800 chunks, scale 500), same machine:

| Command (p95, ms) | Baseline | After | Δ |
|---|---|---|---|
| `rag_search[hybrid]` | 7.40 | **6.36** | −14 % |
| `rag_search[dense]` | 6.78 | **5.65** | −17 % |
| `sync_lexical_ast_mutation[bulk]` (302 KB payload) | impossible (E2BIG) | **~7 ms** | transport fix |
| `publish_docx` | 20.5 | 12.3 | −40 % |
| everything else (25 commands) | — | parity ±0.5 ms | no regressions |

What changed under the hood (see ADR-0008): rayon-parallel deterministic
dense scan + parallel embedding, blob-only scan projection, targeted
`IN (…)` enrichment (the old path loaded the whole chunk table **twice** per
query). `rag_index` is flat at hashing-embedder scale because SQLite
upserts (+FTS triggers) dominate — parallelism pays off with embed-heavy
backends (MiniLM/ONNX seam). The `audio_synthesize` loop deliberately stays
sequential (continuity chain, invariant §1.4). Heavy work in the Tauri shell
now runs on a blocking worker thread.

True remaining hotspots at scale (documented, not debt): `project_export`
~3.1 s and `publish_epub` ~1.6 s on a 530-segment chapter — linear in
segment count, dominated by per-segment WAV/zip/SMIL assembly; `benchmark_run`
gates still pass with headroom.

## 5. Cleanup & debt

- **Dependencies**: verified **zero unused** crates (each of the 18 Cargo
  deps grep-confirmed via qualified-path usage; `rayon` added deliberately)
  and **zero unused** npm packages (`autoprefixer`/`postcss` are wired in
  `postcss.config.js`, `typescript` is the `tsc` build step).
- **Clippy**: lib reduced 17 → **0 warnings** (incl. BS.1770 constants,
  lookahead-limiter loop, sandbox prefix handling — all behavior-preserving,
  170/170 E2E re-run after).
- **Repo hygiene**: `tool-results/` dumps removed from git; `__pycache__`
  untracked earlier in Phase 7 prep; deliverable zips untracked.
- **tech-debt register**: T-1, T-2, T-3, T-4, T-5, T-6, T-8, T-9, T-11
  resolved this phase; T-7 previously resolved; T-10, T-12, T-13, T-14
  remain open (platform liveness, history portability, binary-in-git, CI —
  each needs a human/infra call).

## 6. Verification gate (all green, this machine)

| Gate | Result |
|---|---|
| `cargo build --release` | clean, 0 warnings |
| `cargo clippy --release` | 0 warnings |
| `cargo test --release --lib` | **70/70** (67 + 3 new) |
| `AUTHR_MOCK_KEYCHAIN=1 pytest` | **170/170** (168 + FT7 + CD6) |
| `npm run build` (tsc + vite) | clean |
| `benchmark_run` | **pass** — cold 1.9 ms / RSS 6.2 MB / binary 10.4 MB |
| `scripts/refresh_acceptance.py` | `test_results.json` regenerated from live runs |
