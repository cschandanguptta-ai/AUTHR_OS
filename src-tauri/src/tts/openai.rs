// AUTHR_OS — OpenAI TTS connector (Sprint 9). Live use gated by
// AUTHR_TTS_LIVE=1; API key from keychain `authr.tts.openai/api_key`.

use crate::audio::dsp::fnv64;
use crate::tts::elevenlabs::pcm16le_to_f32;
use crate::error::AppError;
use crate::tts::engines::{OUTPUT_FS, SynthOutput, SynthRequest, TtsEngine};

pub const API_BASE: &str = "https://api.openai.com/v1/audio/speech";
pub const DEFAULT_MODEL: &str = "tts-1";
pub const KEYCHAIN_SERVICE: &str = "authr.tts.openai";
pub const KEYCHAIN_KEY: &str = "api_key";
pub const TIMEOUT_SECS: u64 = 30;

pub struct OpenAiTts {
    pub voice_id: String,
}

pub fn request_body(req: &SynthRequest, model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "voice": req.voice_id,
        "input": req.text,
        "response_format": "pcm",
        "speed": req.rate.clamp(0.25, 4.0),
    })
}

fn api_key() -> Result<String, AppError> {
    let kc = crate::security::keychain::backend()?;
    match kc.get(KEYCHAIN_SERVICE, KEYCHAIN_KEY) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(AppError::security(format!(
            "OpenAI API key missing — keychain `{KEYCHAIN_SERVICE}/{KEYCHAIN_KEY}`"
        ))),
    }
}

impl TtsEngine for OpenAiTts {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError> {
        let key = api_key()?;
        let body = request_body(req, DEFAULT_MODEL);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::storage(format!("openai client: {e}")))?;
        let resp = client
            .post(API_BASE)
            .header("Authorization", &format!("Bearer {key}"))
            .json(&body)
            .send()
            .map_err(|e| AppError::storage(format!("openai request: {e}")))?;
        let status = resp.status();
        let bytes = resp.bytes().map_err(|e| AppError::storage(format!("openai body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::storage(format!(
                "openai HTTP {}: {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            )));
        }
        // pcm response = 24 kHz signed 16-bit LE mono
        Ok(SynthOutput {
            samples: pcm16le_to_f32(&bytes),
            fs: OUTPUT_FS,
            request_id: format!("openai_{:016x}", fnv64(req.text.as_bytes())),
            engine: "OPENAI_TTS".into(),
            degraded: false,
            note: format!("live openai `{}`", self.voice_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_pins_pcm_and_speed() {
        let req = SynthRequest {
            text: "hi".into(),
            speaker: "s".into(),
            voice_id: "alloy".into(),
            rate: 1.4,
            pitch: 1.0,
            emotion: "neutral".into(),
            condition: None,
            prev_request_id: None,
        };
        let b = request_body(&req, DEFAULT_MODEL);
        assert_eq!(b["response_format"], serde_json::json!("pcm"));
        assert_eq!(b["speed"], serde_json::json!(1.4));
        assert_eq!(b["voice"], serde_json::json!("alloy"));
    }
}
