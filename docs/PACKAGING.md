# AUTHR_OS — Packaging, Signing & Auto-Update (Phase 6, Sprint 15)

## Installer matrix

| Platform | Targets | Bundler notes |
|----------|---------|---------------|
| Windows  | MSI (WiX), NSIS | `bundle.windows.wix` / `nsis` in `src-tauri/tauri.conf.json`; SHA-256 digest + DigiCert timestamp configured; Authenticode requires `certificateThumbprint` of the code-signing cert |
| macOS    | DMG (+ .app) | Universal binaries (Apple Silicon + Intel) build from the same `tauri build --target universal-apple-darwin`; signing via `signingIdentity` (Developer ID), notarization via `tauri notarize` |
| Linux    | AppImage, .deb | `bundleMediaFramework:false` keeps the AppImage lean; deb depends on `libwebkit2gtk-4.1-0`, `libgtk-3-0` |

All five targets are pre-configured in `tauri.conf.json` (`bundle.targets`).
Release binaries stay under the 25 MB installer budget (current core binary:
**10.1 MB**, see `docs/BENCHMARKS.md`).

## Build commands

```bash
# Windows (on Windows)
npm run build && cargo tauri build                     # → .msi + NSIS .exe

# macOS (on macOS)
cargo tauri build --target universal-apple-darwin      # → .dmg + .app

# Linux (on Linux)
cargo tauri build                                      # → .AppImage + .deb
```

## Release signing & auto-updater

The updater consumes a Tauri-v2-shaped `latest.json` produced by the core:

```bash
authr release_manifest --args '{
  "artifact": "dist/AUTHR_OS_3.0.0_x64-setup.exe",
  "version": "3.0.0-PROD",
  "url": "https://github.com/authr-os/authr-os/releases/download/v3.0.0/AUTHR_OS_3.0.0_x64-setup.exe"
}'
# → writes latest.json next to the artifact (ed25519 signature + sha256)
```

Key management:

1. Generate/keep the 32-byte ed25519 seed hex in the OS keychain
   (`authr keychain_store --args '{"service":"authr-release","key":"ed25519-seed",…}'`).
2. Export it per-release as `AUTHR_RELEASE_KEY_HEX` (never hard-coded).
3. CI-only sign-offs may omit the key — `release_manifest` then generates an
   ephemeral key and returns it as `generated_key_hex` (clearly marked).
4. Paste the matching public key into `tauri.conf.json → plugins.updater.pubkey`.

Verification is a one-liner and is fully offline:

```bash
authr release_verify --args '{"manifest":"latest.json","artifact":"AUTHR_OS_3.0.0_x64-setup.exe"}'
# {"ok":true,"data":{"valid":true,"checks":{"sha256":true,"signature":true}, …}}
```

The desktop shell (feature `gui`) registers the Tauri updater plugin against
the `endpoints` URL above; a failed signature or hash aborts the update before
the payload is applied (verified by `tests/tier1_features/test_t1_release_bench.py`).

## Golden Master checklist (Sprint 16)

1. `cargo build --release` — zero warnings
2. `python3 tests/run_e2e.py` — full suite green (contract harness asserts the 62-command surface)
3. `authr benchmark_run --args '{}'` — `pass:true` (cold start / RSS / binary)
4. `authr release_manifest` with the production key → `latest.json` + signature
5. `authr release_verify` → `valid:true`
6. Tag `v3.0.0-PROD`, upload installers + `latest.json` to GitHub Releases
