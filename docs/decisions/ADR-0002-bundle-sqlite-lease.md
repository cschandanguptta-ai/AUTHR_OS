# ADR-0002 — Portable `.authr` bundle: SQLite + WAL + 120 s lease

**Status**: Accepted (Phase 1; tests `test_t1_bundle_lifecycle.py`,
`test_t2_stale_lease_recovery.py`, `test_t2_cloud_wal_truncation.py`)

**Context**: Authoring data must live in one directory a user can copy, sync
(Dropbox/OneDrive), and open from two machines — while a single SQLite file
corrupts easily under concurrent multi-host writes or cloud-synced WAL files.

**Decision**:
- Bundle = directory: `project.db*`, `manifest.json` v1, `manuscript/`,
  `audio/`, `exports/`, `assets/`, embedded `.git/` (`lifecycle.rs::SUBDIRS:10-16`,
  contract §6).
- Advisory lease file `.authr_lock` with 120 s TTL, PID liveness, stale
  recovery, foreign-host honoring (`bundle/lock.rs:8,124-172`).
- Fresh foreign lease ⇒ **read-only open**, every writer refuses with
  `READ_ONLY_PROJECT` (`lifecycle.rs:101`, `lock.rs::ensure_writable:259`).
- Cloud-sync guard: bundle path containing a sync-vendor name triggers
  `wal_checkpoint(TRUNCATE)` after every write commit so WAL shrinks to 0
  (`ipc/mod.rs::after_commit:43-49`, `sandbox.rs::is_cloud_sync_path:211`).
- PRAGMAs per connection: WAL, `foreign_keys=ON`, busy_timeout 5000
  (`storage/db.rs:147-154`).

**Alternatives**: file locks/flocking — not implemented in-repo; a server DB —
contradicts local-first portability (no server code exists).

**Consequences**: two machines never write simultaneously (one goes
read-only); crash leftovers self-heal (dead PID/expiry/corrupt ⇒ reclaim,
`lock.rs::classify:153-172`); export intentionally excludes lock/WAL/shm and
`.git/` (`bundle/export.rs`).

**Do not change unless**: you can preserve (a) read-only fallback semantics,
(b) automatic stale recovery, (c) cloud-guard checkpoint behavior — all three
are contract §6 and tier-2 tested.

**Evidence**: files above; contract §6; `docs/data-model.md §3`.
