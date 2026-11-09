// AUTHR_OS — Craft subsystem (contract v1.3): the offline knowledge +
// tooling layer that makes the runtime a one-person authoring & publishing
// house. See docs/CLI_CONTRACT.md §4.14 and ADR-0009.
//
//   catalog.rs — 25 project formats · 36 style personas · 18 plot arcs ·
//                KDP pricing constants (static, deterministic, tested)
//   engine.rs  — craft_state/chapter_beats storage + apply/scaffold/progress
//   kdp.rs     — storefront compliance + royalty projection + launch package

pub mod catalog;
pub mod engine;
pub mod kdp;
