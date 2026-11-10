// AUTHR_OS — Local LLM engine boundary (Phase 5 / Sprint 13).
//
// Engine matrix (selection via `AUTHR_LLM_ENGINE`, default `auto`):
//   offline-template — deterministic scaffold generator grounded in the
//                      Vector RAG context pack + Chekhov ledger. Zero
//                      dependencies, always available, 100% offline.
//   llama-cpp        — llama.cpp C FFI (feature `local-llm`), GPU-accelerated
//                      (Metal / DirectML / CUDA via the llama.cpp build),
//                      pinned prefix KV-cache + greedy sampling (§llama_ffi).
//                      Selected when compiled AND `AUTHR_LLAMA_MODEL` points
//                      at a GGUF model.
//
// `keep_alive = -1` semantics: the llama engine keeps model + context
// resident for the process lifetime and caches the tokenized static prefix,
// so repeated generations reuse the pinned KV state.

use crate::error::AppError;
use serde_json::{json, Value};

/// One generated beat — segment_append-compatible (minus chapter_id).
#[derive(Debug, Clone)]
pub struct Beat {
    pub segment_type: String,
    pub speaker_id: String,
    pub emotion: String,
    pub prosody_rate: f64,
    pub prosody_pitch: f64,
    pub text_content: String,
    pub ambient_cue: Option<String>,
    pub sfx_cue: Option<String>,
}

/// Retrieval hit handed to the engine for grounding.
#[derive(Debug, Clone)]
pub struct RagRef {
    pub source: String,
    pub ref_key: String,
    pub title: String,
    pub snippet: String,
}

/// Open story thread for Act III payoff injection.
#[derive(Debug, Clone)]
pub struct ThreadRef {
    pub thread_title: String,
    pub payoff_requirement: String,
}

/// Generation context assembled by the `ai_generate` command body.
pub struct GenContext {
    pub prompt: String,
    pub beats_requested: usize,
    pub premise: String,
    pub theme: String,
    pub genre: String,
    pub chapter_title: String,
    pub act_number: i64,
    pub rag_hits: Vec<RagRef>,
    pub open_threads: Vec<ThreadRef>,
    pub cast: Vec<(String, String)>, // (character_key, display_name)
    pub seed: u64,
}

pub trait LlmEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn model(&self) -> String;
    fn offline(&self) -> bool;
    fn generate(&self, ctx: &GenContext) -> Result<Vec<Beat>, AppError>;
}

// ── deterministic scaffold engine ────────────────────────────────────────────

#[derive(Default)]
pub struct OfflineTemplateEngine;

const NARRATION_T: &[&str] = &[
    "The {focus} held the scene like a held breath, and {lead} moved through it with the certainty of {theme}.",
    "Beyond the {lore}, {lead} read the weight of {theme} in every silence that followed.",
    "{lead} traced the edges of {focus}, and the world narrowed to a single point of intent.",
    "Around {focus}, the air itself seemed to remember what {theme} cost, and {lead} paid it no mind.",
    "The story of {focus} turned once more, and {lead} stood at its hinge.",
];

const DIALOGUE_T: &[&str] = &[
    "\"{lead}, listen to me — {focus} is not what they told you,\" {speaker} said, voice low.",
    "\"Whatever happens with {focus}, we do it together,\" {speaker} said, jaw tight with {emotion}.",
    "\"I have seen what {focus} leaves behind,\" {speaker} said. \"We finish this.\"",
    "\"Tell me the truth about {focus},\" {speaker} said, and waited, steady as {theme}.",
];

const MONOLOGUE_T: &[&str] = &[
    "*{lead} thought: if {focus} fails, everything after tonight is written by someone else.*",
    "*Somewhere beneath the fear of {focus}, {lead} found the shape of {theme}, and held it.*",
    "*It always came back to {focus} — and to {theme}, which had never once let go.*",
];

