// AUTHR_OS — Video novel studio (contract v1.5, ADR-0011).
//
// The audio studio for the eye: the same architecture as the audio pipeline,
// one medium over. NovelCast Studio's method (text → scenes/characters →
// speech + reference sheets + scene art → timed video) folded into the
// offline-first runtime:
//
//   derive.rs    — draft-time media derivation (the "no double work" hook:
//                  durations, scenes, cast mentions, narration paths —
//                  computed once on the draft write path, persisted, and
//                  consumed by every stage below without re-analysis)
//   frame.rs     — SCRIPTED image engine: deterministic BMP painter with an
//                  embedded 5×7 font, seeded palettes, scene cards /
//                  R2I-style reference sheets / title cards
//   render.rs    — engine negotiation (SCRIPTED | R2I_ENDPOINT fail-closed
//                  seam) + the `video_assets` registry
//   plan.rs      — read-only plan over derived state (ladder, narration
//                  readiness, timings)
//   assemble.rs  — one-pass ffmpeg MP4 (Ken Burns + focal anchors + narration
//                  reuse + SRT soft-mux), M4B-identical binary resolution
//   validate.rs  — kdp_check-shaped gate: plan vs registry vs encoded file
//
// Storage is additive only (`video_scenes`, `video_assets` — eager idempotent
// migrations, no frozen DDL touched, no §5 row shape changed) and reuses the
// existing canonical error codes (AUDIO_ERROR / BUNDLE_ERROR /
// AI_ENGINE_UNAVAILABLE). No new error codes.

pub mod assemble;
pub mod derive;
pub mod frame;
pub mod plan;
pub mod render;
pub mod validate;
