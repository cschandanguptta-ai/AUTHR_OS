# tech-debt.md — known shortcuts, fragile code, scaling limits

Each item: what / where / why it matters / suggested direction. FACT unless
labeled. IDs are stable for reference from other docs and commits.

## T-1 — Desktop Tauri transport is not wired end-to-end — RESOLVED 2026-11-09
- **Where**: `src-tauri/src/shell/mod.rs:9-25` registers a single generic
  `authr_command`; `src/ipc/bridge.ts:52-54` invokes the contract name
  directly (`core.invoke('project_init', …)`), never `authr_command`.
- **Impact**: in a packaged desktop build every frontend call would fail with
  Tauri's "unknown command" (INFERRED — the GUI build isn't exercised by any
  test or doc runbook).
- **Resolution**: `bridge.ts` routes every call through `authr_command` with
  client-side bundle tracking (init/import/open) — ADR-0008; the dev bridge
  also re-targets its bundle tracker after `project_open`. Contract §1
  documents the pass-through. GUI smoke on a desktop target remains a
  manual release step (no headless webview).

## T-2 — `segment_search` does not sanitize FTS MATCH input — RESOLVED 2026-11-09
- **Where**: `storage/segments.rs:73-80` passes the raw query into
  `MATCH ?1`; contrast `rag/store.rs::fts_search:135` which quotes/sanitizes.
- **Resolution**: shared sanitizer `rag::store::fts_sanitize` used by both
  call sites, both now parameter-bound; regression test `test_ft7`
  (`test_t1_fts5_triggers.py`). See `FORENSIC_AUDIT.md §3.3`.

## T-3 — Duplicated duration estimator — RESOLVED 2026-11-09
- **Where**: `ipc/studio.rs:62` and `:200` vs `publish/mod.rs:84-92`
  (`chars×55+400`), plus `audio_plan` segment view.
- **Resolution**: single canonical `audio::scheduler::estimate_duration_ms`;
  every caller delegates; locked-formula unit test in `scheduler.rs`.
  Invariant §3 coupling note updated accordingly.

## T-4 — `chapter_delete` skips `after_commit()` and any immediate git commit — RESOLVED 2026-11-09
- **Where**: `storage/chapters.rs:103-108` (contrast `update:96-98`).
- **Resolution**: delete now runs `after_commit()` → shadow removal →
  contract-locked `chore: manual snapshot` commit (no §8 change needed);
  response carries `commit`. Tests: `test_cd6` (new), `test_x2` updated to
  assert the committed state.

## T-5 — `row_json` BLOB placeholder leaks into projections — RESOLVED 2026-11-09
- **Where**: `storage/db.rs:206` renders any BLOB as `"<N bytes>"`.
- **Resolution**: `row_json` now fails loudly (`InvalidColumnType`) when a
  projection selects a BLOB; binary data must go through dedicated loaders
  (`rag::store::all_chunks`). Unit test in `db.rs`.

## T-6 — Tauri CSP is `null` — RESOLVED 2026-11-09
- **Where**: `src-tauri/tauri.conf.json:23-25` (`"csp": null`).
- **Resolution**: restrictive CSP set (`default-src 'self'; connect-src
  'self' ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; …`).
  No external font/script loads exist (verified), so the policy should be
  asset-neutral; a desktop-target smoke pass remains a manual release step.

## T-7 — Stale counts in shipped docs/metadata — RESOLVED 2026-11-09
- **Where (was)**: `README.md` ("149 tests", "v1.1: 48 commands");
  `src-tauri/src/main.rs:13` clap `about` ("48-command IPC surface");
  `test_results.json` (`totals.test_modules=28`, suite string "contract v1.1"
  / "Phase 1–5").
- **Resolution**: README was corrected in the docs commit that added this
  file; `main.rs:13` now says 69-command; `test_results.json` is regenerated
  from live runs by `scripts/refresh_acceptance.py` (contract v1.2, live
  command/module/test counts, benchmark gates — 170 E2E / 70 unit as of
  Phase 7). Keep counts generated, not hand-edited.

## T-8 — `run_e2e.py --feature` map missing an alias — RESOLVED 2026-11-09
- **Where**: `tests/run_e2e.py:22-49` — `test_t2_publish_boundaries.py` has
  no `--feature` entry (all other 29 modules reachable).
- **Resolution**: added `publish_boundaries` plus the two other missing
  aliases (`publish`, `release_bench`).

## T-9 — RAG dense scan + `all_chunks` enrichment are O(corpus) — RESOLVED 2026-11-09
- **Where**: `rag/search.rs::dense_search:38` scans every row;
  `search():114` loads the full chunk table for snippet enrichment on every
  query.
- **Resolution**: enrichment now fetches only the ≤k fused rows
  (`store::chunks_by_ids`); the scan reads a minimal blob projection
  (`store::all_chunk_blobs`) and fans out across the rayon pool
  deterministically (ADR-0008). Measured: `rag_search` p95 −14–17 % at book
  scale (`scripts/latency_profile.py`, `FORENSIC_AUDIT.md §4`). The
  sqlite-vec swap remains the documented seam for very large corpora.