const EMOTIONS: &[&str] = &["neutral", "tense", "resolute", "wary", "urgent", "reflective"];
const AMBIENTS: &[&str] = &["cyberpunk_rain_heavy", "low_wind_open_plain", "humming_machinery", "distant_alarm_fade", "quiet_room_tone"];
const SFX: &[&str] = &["airlock_cycle", "glass_crack_near", "footsteps_gravel", "panel_hiss", "signal_chirp"];

impl LlmEngine for OfflineTemplateEngine {
    fn name(&self) -> &'static str {
        "offline-template"
    }

    fn model(&self) -> String {
        "heuristic-scaffold-v1".into()
    }

    fn offline(&self) -> bool {
        true
    }

    fn generate(&self, ctx: &GenContext) -> Result<Vec<Beat>, AppError> {
        if ctx.beats_requested == 0 {
            return Err(AppError::storage("beats must be >= 1"));
        }

        // Deterministic RNG (xorshift seeded from seed — same inputs ⇒ same
        // beats, verifiable across processes).
        let mut rng = ctx.seed | 1;

        let lead = ctx
            .cast
            .first()
            .map(|(k, d)| if k == "NARRATOR" { d.clone() } else { d.clone() })
            .unwrap_or_else(|| "The narrator".into());
        // Focus prefers a lore title (a world anchor), then the chapter,
        // then a neutral placeholder — never raw segment titles.
        let lore_hit = ctx
            .rag_hits
            .iter()
            .find(|h| h.source == "lore" && !h.title.trim().is_empty());
        let focus = lore_hit
            .map(|h| h.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| {
                if ctx.chapter_title.trim().is_empty() {
                    "the unknown".into()
                } else {
                    ctx.chapter_title.clone()
                }
            });
        let lore_title = lore_hit
            .map(|h| h.title.clone())
            .unwrap_or_else(|| focus.clone());
        let theme_fallback = if ctx.theme.trim().is_empty() {
            if ctx.premise.trim().is_empty() {
                "the story".into()
            } else {
                ctx.premise.clone()
            }
        } else {
            ctx.theme.clone()
        };

        // Dialogue/monologue speakers exclude the narrator when a cast exists.
        let speakers: Vec<(String, String)> = {
            let voiced: Vec<_> = ctx
                .cast
                .iter()
                .filter(|(k, _)| k != "NARRATOR")
                .cloned()
                .collect();
            if voiced.is_empty() {
                vec![("NARRATOR".into(), "Narrator".into())]
            } else {
                voiced
            }
        };

        // 4-beat scene cycle (§5.2 rhythm): narration → dialogue → narration
        // → internal monologue, repeated.
        let mut beats = Vec::with_capacity(ctx.beats_requested);
        for i in 0..ctx.beats_requested {
            let kind = match i % 4 {
                0 => "NARRATION",
                1 => "DIALOGUE",
                2 => "NARRATION",
                _ => "INTERNAL_MONOLOGUE",
            };
            let emotion = EMOTIONS[next(&mut rng, EMOTIONS.len())].to_string();
            let (speaker_key, speaker_display) = if kind == "NARRATION" {
                ("NARRATOR".to_string(), "Narrator".to_string())
            } else {
                let idx = next(&mut rng, speakers.len());
                speakers[idx].clone()
            };
            let template = match kind {
                "NARRATION" => NARRATION_T[next(&mut rng, NARRATION_T.len())],
                "DIALOGUE" => DIALOGUE_T[next(&mut rng, DIALOGUE_T.len())],
                _ => MONOLOGUE_T[next(&mut rng, MONOLOGUE_T.len())],
            };
            let text = template
                .replace("{lead}", &lead)
                .replace("{speaker}", &speaker_display)
                .replace("{focus}", &focus)
                .replace("{lore}", &lore_title)
                .replace("{theme}", &theme_fallback)
                .replace("{emotion}", &emotion);

            // Act III: inject the Chekhov payoff requirement into the first
            // beat (Sprint 5 contract honored by the local engine too).
            if i == 0 && ctx.act_number >= 3 {
                if let Some(t) = ctx.open_threads.first() {
                    beats.push(Beat {
                        segment_type: "NARRATION".into(),
                        speaker_id: "NARRATOR".into(),
                        emotion: "resolute".into(),
                        prosody_rate: 1.0,
                        prosody_pitch: 1.0,
                        text_content: format!(
                            "{text} The debt of \"{}\" finally comes due: {}.",
                            t.thread_title, t.payoff_requirement
                        ),
                        ambient_cue: Some(AMBIENTS[next(&mut rng, AMBIENTS.len())].into()),
                        sfx_cue: None,
                    });
                    continue;
                }
            }

            beats.push(Beat {
                segment_type: kind.into(),
                speaker_id: speaker_key,
                emotion,
                prosody_rate: 1.0 + (next(&mut rng, 6) as f64 - 2.5) / 100.0,
                prosody_pitch: 1.0 + (next(&mut rng, 6) as f64 - 2.5) / 100.0,
                text_content: text,
                ambient_cue: match kind {
                    "NARRATION" => Some(AMBIENTS[next(&mut rng, AMBIENTS.len())].into()),
                    _ => None,
                },
                sfx_cue: match kind {
                    "DIALOGUE" => Some(SFX[next(&mut rng, SFX.len())].into()),
                    _ => None,
                },
            });
        }
        Ok(beats)
    }
}

