// AUTHR_OS — Cartesia Sonic REST connector (Sprint 9). Live use gated by
// AUTHR_TTS_LIVE=1; API key from keychain `authr.tts.cartesia/api_key`.

use super::engines::{OUTPUT_FS, SynthOutput, SynthRequest, TtsEngine};
use crate::audio::dsp::fnv64;
use crate::tts::elevenlabs::pcm16le_to_f32;
use crate::error::AppError;

pub const API_BASE: &str = "https://api.cartesia.ai";
pub const DEFAULT_MODEL: &str = "sonic-english";
pub const KEYCHAIN_SERVICE: &str = "authr.tts.cartesia";
pub const KEYCHAIN_KEY: &str = "api_key";
pub const TIMEOUT_SECS: u64 = 20;

pub struct Cartesia {
    pub voice_id: String,
}

/// Request body builder (unit-tested offline). Cartesia emits raw PCM when
/// `container=raw` + `format=pcm_s16le` at the requested sample rate.
pub fn request_body(req: &SynthRequest, model: &str) -> serde_json::Value {
    serde_json::json!({
        "transcript": req.text,
        "model_id": model,
        "voice": { "mode": "id", "id": req.voice_id },
        "output_format": {
            "container": "raw",
            "encoding": "pcm_s16le",
            "sample_rate": OUTPUT_FS,
        },
        "language": "en",
        // Cartesia context continuity: replay the prior audio id for this voice
        "_previous_request_id": req.prev_request_id,
    })
}

fn api_key() -> Result<String, AppError> {
    let kc = crate::security::keychain::backend()?;
    match kc.get(KEYCHAIN_SERVICE, KEYCHAIN_KEY) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(AppError::security(format!(
            "Cartesia API key missing — keychain `{KEYCHAIN_SERVICE}/{KEYCHAIN_KEY}`"
        ))),
    }
}

impl TtsEngine for Cartesia {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError> {
        let key = api_key()?;
        let url = format!("{API_BASE}/tts/bytes?model={DEFAULT_MODEL}&output_format=raw&voice_id={}", self.voice_id);
        let body = request_body(req, DEFAULT_MODEL);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::storage(format!("cartesia client: {e}")))?;
        let resp = client
            .post(&url)
            .header("X-API-Key", &key)
            .header("Cartesia-Version", "2024-06-10")
            .json(&body)
            .send()
            .map_err(|e| AppError::storage(format!("cartesia request: {e}")))?;
        let status = resp.status();
        let request_id = resp
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = resp.bytes().map_err(|e| AppError::storage(format!("cartesia body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::storage(format!(
                "cartesia HTTP {}: {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            )));
        }
        Ok(SynthOutput {
            samples: pcm16le_to_f32(&bytes),
            fs: OUTPUT_FS,
            request_id: if request_id.is_empty() {
                format!("cartesia_{:016x}", fnv64(req.text.as_bytes()))
            } else {
                request_id
            },
            engine: "CARTESIA".into(),
            degraded: false,
            note: format!("live cartesia `{}`", self.voice_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_pins_raw_pcm_24k() {
        let req = SynthRequest {
            text: "x".into(),
            speaker: "s".into(),
            voice_id: "sonic-voice".into(),
            rate: 1.0,
            pitch: 1.0,
            emotion: "neutral".into(),
            condition: None,
            prev_request_id: Some("prev-1".into()),
        };
        let b = request_body(&req, DEFAULT_MODEL);
        assert_eq!(b["output_format"]["sample_rate"], serde_json::json!(OUTPUT_FS));
        assert_eq!(b["output_format"]["encoding"], serde_json::json!("pcm_s16le"));
        assert_eq!(b["_previous_request_id"], serde_json::json!("prev-1"));
        assert_eq!(b["voice"]["id"], serde_json::json!("sonic-voice"));
    }
}
