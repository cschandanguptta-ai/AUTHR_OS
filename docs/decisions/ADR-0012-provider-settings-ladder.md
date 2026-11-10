# ADR-0012 — Provider Settings Ladder & Scene Studio parity (contract v1.6)

**Status**: Accepted (2026-11-09)
**Extends**: ADR-0005 (offline-first AI seams), ADR-0011 (video novel studio)
**Source method**: the NovelCast Studio reference snapshot
(`upload/novelcast-source-snapshot.zip`) — its ADR-0002 (three-tier provider
fallback: custom > free > built-in) and its settings dialog define the user
model; this ADR re-expresses it under AUTHR_OS's fail-closed, offline-first
rules.

## Context

The R2I endpoint seam shipped in v1.5 as env-only (`AUTHR_IMG_ENDPOINT`):
fail-closed, but with no settings surface, no key management story beyond
env vars, and no probe. Novelcast, by contrast, stores per-kind provider
config (LLM / TTS / IMAGE) in its database, keeps keys server-side, tests
providers from the settings dialog before saving, and drives its whole
storyboard experience from that configuration: art styles pick a coherent
look, scene cards repaint through the live tier, and the assembly consumes
whatever the painter produced.

Two questions fell out of the parity workstream: (1) *does AUTHR_OS
settings contain image-generation endpoints?* — after this ADR, yes, at
every layer; (2) *how does the storyboard show real pixels without
exposing the filesystem?*

## Decision

1. **One ladder for every live endpoint** (`providers.rs`):
   `per-call args > project settings > env > fail-closed`. The settings
   layer is the generic, additive `provider_settings(section, key, value)`
   table (sections: `image`, `llm`, `art`). Keys never live there —
   keychain (service `authr_img`) or env only, because bundles travel
   (export/import, git shadow layer). `provider_get` redacts to badges;
   `provider_set` validates; `provider_test` runs one real generation and
   never writes assets. Offline ⇒ `AI_ENGINE_UNAVAILABLE`, the canonical
   fail-closed behavior of ADR-0005 — the SCRIPTED engine remains the
   default that always works.
2. **Hardened shared fetch**: the R2I render tier and the probe share one
   path — bounded retries with exponential backoff (transport faults +
   429/5xx), per-attempt timeout (5–300 s, settings-tunable), 64 MiB
   response cap, PNG/JPEG magic-byte sniffing, dims verified before
   registration.
3. **Settings UI parity**: the cockpit settings popover gains an
   "AI providers" dialog (image endpoint/model/timeout/retries, API key →
   keychain, local LLM server URL, live probe with latency) — zero new
   dependencies, same modal + focus-trap chrome as the rest of the cockpit.
4. **Art styles** (§4.18 `video_style`): the ten-style novelcast catalog is
   mirrored on both sides (Rust authoritative, TS mirror for the picker).
   The style prompt fragment rides every image prompt; the style id joins
   every render seed, so switching styles deterministically repaints while
   a given style stays byte-identical. Selection persists per book in
   `provider_settings` (section `art`); unknown ids fall back to the
   default so catalog renames never strand bundles.
5. **Scene frames honor the live tier**: the v1.5 scene painter was
   SCRIPTED-only; sheets could use the endpoint but scenes could not — the
   parity gap the workstream started from. Scene prompts now carry the
   segment text verbatim + every in-frame character's bio verbatim (the
   consistency ladder) + the style fragment.
6. **Registry-driven assembly**: `video_assemble` reads its clip table from
   `video_assets` (BMP/PNG/JPEG alike) instead of hardcoding `.bmp` names;
   gaps fall back to a SCRIPTED ensure-render with `missing_only`, which
   renders ONLY registry/file gaps — live endpoint art is never clobbered
   by a scripted card.
7. **`media_read`** (§4.19): bundle-scoped, read-only accessor returning
   base64 + mime + sha256. `media/` prefix enforced, absolute paths and
   `..` rejected (`PATH_TRAVERSAL_VIOLATION`), canonicalized containment
   inside root AND media tree (symlink escapes included), image containers
   only, 32 MiB cap. The UI caches data URLs keyed by (path, sha256): new
   pixels refetch, unchanged art is free.

## Consequences

- The v1.6 surface grew from 69 documented (72 shipped) to 74 commands;
  the contract doc now lists the provider family explicitly.
- `main.rs`'s `about` and README counts are regenerated alongside the
  contract (the v1.6 provider commands shipped undocumented once — that
  drift is logged as resolved tech debt and guarded by the count constants
  in `lib.rs`).
- The frontend consumes the ladder in two places: the settings dialog
  (configuration + probe) and the Scene Studio engine chip (visible
  SCRIPTED/R2I negotiation, fail-closed messaging).
- Workerisation on the frontend side (ADR-0008's frontend complement):
  chapter text stats moved to a module Web Worker and the five secondary
  studios are code-split (React.lazy) — the initial bundle parse shrinks
  and text re-counting leaves the paint path.

## Verification

- `tests/tier1_features/test_t1_provider.py` — T1.PV1–T1.PV9: redaction,
  validation, clear roundtrip, settings-vs-env ladder (image + llm),
  offline fail-closed probe, style catalog/persistence/isolation,
  `media_read` roundtrip + sandbox, style-seed determinism.
- Rust units in `providers.rs` (ladder, backoff, retry policy, container
  sniffing), `media.rs` (ext/mime map, base64 roundtrip),
  `video/style.rs` (catalog uniqueness, fallback),
  `video/render.rs` (prompt composition, seed divergence).
