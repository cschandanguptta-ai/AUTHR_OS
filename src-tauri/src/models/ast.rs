//! The co-synchronized Narrative & Audio AST data contract
//! (`AUTHR_DESKTOP_PRODUCTION_BUILD_PLAN.md` §5.2).
//!
//! This is the authoritative structure streamed across the Tauri IPC bridge
//! between the Rust Native Core and the React 19 frontend, and persisted in
//! the `chapter_segments` table / shadow `.ast.json` layer.

use serde::{Deserialize, Serialize};

/// Kind of narrative segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SegmentType {
    Narration,
    Dialogue,
    InternalMonologue,
}

impl SegmentType {
    pub fn as_str(self) -> &'static str {
        match self {
            SegmentType::Narration => "NARRATION",
            SegmentType::Dialogue => "DIALOGUE",
            SegmentType::InternalMonologue => "INTERNAL_MONOLOGUE",
        }
    }
}

/// One narrative segment: literary prose or spoken dialogue bound, at the
/// moment of creation, to its speaker, emotional tone, prosody parameters,
/// Foley/ambient cues and audio timeline metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NarrativeSegmentAst {
    /// UUID v4 canonical key of the segment.
    pub segment_id: String,
    /// Relational reference to `chapters.id`.
    pub chapter_id: i64,
    /// 0-indexed position within the chapter.
    pub sequence_order: i64,
    /// `NARRATION` | `DIALOGUE` | `INTERNAL_MONOLOGUE`.
    pub segment_type: SegmentType,
    /// Canonical speaker key: `NARRATOR`, `CHAR_KIRA`, ...
    pub speaker_id: String,
    /// e.g. `neutral`, `tense`, `whisper`, `cold_anger`.
    pub emotion: String,
    /// 0.8 to 1.25 (default 1.0).
    pub prosody_rate: f64,
    /// 0.85 to 1.15 (default 1.0).
    pub prosody_pitch: f64,
    /// Literary prose or spoken dialogue.
    pub text_content: String,
    /// Continuous ambient loop cue, e.g. `cyberpunk_rain_heavy`.
    pub ambient_cue: Option<String>,
    /// One-shot Foley cue, e.g. `thermal_cutter_ignite`.
    pub sfx_cue: Option<String>,
    /// Pointer to the speaker's prior segment for acoustic continuity latents.
    pub prev_request_id: Option<String>,
    /// SHA-256 content-addressable chunk cache key.
    pub audio_hash: Option<String>,
    /// Timeline offset for Read-Along SMIL sync (ms).
    pub audio_start_ms: i64,
    /// Duration in milliseconds.
    pub audio_duration_ms: i64,
}
