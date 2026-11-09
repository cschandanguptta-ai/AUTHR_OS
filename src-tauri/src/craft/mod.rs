// AUTHR_OS — Craft subsystem (contract v1.3/v1.4): the offline knowledge +
// tooling layer that makes the runtime a one-person authoring & publishing
// house. See docs/CLI_CONTRACT.md §4.14/§4.15 and ADR-0009/ADR-0010.
//
//   catalog.rs   — 25 project formats · 37 style personas · 18 plot arcs ·
//                  KDP pricing constants (static, deterministic, tested)
//   engine.rs    — craft_state/chapter_beats storage + apply/scaffold/progress
//   kdp.rs       — storefront compliance + royalty projection + launch package
//   retention.rs — SFB-2.0 reader-retention kit: chapter beats, ending
//                  taxonomy, micro-arc cycle, QC rubric, worksheet, cadence

pub mod catalog;
pub mod engine;
pub mod kdp;
pub mod retention;
