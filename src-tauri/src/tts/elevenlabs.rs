// AUTHR_OS — ElevenLabs REST connector with `previous_request_id` acoustic
// continuity (Sprint 10). API key comes from the OS keychain (service
// `authr.tts.elevenlabs`, key `api_key`); live use gated by AUTHR_TTS_LIVE=1.

use super::engines::{OUTPUT_FS, SynthOutput, SynthRequest, TtsEngine};
use crate::audio::dsp::fnv64;
use crate::error::AppError;

pub const API_BASE: &str = "https://api.elevenlabs.io/v1";
pub const DEFAULT_MODEL: &str = "eleven_multilingual_v2";
pub const KEYCHAIN_SERVICE: &str = "authr.tts.elevenlabs";
pub const KEYCHAIN_KEY: &str = "api_key";
pub const TIMEOUT_SECS: u64 = 20;

pub struct ElevenLabs {
    pub voice_id: String,
}

/// Request body builder (unit-tested offline).
pub fn request_body(req: &SynthRequest, model: &str) -> serde_json::Value {
    serde_json::json!({
        "text": req.text,
        "model_id": model,
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.75,
            "speed": req.rate.clamp(0.7, 1.2),
        },
        // acoustic continuity: condition this generation on the speaker's
        // previous request (§6.3 / Sprint 10)
        "previous_request_id": req.prev_request_id,
    })
}

pub fn output_format_param(_rate: f64, _pitch: f64) -> String {
    format!("pcm_{}", OUTPUT_FS)
}

fn api_key() -> Result<String, AppError> {
    let kc = crate::security::keychain::backend()?;
    match kc.get(KEYCHAIN_SERVICE, KEYCHAIN_KEY) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(AppError::security(format!(
            "ElevenLabs API key missing — keychain `{KEYCHAIN_SERVICE}/{KEYCHAIN_KEY}`"
        ))),
    }
}

impl TtsEngine for ElevenLabs {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError> {
        let key = api_key()?;
        let url = format!("{API_BASE}/text-to-speech/{}?output_format={}", self.voice_id,
                          output_format_param(req.rate, req.pitch));
        let body = request_body(req, DEFAULT_MODEL);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::storage(format!("elevenlabs client: {e}")))?;
        let resp = client
            .post(&url)
            .header("xi-api-key", &key)
            .header("Accept", "audio/*")
            .json(&body)
            .send()
            .map_err(|e| AppError::storage(format!("elevenlabs request: {e}")))?;
        let request_id = resp
            .headers()
            .get("request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let status = resp.status();
        let bytes = resp.bytes().map_err(|e| AppError::storage(format!("elevenlabs body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::storage(format!(
                "elevenlabs HTTP {}: {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            )));
        }
        // pcm_24000 → raw signed 16-bit LE mono
        let samples = pcm16le_to_f32(&bytes);
        Ok(SynthOutput {
            samples,
            fs: OUTPUT_FS,
            request_id: if request_id.is_empty() {
                format!("eleven_{:016x}", fnv64(req.text.as_bytes()))
            } else {
                request_id
            },
            engine: "ELEVEN_LABS".into(),
            degraded: false,
            note: format!("live elevenlabs `{}`", self.voice_id),
        })
    }
}

pub fn pcm16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / i16::MAX as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(prev: Option<&str>) -> SynthRequest {
        SynthRequest {
            text: "Hello continuity".into(),
            speaker: "CHAR_KIRA".into(),
            voice_id: "EXAVIT".into(),
            rate: 1.0,
            pitch: 1.0,
            emotion: "neutral".into(),
            condition: None,
            prev_request_id: prev.map(str::to_string),
        }
    }

    #[test]
    fn body_carries_previous_request_id() {
        let b = request_body(&req(Some("abc123")), DEFAULT_MODEL);
        assert_eq!(b["previous_request_id"], serde_json::json!("abc123"));
        assert_eq!(b["model_id"], serde_json::json!(DEFAULT_MODEL));
        let b2 = request_body(&req(None), DEFAULT_MODEL);
        assert!(b2["previous_request_id"].is_null());
    }

    #[test]
    fn output_format_pins_24k() {
        assert_eq!(output_format_param(1.0, 1.0), "pcm_24000");
    }

    #[test]
    fn pcm16_decode_roundtrip() {
        let mut bytes = Vec::new();
        for v in [0i16, 16384, -16384, 32767] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let f = pcm16le_to_f32(&bytes);
        assert_eq!(f[0], 0.0);
        assert!((f[1] - 0.5).abs() < 0.001);
        assert!((f[2] + 0.5).abs() < 0.001);
    }
}
