# config-and-env.md — every env var, feature flag, and external binary

No values are stored in-repo (verified: `docs/SECURITY_AUDIT.md §1` repo-wide
secret scan). All runtime configuration is environment-driven or arg-driven.

## 1. Environment variables

| Name | Purpose | Where used (code) | Required? | If missing / wrong |
|---|---|---|---|---|
| `AUTHR_MOCK_KEYCHAIN` | Force file-backed keyring instead of OS keyring | `security/keychain.rs::backend:131` (checks `=1`) | No (CI always sets it) | Headless runs fail `keychain_*` with `SECURITY_ERROR` when no OS keyring exists |
| `AUTHR_MOCK_KEYRING_FILE` | Path of the mock keyring JSON | `keychain.rs` (MockKeyring; default `~/.authr/mock_keyring.json`) | No | All mock backends share one file → cross-test bleed; `conftest.py:42-48` gives each test its own |
| `AUTHR_TTS_OFFLINE` | Deterministically block network TTS engines | `audio/tts.rs::offline_guard:82-90` (exact match `"1"`) | No | Any value other than `1` (e.g. `true`, `0`) does NOT block |
| `ELEVENLABS_API_KEY` | ElevenLabs REST auth | `tts.rs:329` | For `ELEVEN_LABS` engine | `AUDIO_ENGINE_UNAVAILABLE` |
| `CARTESIA_API_KEY` | Cartesia REST auth | `tts.rs:340` | For `CARTESIA` | `AUDIO_ENGINE_UNAVAILABLE` |
| `OPENAI_API_KEY` | OpenAI `/v1/audio/speech` auth | `tts.rs:354` | For `OPENAI_TTS` | `AUDIO_ENGINE_UNAVAILABLE` |
| `HUGGINGFACE_API_KEY` | HF inference (facebook/mms-tts-*) auth | `tts.rs:365` | For `HF_MMS` | `AUDIO_ENGINE_UNAVAILABLE` |
| `AUTHR_LLAMA_SERVER` | Default llama-server endpoint for `local_llm_generate` | `localai/llm.rs` (`DEFAULT_SERVER_URL` fallback `http://127.0.0.1:8080`, `llm.rs:16`) | No | Offline `OFFLINE_GRAMMAR` engine answers instead |
| `AUTHR_RELEASE_KEY_HEX` | 32-byte ed25519 seed (hex) for release signing | `publish/release.rs::seed32/45` | For real releases | Ephemeral key generated and returned as `generated_key_hex` (CI-only mode) |
| `AUTHR_FFMPEG_BIN` | ffmpeg path override for `publish_m4b` | `publish/m4b.rs::ffmpeg_bin:139-146` | No | PATH lookup; absent ⇒ `BUNDLE_ERROR` with install guidance |
| `AUTHR_TYPST_BIN` | typst path override for `publish_typst` PDF compile | `publish/typst.rs:29-41` | No | `.typ` still emitted; `compiled:false` + guidance |
| `AUTHR_BIN` | Binary path for the test suite | `tests/conftest.py:24-39` | No | Defaults to `src-tauri/target/release/authr` |
| `AUTHR_CLI` | Binary the dev bridge spawns | `scripts/ipc-proxy.mjs::resolveBinary` | No | Defaults to `src-tauri/target/release/authr`; missing ⇒ HTTP 500 on `/api/ipc` |
| `AUTHR_BUNDLE` | Default bundle for the dev bridge | `scripts/ipc-proxy.mjs::defaultBundle` | No | `<repo>/sandbox.authr` |

Edge TTS uses **no key** (the Sec-MS-GEC token is computed,
`tts.rs:172-191`).

## 2. Cargo features (compile-time flags)

| Feature | Enables | Where | If omitted |
|---|---|---|---|
| `gui` | `tauri`, `tauri-build`; `shell` module compiled | `Cargo.toml:19`, `lib.rs:15-16` | No desktop binary; CLI/tests unaffected (default build is hermetic) |
| `os-keychain` | `keyring` crate; real OS keyring backend | `Cargo.toml:20`, `keychain.rs:20-59` | Only mock backend exists; `keychain_*` still works under the mock |

Desktop build: `cargo build --release --features gui,os-keychain`
(`README.md`, `docs/PACKAGING.md`).

## 3. External binaries

| Binary | Used by | Resolution order | Failure mode |
|---|---|---|---|
| `ffmpeg` | `publish_m4b` | arg `ffmpeg_bin` → `AUTHR_FFMPEG_BIN` → PATH (`m4b.rs:139-146`) | `BUNDLE_ERROR` |
| `typst` | `publish_typst` PDF | arg `typ_bin` → `AUTHR_TYPST_BIN` → PATH (`typst.rs:29-32`) | `compiled:false` (graceful) |

## 4. Feature flags in configs (not env)

| Flag | File | Meaning | Status |
|---|---|---|---|
| `bundle.targets` | `src-tauri/tauri.conf.json:29` | msi, nsis, dmg, appimage, deb | configured; per-OS build selects |
| `plugins.updater.endpoints` | `tauri.conf.json:61-68` | GitHub Releases `latest.json` URL | configured |
| `plugins.updater.pubkey` | `tauri.conf.json:66` | ed25519 public key for update verification | **placeholder** — must be replaced at release (`docs/PACKAGING.md §Key management`) |
| `bundle.windows.certificateThumbprint` / `macOS.signingIdentity` | `tauri.conf.json:43,50` | code-signing | `null` — release-machine task |
| `app.security.csp` | `tauri.conf.json:23-25` | Content-Security-Policy for the webview | `null` — see `tech-debt.md T-6` |
| `profile.release` `opt-level=2`, `lto=false` | `src-tauri/Cargo.toml:48-50` | binary-size vs build-time tradeoff | load-bearing for the 25 MB benchmark gate |

## 5. Runtime tuning constants (in-code, not env)

| Constant | Value | File |
|---|---|---|
| Lease TTL | 120 s | `bundle/lock.rs:8` |
| Target RMS | −20.0 dB | `audio/master.rs:11` |
| Limiter ceiling | −3.1 dBFS | `audio/master.rs:12` |
| Ducking defaults | 18 dB / 120 ms / 750 ms | `audio/mixer.rs:46-52` |
| RRF k | 60 | `rag/search.rs:17` |
| Embedding dim | 384 | `rag/embed.rs:13` |
| Duration estimator | chars×55 + 400 ms | `ipc/studio.rs:62`, `publish/mod.rs:89` |
| IPA batch cap | 256 names | `ipc/studio.rs:38-40` |
| Layers cap | 1–16 | `ipc/studio.rs:307` |
| Sample-rate range | 8000–96000 | `ipc/studio.rs:110,325` |
| Slug length cap | 64 chars | `publish/mod.rs:125` |
| SQLite busy timeout | 5000 ms | `storage/db.rs:152` |

These are contract-visible defaults (§4.11/§4.13) — changing any requires a
contract review, not just a code change.
