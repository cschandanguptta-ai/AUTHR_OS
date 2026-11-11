# ADR-0006 — Path sandbox + closed error taxonomy

**Status**: Accepted (Phase 1 security core; Phase 6 reuse; evidence in
`docs/SECURITY_AUDIT.md`, `test_t1_sandbox.py`, tier-2 boundary modules)

**Context**: Every command takes bundle-relative paths; archives are
imported/exported; publish outputs derive from user-controlled book titles.
Any path escape is a critical vuln.

**Decision**:
- Inbound strings: `normalize_raw` strips Windows verbatim prefixes
  (`\\?\`, `\\?\UNC\`, `\??\`), percent-decodes (rejecting encoded
  traversal), normalizes separators/drive casing, rejects `..` segments
  (`security/sandbox.rs:12-118`).
- File access: `resolve_in_root` rejects `ParentDir`/`Prefix` components and
  symlink components, then canonicalize-checks containment
  (`sandbox.rs:119-169`).
- Archives: `validate_zip_entry` rejects absolute/backslashed/drive-letter/
  `..`/null entries (`sandbox.rs:174-208`); exports slug titles via
  `slugify` (≤64 chars) plus a bare-name re-check (`publish/mod.rs:107,150`).
- Errors: closed taxonomy — 13 codes / 5 categories (9 v1 + 4 v1.1) (`error.rs`); Phase 6
  publishing reuses `BUNDLE_ERROR` instead of adding codes
  (`CLI_CONTRACT.md:5-7,197-199`).

**Alternatives**: chroot/jail process isolation — not implemented (single
process, no plugins); per-call ad-hoc checks — replaced by the two choke
points above.

**Consequences**: every file-touching handler must route through the choke
points (hard rule 3 in `AGENTS.md`); the taxonomy is closed, so new failure
modes map onto existing codes; security boundary tests are pure string/path
vectors and run in milliseconds.

**Do not change unless**: you extend the vector tests at the same time
(`test_t2_path_traversal.py`, `test_t2_windows_prefixes.py`,
`test_t2_zip_slip.py`) and keep codes closed.

**Evidence**: `src-tauri/src/security/sandbox.rs`, `error.rs`,
`docs/SECURITY_AUDIT.md §2-3`.