fn next(rng: &mut u64, bound: usize) -> usize {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    (*rng >> 11) as usize % bound.max(1)
}

// ── engine selection ─────────────────────────────────────────────────────────

/// Resolve the active LLM engine from `AUTHR_LLM_ENGINE`:
/// `auto` → llama-cpp when compiled with `local-llm` AND `AUTHR_LLAMA_MODEL`
/// is set; otherwise the offline scaffold engine.
pub fn default_engine() -> Result<std::sync::Arc<dyn LlmEngine>, AppError> {
    let choice = std::env::var("AUTHR_LLM_ENGINE").unwrap_or_else(|_| "auto".into());
    match choice.as_str() {
        "auto" => {
            if llama_available() {
                Ok(llama_engine()?)
            } else {
                Ok(std::sync::Arc::new(OfflineTemplateEngine))
            }
        }
        "offline-template" => Ok(std::sync::Arc::new(OfflineTemplateEngine)),
        "llama-cpp" => llama_engine(),
        other => Err(AppError::storage(format!(
            "unknown llm engine `{other}` — available: offline-template{}",
            if llama_available() { ", llama-cpp" } else { "" }
        ))),
    }
}

/// The llama engine is selectable only when compiled AND a model is wired.
#[cfg(feature = "local-llm")]
fn llama_available() -> bool {
    std::env::var("AUTHR_LLAMA_MODEL").is_ok()
}

#[cfg(not(feature = "local-llm"))]
fn llama_available() -> bool {
    false
}

#[cfg(feature = "local-llm")]
fn llama_engine() -> Result<std::sync::Arc<dyn LlmEngine>, AppError> {
    Ok(std::sync::Arc::new(crate::ai::llama_ffi::LlamaCppEngine::from_env()?))
}

#[cfg(not(feature = "local-llm"))]
fn llama_engine() -> Result<std::sync::Arc<dyn LlmEngine>, AppError> {
    Err(AppError::storage(
        "AUTHR_LLM_ENGINE=llama-cpp requires a build with `--features local-llm`",
    ))
}

/// Engine registry for `ai_status` (availability reflects compile features).
pub fn registry() -> Vec<Value> {
    let mut v = vec![json!({
        "id": "offline-template",
        "name": "offline-template",
        "model": "heuristic-scaffold-v1",
        "offline": true,
        "available": true,
    })];
    v.push(json!({
        "id": "llama-cpp",
        "name": "llama-cpp",
        "model": std::env::var("AUTHR_LLAMA_MODEL").unwrap_or_default(),
        "offline": true,
        "available": cfg!(feature = "local-llm"),
    }));
    v
}