## T-10 — PID liveness is Linux-only
- **Where**: `bundle/lock.rs:124-141` (`/proc/<pid>`; non-Linux trusts
  same-host leases while fresh).
- **Impact**: on Windows/macOS a crashed process holds the bundle read-only
  until the 120 s TTL expires (self-healing, but slower).
- **Direction**: platform liveness backends (`sysinfo`-style) if desktop
  shipping demands instant recovery.

## T-11 — Slug collisions after 64-char truncation — RESOLVED 2026-11-09
- **Where**: `publish/mod.rs::slugify:125` truncates at 64 chars; two titles
  sharing a 64-char prefix overwrite each other's export file (INFERRED).
- **Resolution**: when truncation occurs, an 8-hex SHA-256 prefix of the
  full slug is appended — colliding titles diverge, republishing the same
  title stays idempotent. Unit-tested (`slugify_disambiguates_long_prefix_collisions`).

## T-12 — Export drops revision history by design
- **Where**: `bundle/export.rs` excludes `.git/` (contract #5).
- **Impact**: `project_import` yields a bundle with no history; rollback
  after import relies on external backups.
- **Direction**: if history portability is wanted, add an opt-in flag +
  contract bump (never silently change the exclusion list).

## T-13 — Prebuilt binary tracked in git
- **Where**: `src-tauri/target/release/authr` is committed deliberately
  (untracking decision in git `2ab4eb8` kept only the binary).
- **Impact**: repo size/drift risk; tests depend on it matching source
  (stale binary masks source changes when `AUTHR_BIN` isn't rebuilt).
- **Direction**: CI must rebuild before test runs; consider LFS or a release
  artifact instead of git (needs human call — `open-questions.md Q-5`).

## T-14 — No CI pipeline in-repo
- **Where**: no `.github/`, no CI config files (verified `2026-11-08`).
- **Impact**: the "168 green" state exists only as `test_results.json` +
  docs; nothing enforces it per commit.
- **Direction**: wire the §6 recipe from `docs/testing.md` into CI.

## T-15 — v1.6 provider surface shipped without docs or UI — RESOLVED 2026-11-09
- **Where (was)**: `providers.rs` + `provider_get/set/test` landed in commit
  `1ee64d0` with the contract doc still at v1.5/69 commands, `main.rs`
  about-stale ("69-command"), no settings UI consuming the IPC, and no E2E.
- **Impact**: the resolution ladder was invisible to users (the question
  "does settings contain image-generation endpoints?" had the answer "in the
  backend only"), and the frozen-contract doc under-counted its own surface.
- **Resolution**: contract v1.6 documents §4.17–4.19 (74 commands,
  `COMMAND_COUNT` constant guard), `main.rs`/README counts regenerated, the
  AI providers settings dialog ships (Settings → AI providers: image
  endpoint/model/timeout/retries, API key → keychain, LLM server URL, live
  probe), and T1.PV1–PV9 E2E lock the ladder (redaction, validation, ladder
  order, fail-closed probe, style catalog, media sandbox).

## T-16 — Repository-root clutter: stale phase zips, one-off scripts, junk files — RESOLVED 2026-11-09
- **Where (was)**: `download/AUTHR_OS_phase3..6.zip` (stale intermediate
  deliverables, superseded by the two canonical zips), one-off root
  `scripts/patch_clippy.py` / `render_diagrams.py` + duplicated design
  HTMLs, an empty `download/README.md`, a stray root `.env` (novelcast
  `DATABASE_URL` leftover), and the `upload/` novelcast reference snapshot
  tracked wholesale in git.
- **Impact**: `git ls-files` (the PROD zip's single source of truth via
  `scripts/build_deliverables.py`) shipped stale artifacts; workspace
  ambiguity about what is canonical.
- **Resolution**: phase zips deleted (git + disk), one-off scripts
  untracked, design artifacts moved under `docs/design/`, `upload/`
  untracked (drop-zone, gitignored, files kept on disk), `.env` removed,
  `download/README.md` rewritten as a deliverables index.

## Scaling limits (documented, by design)
- One SQLite file per project; no multi-user concurrency (lease = single
  writer).
- Dense RAG scan is exact but O(corpus) memory/CPU (parallelized,
  enrichment fixed — T-9); sqlite-vec is the documented next seam for very
  large corpora.
- Audio cache grows unbounded (no GC; `audio_cache_stats` only reports).
- `benchmark_run` thresholds assume the release profile
  (`Cargo.toml:48-50`); debug builds fail by design.
- argv `--args` ceiling (~128 KB) removed via `--args-stdin` (Phase 7),
  but per-command wall-clock for zip/EPUB assembly grows linearly with
  segment count (measured: `project_export` ~3.1 s / `publish_epub` ~1.6 s
  at 530 segments — `scripts/latency_profile.py`).
