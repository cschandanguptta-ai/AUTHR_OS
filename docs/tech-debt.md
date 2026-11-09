# tech-debt.md — known shortcuts, fragile code, scaling limits

Each item: what / where / why it matters / suggested direction. FACT unless
labeled. IDs are stable for reference from other docs and commits.

## T-1 — Desktop Tauri transport is not wired end-to-end
- **Where**: `src-tauri/src/shell/mod.rs:9-25` registers a single generic
  `authr_command`; `src/ipc/bridge.ts:52-54` invokes the contract name
  directly (`core.invoke('project_init', …)`), never `authr_command`.
- **Impact**: in a packaged desktop build every frontend call would fail with
  Tauri's "unknown command" (INFERRED — the GUI build isn't exercised by any
  test or doc runbook).
- **Direction**: either map in `bridge.ts` (`invoke('authr_command', {command,
  bundle, args})`) or generate 56 `#[tauri::command]` wrappers. Then add a
  smoke test/`open-questions.md Q-2` resolution.
- **Status**: highest-priority debt for the desktop release.

## T-2 — `segment_search` does not sanitize FTS MATCH input
- **Where**: `storage/segments.rs:73-80` passes the raw query into
  `MATCH ?1`; contrast `rag/store.rs::fts_search:135` which quotes/sanitizes.
- **Impact**: queries containing FTS5 syntax (`AND`, `OR`, `NOT`, `"`, `*`,
  `NEAR(`) raise SQL errors → `STORAGE_ERROR` (or narrow results) instead of
  literal-text search; inconsistent with `rag_search` behavior.
- **Direction**: reuse the rag sanitizer for both call sites.

## T-3 — Duplicated duration estimator
- **Where**: `ipc/studio.rs:62` and `:200` vs `publish/mod.rs:84-92`
  (`chars×55+400`), plus `audio_plan` segment view.
- **Impact**: edits in one copy silently desync `audio_plan` estimates from
  captions/EPUB SMIL/M4B chapter marks (invariant §3,
  `invariants-and-gotchas.md`).
- **Direction**: move to a single `pub fn` (e.g. `audio::scheduler` or
  `publish::mod`) and call from both.

## T-4 — `chapter_delete` skips `after_commit()` and any immediate git commit
- **Where**: `storage/chapters.rs:103-108` (contrast `update:96-98`).
- **Impact**: manifest `last_modified_at` goes stale after deletions; the
  deletion is only captured in git at the next chapter sync (or never, for a
  delete-only session).
- **Direction**: call `ctx.after_commit()` + a snapshot commit (message per
  contract §8 — needs a decision: extend §8 with a delete message or reuse
  `chore: manual snapshot`).

## T-5 — `row_json` BLOB placeholder leaks into projections
- **Where**: `storage/db.rs:206` renders any BLOB as `"<N bytes>"`.
- **Impact**: harmless today (no BLOB column is in the §5 projections), but a
  future column add to a projection list would silently emit placeholders.
- **Direction**: either drop BLOB handling (panic loudly) or document a
  `row_json_no_blobs` variant.

## T-6 — Tauri CSP is `null`
- **Where**: `src-tauri/tauri.conf.json:23-25` (`"csp": null`).
- **Impact**: no Content-Security-Policy in the desktop webview; acceptable
  while the frontend only talks to the local bridge, weakens
  defense-in-depth (INFERRED).
- **Direction**: set a restrictive CSP before public release; verify Lexical
  + vite asset loading still work.

## T-7 — Stale counts in shipped docs/metadata — RESOLVED 2026-11-09
- **Where (was)**: `README.md` ("149 tests", "v1.1: 48 commands");
  `src-tauri/src/main.rs:13` clap `about` ("48-command IPC surface");
  `test_results.json` (`totals.test_modules=28`, suite string "contract v1.1"
  / "Phase 1–5").
- **Resolution**: README was corrected in the docs commit that added this
  file; `main.rs:13` now says 56-command; `test_results.json` is regenerated
  from live runs by `scripts/refresh_acceptance.py` (records contract v1.2,
  56 commands, 30 modules, 168 E2E, 67 unit tests, benchmark gates). Keep
  counts generated, not hand-edited.

## T-8 — `run_e2e.py --feature` map missing an alias
- **Where**: `tests/run_e2e.py:22-49` — `test_t2_publish_boundaries.py` has
  no `--feature` entry (all other 29 modules reachable).
- **Impact**: minor ergonomics; the module still runs under `--tier=2`.
- **Direction**: add `"publish_boundaries": "tier2_boundaries/…"`.

## T-9 — RAG dense scan + `all_chunks` enrichment are O(corpus)
- **Where**: `rag/search.rs::dense_search:38` scans every row;
  `search():114` loads the full chunk table for snippet enrichment on every
  query.
- **Impact**: fine at book scale (thousands of chunks; sub-millisecond per
  unit test `rag/search.rs:236`), degrades with very large corpora.
- **Direction**: sqlite-vec swap is already the documented seam
  (`docs/LOCAL_AI.md §1`); enrichment can join instead of full-load.

## T-10 — PID liveness is Linux-only
- **Where**: `bundle/lock.rs:124-141` (`/proc/<pid>`; non-Linux trusts
  same-host leases while fresh).
- **Impact**: on Windows/macOS a crashed process holds the bundle read-only
  until the 120 s TTL expires (self-healing, but slower).
- **Direction**: platform liveness backends (`sysinfo`-style) if desktop
  shipping demands instant recovery.

## T-11 — Slug collisions after 64-char truncation
- **Where**: `publish/mod.rs::slugify:125` truncates at 64 chars; two titles
  sharing a 64-char prefix overwrite each other's export file (INFERRED).
- **Impact**: improbable but data-lossy for generated titles.
- **Direction**: disambiguate with a short content hash suffix on collision.

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

## Scaling limits (documented, by design)
- One SQLite file per project; no multi-user concurrency (lease = single
  writer).
- Whole-corpus FTS/dense scans (T-9) bound practical RAG corpus size.
- Audio cache grows unbounded (no GC; `audio_cache_stats` only reports).
- `benchmark_run` thresholds assume the release profile
  (`Cargo.toml:48-50`); debug builds fail by design.
