# ADR-0008 — Workerisation boundaries, RAG scan redesign, mock-keychain gating

**Status**: Accepted (Phase 7 — Core System Optimization & Forensic Integrity Audit)

## Context

The dispatcher (ADR-0001) is synchronous and frozen. Three performance and
integrity problems needed decisions that do **not** touch the command surface:

1. Heavy commands (DSP mastering, RAG indexing over a corpus, zip export, git
   snapshots) execute CPU/IO-bound work. In the Tauri shell they ran directly
   on the command-handler thread.
2. `rag_search` loaded the **entire chunk table twice** per query (once for
   the dense scan, once for snippet enrichment) and scanned it single-threaded.
3. `keychain::backend()` checked `AUTHR_MOCK_KEYCHAIN=1` **before** the
   compiled backend, so a production `gui,os-keychain` desktop build could be
   silently downgraded to the plaintext file-backed mock.

## Decision

1. **Shell-level workerisation**: `shell::authr_command` is now `async` and
   runs the frozen dispatcher on a blocking worker thread
   (`tauri::async_runtime::spawn_blocking`). The webview IPC path never
   blocks. The dispatcher itself stays synchronous and frozen (ADR-0001
   untouched; surface unchanged).
2. **Data-parallel RAG internals** (rayon, deterministic):
   - `rag_index` embeds rows with `par_iter` (the `Embedder` trait is
     `Send + Sync` and pure) and upserts sequentially in the original
     `ORDER BY …` order — chunk insertion order and results are identical
     to the sequential pipeline.
   - `rag_search::dense_scan` loads a minimal `(chunk_id, dim, embedding)`
     projection (`store::all_chunk_blobs`), fans the per-chunk cosine out
     across the pool, collects in index order, and keeps the
     score-desc/chunk_id-asc ordering. Dimension-mismatch errors are
     detected **before** scoring (lowest `chunk_id` wins) so the reported
     chunk never depends on worker scheduling.
   - Enrichment fetches only the fused hit rows (`store::chunks_by_ids`,
     ≤50 ids, primary-key `IN` query) instead of the whole table.
   - What deliberately stays **sequential**: the `audio_synthesize` loop
     (per-speaker `previous_request_id` continuity chain + single-writer
     SQLite stamping — invariants §1.4), and the `apply_mutation`
     transaction (single `BEGIN IMMEDIATE`). The scheduler's waves already
     model intra-chapter parallelism for future engine-level fan-out.
3. **Mock keychain gating**: the file-backed `MockKeyring` is reachable only
   in builds compiled **without** `os-keychain` (headless CI / sandbox, where
   `AUTHR_MOCK_KEYCHAIN=1` still selects it). Desktop builds always use the
   OS keychain; the mock can never hold production credentials.
4. **Bulk-args transport**: additive `--args-stdin` CLI flag + dev-bridge
   switch above 96 KB, removing the OS argv ceiling (~128 KB) for bulk AST
   mutations. Existing `--args` behavior is unchanged.

## Alternatives

- sqlite-vec for the dense scan (documented seam, `docs/LOCAL_AI.md §1`) —
  deferred: adds an extension dependency for a win that matters only at
  corpus sizes the product does not reach yet; the deterministic pure-Rust
  scan plus the enrichment fix bounds the current cost.
- Parallelizing `audio_synthesize` waves — rejected: breaks the acoustic
  continuity chain and single-writer stamping guarantees.
- 56 per-command `#[tauri::command]` wrappers — rejected again (ADR-0001).

## Consequences

- Determinism guarantees (invariants §6) are preserved and unit-tested:
  same scores, same ranks, same insertion order, same error selection.
- New dependency: `rayon = "1.10"` (+0.26 MB binary; benchmark gate still
  passes: cold 1.9 ms / RSS 6.2 MB / binary 10.4 MB vs 25/50/1500 limits).
- `chapter_delete` now performs `after_commit()` + a contract-locked
  `chore: manual snapshot` commit (T-4) — the response carries `commit`.
- Clippy is at **zero warnings** for the lib (17 pre-existing style lints
  fixed); three new Rust regression tests + two E2E regression tests.

**Do not change unless**: you re-run the full gate (`scripts/refresh_acceptance.py`)
and re-verify determinism tests (`rag::search`, `rag::index`, scheduler) after
touching the parallel sections.

## Evidence

`src-tauri/src/shell/mod.rs`, `src-tauri/src/rag/{search,index,store}.rs`,
`src-tauri/src/security/keychain.rs`, `src-tauri/src/main.rs` (`--args-stdin`),
`scripts/ipc-proxy.mjs`, `scripts/latency_profile.py`, `docs/FORENSIC_AUDIT.md`.
