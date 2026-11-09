# ADR-0011 — Video Novel Studio (contract v1.5)

**Status**: Accepted (2026-11-09)
**Extends**: ADR-0005 (canonical audio), ADR-0008 (workerisation),
ADR-0009/0010 (craft/retention catalogs)
**Source method**: the NovelCast Studio reference snapshot
(`upload/novelcast-source-snapshot.zip`) — its ADR-0002 (provider fallback),
ADR-0004 (R2I reference-sheet engine) and ADR-0008 (offline video render)
define the method; this ADR folds it into AUTHR_OS's offline-first runtime
without adopting its web-native substrate.

## Context

The user directive: generate **video novels** with the *same method as audio
generation, but with images* — and ensure all audio/video/image steps happen
at draft time so nothing is derived twice by an LLM. NovelCast turns a
pasted novel into a voiced, illustrated video via an ANALYSIS job (LLM
segments + characters + scenes) and a PRODUCTION job (speech + R2I reference
sheets + one illustration per scene + client-side WebCodecs render).
AUTHR_OS already owns the audio half (Phase 4) and a VideoStudio pacing UI
whose render button was the **last fake-success path in the app** (a
`setTimeout` toast — the exact pattern the Phase 7 forensic audit hunts).

## Decision

1. **Draft-time derivation (the "no double work" contract)**:
   `video::derive::refresh_chapter_media` is hooked into BOTH draft write
   paths (`segment_append`, `sync_lexical_ast_mutation`, best-effort —
   never blocks a draft write) and persists the canonical `video_scenes`
   rows: one scene per narration segment, canonical duration (stored
   `audio_duration_ms` else the shared `audio::scheduler::estimate_duration_ms`
   — the single estimator), a deterministic ≤3-character whole-word mention
   scan (the novelcast ladder cap), focal defaults. `video_plan` /
   `video_render` / `video_assemble` consume the rows and re-run the hook
   only as an idempotent catch-up (`refreshed:false` when state matches).
   No LLM and no stage ever re-derives what the write path already knows.
2. **Engines mirror the TTS negotiation**: `SCRIPTED` (default, always
   available) is a deterministic offline BMP painter — `video::frame`:
   header-legal 24-bit BMPs, an embedded 5×7 column-bitmask font (real text
   on frames, no font deps), SHA-256-seeded HSV palettes, scene cards with
   the storyboard signature (gradient + horizon glow + vignette + caption
   band), R2I-style reference sheets (three-view grid + swatches + verbatim
   bio) and title cards. `R2I_ENDPOINT` posts to an OpenAI-compatible
   images endpoint (`AUTHR_IMG_ENDPOINT`, novelcast's custom-provider tier)
   and **fails closed** with `AI_ENGINE_UNAVAILABLE` when unset — never
   fakes a frame; results are header-probed (`publish::image_dimensions`)
   before registration.
3. **Consistency ladder preserved**: bio text rides verbatim on every tier;
   sheets are book-level (`video_assets.chapter_number = 0` sentinel);
   multi-reference image editing remains the documented future seam.
4. **Assembly is one ffmpeg pass** with the M4B-identical binary resolution
   (args → `AUTHR_FFMPEG_BIN` → PATH, missing ⇒ `BUNDLE_ERROR`):
   single-frame still inputs → per-scene `zoompan` Ken Burns (alternating
   push-in/pull-out anchored on the persisted focal point — NOT `-loop 1`,
   which multiplies zoompan duplication into a 12-minute render of a 9-second
   plan; measured and documented) → concat → `libx264 yuv420p` + AAC.
   Narration reuses the audio pipeline's per-scene WAVs
   (`auto:pipeline-audio`) when every scene carries one — the audiobook and
   the video novel share one voice track; otherwise a silent track sized to
   the plan (no `-shortest`: the title card tail survives). SRT soft-muxes
   as `mov_text`.
5. **Storage additive only**: `video_scenes` (canonical derived plan;
   `focal` is USER state — inherited by segment identity across re-derives,
   never reset by bookkeeping) and `video_assets` (rendered-asset registry,
   UNIQUE per book/chapter/kind/ref). Eager idempotent migrations; frozen
   DDL untouched; **no new error codes** (AUDIO_ERROR / BUNDLE_ERROR /
   AI_ENGINE_UNAVAILABLE reused).
6. **The VideoStudio render stub is replaced by the real pipeline**: plan →
   render → assemble → validate with persisted focals — the last fake-success
   path in the frontend is gone.
7. **`video_validate` gates the chain** (kdp_check shape): derived plan →
   registry coverage → file presence → encoded-vs-plan duration (1.5 s
   tolerance for AAC priming; probe strips the `, start:` tail before
   parsing `Duration:`).

## Consequences

- 65 → 69 commands (§4.16); harness, runner alias map (`video`, and the
  long-missing `retention` — the T-8 pattern again), docs and
  `refresh_acceptance.py` updated together (205 E2E / 120 Rust units).
- 120 new Rust unit tests' worth of kernels (20) live in `video/*`:
  BMP legality, byte-determinism, palette bounds, focal anchors, graph
  builder, duration probe.
- The SCRIPTED engine is deliberately *stylized*, not photographic: it
  guarantees the offline/zero-dep floor; the endpoint tier upgrades
  fidelity when a model is configured. Fidelity-vs-offline is a settings
  decision, not a code fork.
- Future hooks (not built): multi-ref image editing (ladder top tier),
  per-character voice-to-image prompt fusion with craft personas, chapter
  crossfade transitions, WebM/AV1 output variants.

## Evidence

- Live CLI smoke: plan (`refreshed:false` off the hook) → render (4 assets,
  re-render sha-stable) → assemble (real ffmpeg, `auto:pipeline-audio`,
  243 KB / 9.0 s MP4) → validate (6/6 PASS, Δ15 ms vs plan) →
  `R2I_ENDPOINT` fail-closed.
- E2E: `tests/tier1_features/test_t1_video.py` T1.VD1–VD10 (205/205).
