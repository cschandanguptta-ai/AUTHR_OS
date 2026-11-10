//! Typed IPC payload DTOs. Enums enforce the same value domains as the
//! SQLite CHECK constraints, so invalid payloads are rejected at the
//! validation layer before touching the database.

use crate::models::ast::SegmentType;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Empty {}

#[derive(Deserialize)]
pub struct ProjectMount {
    pub session_id: Option<String>,
    pub lease_secs: Option<i64>,
}

#[derive(Deserialize)]
pub struct SessionPayload {
    pub session_id: String,
}

#[derive(Deserialize)]
pub struct ProjectExport {
    pub out: String,
}

/// Phase 2 addition: `project_git_log` payload (additive to the Phase 1
/// surface; see `docs/DESKTOP_SHELL.md`).
#[derive(Deserialize)]
pub struct GitLogPayload {
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct IdPayload {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct BookIdPayload {
    pub book_id: i64,
}

#[derive(Deserialize)]
pub struct ChapterIdPayload {
    pub chapter_id: i64,
}

#[derive(Deserialize)]
pub struct PathPayload {
    pub path: String,
}

// ---------- enums (mirror the schema CHECK domains) ----------

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrativeStructure {
    ThreeAct,
    SaveTheCat,
    FichteanCurve,
    HerosJourney,
    Kishotenketsu,
    SevenPoint,
    NonLinear,
}

impl NarrativeStructure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThreeAct => "three_act",
            Self::SaveTheCat => "save_the_cat",
            Self::FichteanCurve => "fichtean_curve",
            Self::HerosJourney => "heros_journey",
            Self::Kishotenketsu => "kishotenketsu",
            Self::SevenPoint => "seven_point",
            Self::NonLinear => "non_linear",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookStatus {
    Draft,
    Outlining,
    Drafting,
    Revising,
    Polishing,
    Published,
    Archived,
}

impl BookStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Outlining => "outlining",
            Self::Drafting => "drafting",
            Self::Revising => "revising",
            Self::Polishing => "polishing",
            Self::Published => "published",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterStatus {
    Outline,
    Draft,
    Revised,
    Final,
}

impl ChapterStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Draft => "draft",
            Self::Revised => "revised",
            Self::Final => "final",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterRole {
    Protagonist,
    Antagonist,
    Secondary,
    Supporting,
    Narrator,
    Minor,
}

impl CharacterRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Protagonist => "protagonist",
            Self::Antagonist => "antagonist",
            Self::Secondary => "secondary",
            Self::Supporting => "supporting",
            Self::Narrator => "narrator",
            Self::Minor => "minor",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceEngine {
    Kokoro,
    EdgeTts,
    KittenTts,
    HfTts,
    Elevenlabs,
    WindowsSapi,
    MacosAvspeech,
    None,
}

impl VoiceEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kokoro => "kokoro",
            Self::EdgeTts => "edge_tts",
            Self::KittenTts => "kitten_tts",
            Self::HfTts => "hf_tts",
            Self::Elevenlabs => "elevenlabs",
            Self::WindowsSapi => "windows_sapi",
            Self::MacosAvspeech => "macos_avspeech",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoreCategory {
    Location,
    Faction,
    Character,
    MagicSystem,
    Technology,
    Item,
    Event,
    Rule,
    Custom,
}

impl LoreCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Location => "location",
            Self::Faction => "faction",
            Self::Character => "character",
            Self::MagicSystem => "magic_system",
            Self::Technology => "technology",
            Self::Item => "item",
            Self::Event => "event",
            Self::Rule => "rule",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Planted,
    Active,
    PaidOff,
    Abandoned,
}

impl ThreadStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planted => "planted",
            Self::Active => "active",
            Self::PaidOff => "paid_off",
            Self::Abandoned => "abandoned",
        }
    }
}

// ---------- entity payloads ----------

