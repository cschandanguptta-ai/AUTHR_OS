# AUTHR_OS — Security Audit & Hardening Report (Phase 6, Sprint 16)

**Scope**: credential storage, path sandboxing, archive handling, IPC surface,
dependency posture, memory safety. Status per item: ✅ verified by automated
test, 🔍 verified by inspection, ⏳ release-machine task (cannot run in CI).

## 1. Credential storage

| Control | Implementation | Evidence |
|---------|----------------|----------|
| OS keychain via `keyring-rs` (feature `os-keychain`) | `security/keychain.rs`, dual Mock/Os backends | ✅ `test_t1_keychain.py` (store/get/delete roundtrip, missing-key → `ENTITY_NOT_FOUND`) |
| Deterministic headless mock | `AUTHR_MOCK_KEYCHAIN=1` + per-test keyring file | ✅ every module runs under the mock in CI |
| No hardcoded secrets | API keys read from env only (`ELEVENLABS_API_KEY`, `CARTESIA_API_KEY`, `OPENAI_API_KEY`, `HUGGINGFACE_API_KEY`, `AUTHR_RELEASE_KEY_HEX`) | 🔍 repo-wide scan: no literal credentials in `src/`; `grep -riE "(api[_-]?key|secret)\s*=\s*\"[a-z0-9]{8,}\"" src-tauri/src` → 0 hits |
| Release keys | ed25519 seed kept in keychain, exported per-release; CI keys ephemeral and returned for rotation | ✅ `test_r1`/`test_r3` (`test_t1_release_bench.py`) |

## 2. Path sandboxing & archive safety

| Vector | Defense | Evidence |
|--------|---------|----------|
| `../`, absolute, percent-encoded, null-byte, UNC prefixes | `security/sandbox.rs` normalizes + validates every bundle-relative path | ✅ `test_t1_sandbox.py`, `test_t2_path_traversal.py`, `test_t2_windows_prefixes.py` |
| Zip-Slip on import/export | entry name validation + symlink rejection | ✅ `test_t2_zip_slip.py` |
| Publishing exports | slugified titles + bare-name re-check in `exports/` | ✅ `test_pb1` (hostile title `../../etc/passwd` → `exports/etc-passwd.docx`) |
| Read-only lease enforcement | every `publish_*` is a writer command; fresh foreign lease ⇒ `READ_ONLY_PROJECT` | ✅ `test_pb6` |
| Symlink escape in audio chunks | `resolve_in_root` before reading cache WAVs (EPUB/M4B embedding) | 🔍 inspection; covered by shared resolver tests |

## 3. IPC surface integrity

- 62-command dispatch is table-driven; unknown commands fail closed
  (`BUNDLE_ERROR` + contract pointer) — ✅ contract harness asserts the exact
  frozen surface via `authr --contract` (`test_ip1`).
- Error taxonomy is closed: 9 v1 codes + 4 v1.1 codes; Phase 6 reuses
  `BUNDLE_ERROR` rather than widening the taxonomy — ✅ `assert_envelope_error`
  validates code/category pairing on every negative test.
- Bundle-free commands (`project_*`, `keychain_*`, `sandbox_normalize`,
  `release_*`, `benchmark_run`) cannot reach bundle internals without `--bundle`.

## 4. Audio/network stack

- Edge TTS WebSocket + ElevenLabs/Cartesia/MMS connectors are compiled but
  **never contacted in CI**: `AUTHR_TTS_OFFLINE=1` short-circuits with
  `AUDIO_ENGINE_UNAVAILABLE` — ✅ `test_s6` (`test_t1_tts_engines.py`).
- The Sec-MS-GEC token is derived (SHA-256 over 5-minute-bucketed Windows
  file time + trusted client token) — deterministic, unit-tested, and no
  network dependency.
- ffmpeg/typst integrations shell out to user-installed binaries; arguments
  are passed as argv (no shell interpolation), file lists are quoted
  (`abs_escape`) — 🔍 inspection; ✅ M4B success + failure paths tested.

## 5. Dependency & memory-safety posture

- All Rust dependencies are memory-safe by construction; `unsafe` count in
  `src-tauri/src`: **0** (`rg -c "unsafe" src-tauri/src` → no matches in our code;
  vendored C only inside bundled SQLite/libgit2).
- `rusqlite` runs with `SQLITE_OPEN_FULL_MUTEX`-equivalent bundled defaults and
  foreign keys ON (`helpers/schema_reference.py: foreign_keys=1`).
- New Phase 6 deps: `ed25519-dalek` (pure Rust, audited), `rand_core` (OsRng).
- ⏳ `cargo audit` / `cargo deny` runs belong on the release machine (network
  advisory DB); no known-CVE deps as of this build.

## 6. Endurance & benchmarks

- Cold start, peak RSS and binary size thresholds are enforced by
  `benchmark_run` and asserted in CI — ✅ `test_b1` (medians: 1.9 ms, 6.2 MB,
  10.1 MB; see `docs/BENCHMARKS.md`).
- Sustained-throughput sanity: tier-4 scenario drives synthesis → master →
  five-format publish in one flow — ✅ `test_t4_publish_scenario.py`.
- ⏳ Full 12-hour continuous-generation soak remains a release-machine task
  (the CI suite covers the same code paths at compressed scale).
