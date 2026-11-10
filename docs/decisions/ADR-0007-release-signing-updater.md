# ADR-0007 — ed25519 release signing + Tauri-updater manifest

**Status**: Accepted (Phase 6 Sprint 15; `test_t1_release_bench.py` r1–r3)

**Context**: The desktop auto-updater needs signed update manifests; keys
must never live in the repo; CI needs a keyless path that is obviously
non-production.

**Decision**:
- `release_manifest` signs artifact bytes with ed25519 (`ed25519-dalek`);
  key resolution: arg `key_hex` → `AUTHR_RELEASE_KEY_HEX` → ephemeral key
  that is **returned** as `generated_key_hex` (CI-only mode,
  `publish/release.rs:45-105`).
- Emits Tauri-v2-shaped `latest.json` (version, pub_key, platforms with
  signature + sha256) next to the artifact.
- `release_verify` independently re-checks sha256 + signature and reports
  `valid:false` for tampered artifacts while still answering `ok:true`
  (`release.rs:107-155`; tamper test in `release_bench`).
- Operational key custody: seed in OS keychain via `keychain_store`
  (`service:"authr-release"`, `key:"ed25519-seed"`), exported per-release as
  `AUTHR_RELEASE_KEY_HEX` (`docs/PACKAGING.md §Key management`).

**Alternatives**: Authenticode/codesign only — platform-bound, doesn't cover
`latest.json`; RSA/minisign — no crate evidence in `Cargo.toml`
(`ed25519-dalek` is the only signature dep).

**Consequences**: the updater `pubkey` in `tauri.conf.json:66` is currently a
placeholder that must be filled from a real `release_manifest` run (see
`open-questions.md Q-3`); ephemeral CI keys are single-use by design.

**Do not change unless**: the `latest.json` field mapping to Tauri's updater
schema is re-verified against the vendored Tauri version, and r1–r3 tests are
extended for any new field.

**Evidence**: `src-tauri/src/publish/release.rs`, `docs/PACKAGING.md`,
`tauri.conf.json:61-68`.
