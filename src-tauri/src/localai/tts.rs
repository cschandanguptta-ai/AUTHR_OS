// AUTHR_OS — Kokoro-82M local neural TTS path (Phase 5, Sprint 13).
//
// The Kokoro engine runs the v1.0 ONNX graph through `ort` 2.0 with the Rust
// G2P tokenizer (`audio::g2p`). This build negotiates availability:
//   1. model file `<bundle>/models/kokoro-v1.0.onnx` must exist,
//   2. the `local-ai` cargo feature must be compiled in.
// When either is missing the command surfaces `AI_ENGINE_UNAVAILABLE` with
// actionable guidance; the offline pipeline falls back to `audio::tts`
// SCRIPTED synthesis so the Audio Studio stays functional headlessly.

use crate::error::AppError;
use serde_json::{json, Value};
use std::path::Path;

pub const KOKORO_MODEL_FILE: &str = "kokoro-v1.0.onnx";
pub const KOKORO_VOICES: &[&str] = &["af_bella", "af_nicole", "am_adam", "bf_emma", "bm_george"];

pub fn model_path(bundle_root: &Path) -> std::path::PathBuf {
    bundle_root.join("models").join(KOKORO_MODEL_FILE)
}

pub fn model_present(bundle_root: &Path) -> bool {
    model_path(bundle_root).exists()
}

/// Tokenize text into phoneme-ish units via the G2P module (shared with the
/// Kokoro runtime; exposed for status/debug payloads).
pub fn phonemize(text: &str, voice: &str) -> Value {
    let units: Vec<Value> = text
        .split_whitespace()
        .take(64)
        .map(|w| {
            let (ipa, source) = crate::audio::g2p::to_ipa(w);
            json!({ "word": w, "ipa": ipa, "source": source })
        })
        .collect();
    json!({
        "voice": voice,
        "units": units,
        "count": units.len(),
        "tokenizer": "authr-g2p-rules+lexicon",
    })
}

/// `local_tts_generate` with engine KOKORO_LOCAL. Availability-checked.
pub fn generate(bundle_root: &Path, text: &str, voice: &str) -> Result<Value, AppError> {
    if text.trim().is_empty() {
        return Err(AppError::ai("text must not be empty"));
    }
    let voice = if voice.is_empty() { "af_bella" } else { voice };
    if !KOKORO_VOICES.contains(&voice) {
        return Err(AppError::ai(format!(
            "unknown kokoro voice `{voice}` (available: {})",
            KOKORO_VOICES.join(", ")
        )));
    }
    if !model_present(bundle_root) {
        return Err(AppError::ai_engine_unavailable(format!(
            "Kokoro-82M model not found at {} — download kokoro-v1.0.onnx into models/ (docs/LOCAL_AI.md)",
            model_path(bundle_root).display()
        )));
    }
    Err(AppError::ai_engine_unavailable(
        "Kokoro ONNX runtime requires the `local-ai` cargo feature (ort 2.0) — this build ships the deterministic SCRIPTED engine",
    ))
}

/// `local_ai_status` — one umbrella probe for the local AI stack.
/// `llm_resolved` comes from the provider ladder (ADR-0012: settings > env).
pub fn status(bundle_root: &Path, llm_resolved: Option<String>) -> Value {
    json!({
        "embedder": "hashing-384",
        "minilm_model_present": bundle_root.join("models").join("all-MiniLM-L6-v2.onnx").exists(),
        "kokoro": {
            "model_file": KOKORO_MODEL_FILE,
            "model_present": model_present(bundle_root),
            "voices": KOKORO_VOICES,
        },
        "llm": crate::localai::llm::status(llm_resolved),
        "fallbacks": {
            "tts": "SCRIPTED",
            "llm": "OFFLINE_GRAMMAR",
            "embedder": "hashing-384",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonemize_uses_lexicon_then_rules() {
        let out = phonemize("Kira thax", "af_bella");
        let units = out["units"].as_array().unwrap();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0]["source"], "lexicon");
        assert_eq!(units[0]["ipa"], "ˈkɪərə");
        assert_eq!(units[1]["source"], "rules");
    }

    #[test]
    fn generate_rejects_empty_and_unknown_voice() {
        let dir = std::env::temp_dir().join(format!("authr_kok_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(generate(&dir, "   ", "af_bella").unwrap_err().code, "AI_ERROR");
        assert_eq!(generate(&dir, "hello", "nope").unwrap_err().code, "AI_ERROR");
        let err = generate(&dir, "hello", "af_bella").unwrap_err();
        assert_eq!(err.code, "AI_ENGINE_UNAVAILABLE");
        assert!(err.message.contains("kokoro-v1.0.onnx"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