#[derive(Deserialize)]
pub struct BookCreate {
    pub title: String,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub narrative_structure: Option<NarrativeStructure>,
    pub status: Option<BookStatus>,
    pub target_word_count: Option<i64>,
}

#[derive(Deserialize)]
pub struct BookUpdate {
    pub id: i64,
    pub title: Option<String>,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub narrative_structure: Option<NarrativeStructure>,
    pub status: Option<BookStatus>,
    pub target_word_count: Option<i64>,
}

#[derive(Deserialize)]
pub struct ChapterCreate {
    pub book_id: i64,
    pub chapter_index: Option<i64>,
    pub title: Option<String>,
    pub summary: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterUpdate {
    pub id: i64,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub status: Option<ChapterStatus>,
}

#[derive(Deserialize)]
pub struct CharacterCreate {
    pub book_id: i64,
    pub character_key: String,
    pub display_name: String,
    pub role: Option<CharacterRole>,
    pub voice_engine: Option<VoiceEngine>,
    pub voice_config: Option<serde_json::Value>,
    pub ipa_phonetic: Option<String>,
}

#[derive(Deserialize)]
pub struct CharacterUpdate {
    pub id: i64,
    pub character_key: Option<String>,
    pub display_name: Option<String>,
    pub role: Option<CharacterRole>,
    pub voice_engine: Option<VoiceEngine>,
    pub voice_config: Option<serde_json::Value>,
    pub ipa_phonetic: Option<String>,
}

#[derive(Deserialize)]
pub struct LoreCreate {
    pub book_id: i64,
    pub category: Option<LoreCategory>,
    pub term: String,
    pub content: Option<String>,
    pub ipa_phonetic: Option<String>,
}

#[derive(Deserialize)]
pub struct LoreUpdate {
    pub id: i64,
    pub category: Option<LoreCategory>,
    pub content: Option<String>,
    pub ipa_phonetic: Option<String>,
}

#[derive(Deserialize)]
pub struct ThreadCreate {
    pub book_id: i64,
    pub chapter_id: Option<i64>,
    pub title: String,
    pub status: Option<ThreadStatus>,
    pub planted_chapter_index: Option<i64>,
    pub payoff_chapter_index: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct ThreadUpdate {
    pub id: i64,
    pub status: Option<ThreadStatus>,
    pub payoff_chapter_index: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct SegmentCreate {
    pub chapter_id: i64,
    /// Defaults to `NARRATOR` (narration segments).
    pub speaker_id: Option<String>,
    pub segment_type: Option<SegmentType>,
    pub text_content: String,
    pub emotion: Option<String>,
    pub prosody_rate: Option<f64>,
    pub prosody_pitch: Option<f64>,
    pub ambient_cue: Option<String>,
    pub sfx_cue: Option<String>,
    pub audio_hash: Option<String>,
    pub sequence_order: Option<i64>,
}

#[derive(Deserialize)]
pub struct SegmentUpdate {
    pub id: i64,
    pub segment_type: Option<SegmentType>,
    pub speaker_id: Option<String>,
    pub emotion: Option<String>,
    pub prosody_rate: Option<f64>,
    pub prosody_pitch: Option<f64>,
    pub text_content: Option<String>,
    pub ambient_cue: Option<String>,
    pub sfx_cue: Option<String>,
    pub audio_hash: Option<String>,
}

#[derive(Deserialize)]
pub struct SegmentGet {
    pub id: Option<i64>,
    pub segment_id: Option<String>,
}

/// Additive desktop command (Phase 3): whole-chapter sequence re-index
/// after Lexical AST splits / merges / deletes. `order` must be a
/// permutation of every segment row id of the chapter (1-based index =
/// list position).
#[derive(Deserialize)]
pub struct SegmentReorder {
    pub chapter_id: i64,
    pub order: Vec<i64>,
}

#[derive(Deserialize)]
pub struct SearchPayload {
    pub query: String,
    pub limit: Option<i64>,
}
