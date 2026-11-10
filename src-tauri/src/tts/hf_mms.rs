// AUTHR_OS — HuggingFace MMS-TTS connector (Sprint 9: "HuggingFace MMS
// connectors"). MMS returns 16 kHz WAV; resampled to the unified 24 kHz
// chunk rate. Live use gated by AUTHR_TTS_LIVE=1; API key from keychain
// `authr.tts.huggingface/api_key`.

use crate::audio::dsp::fnv64;
use crate::audio::wav::{read_wav_bytes, resample};
use crate::error::AppError;
use crate::tts::engines::{OUTPUT_FS, SynthOutput, SynthRequest, TtsEngine};

pub const API_BASE: &str = "https://api-inference.huggingface.co/models";
pub const DEFAULT_MODEL: &str = "facebook/mms-tts-eng";
pub const KEYCHAIN_SERVICE: &str = "authr.tts.huggingface";
pub const KEYCHAIN_KEY: &str = "api_key";
pub const TIMEOUT_SECS: u64 = 30;

pub struct HfMms {
    pub model: String,
}

pub fn model_url(model: &str) -> String {
    let m = if model.is_empty() { DEFAULT_MODEL } else { model };
    format!("{API_BASE}/{m}")
}

fn api_key() -> Result<String, AppError> {
    let kc = crate::security::keychain::backend()?;
    match kc.get(KEYCHAIN_SERVICE, KEYCHAIN_KEY) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(AppError::security(format!(
            "HuggingFace API key missing — keychain `{KEYCHAIN_SERVICE}/{KEYCHAIN_KEY}`"
        ))),
    }
}

impl TtsEngine for HfMms {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError> {
        let key = api_key()?;
        let model = if self.model.is_empty() || self.model == "local_default" {
            DEFAULT_MODEL.to_string()
        } else if self.model.contains('/') {
            self.model.clone()
        } else {
            format!("{DEFAULT_MODEL}-{}", self.model)
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::storage(format!("hf client: {e}")))?;
        let resp = client
            .post(model_url(&model))
            .header("Authorization", &format!("Bearer {key}"))
            .json(&serde_json::json!({ "inputs": req.text }))
            .send()
            .map_err(|e| AppError::storage(format!("hf request: {e}")))?;
        let status = resp.status();
        let bytes = resp.bytes().map_err(|e| AppError::storage(format!("hf body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::storage(format!(
                "hf HTTP {}: {}",
                status.as_u16(),
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            )));
        }
        let (mono, fs) = read_wav_bytes(&bytes)?;
        let samples = resample(&mono, fs, OUTPUT_FS);
        Ok(SynthOutput {
            samples,
            fs: OUTPUT_FS,
            request_id: format!("hf_{:016x}", fnv64(req.text.as_bytes())),
            engine: "HF_MMS".into(),
            degraded: false,
            note: format!("live hf-mms `{model}`"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_shape() {
        assert_eq!(model_url("facebook/mms-tts-eng"),
                   "https://api-inference.huggingface.co/models/facebook/mms-tts-eng");
        assert_eq!(model_url(""), format!("{API_BASE}/{DEFAULT_MODEL}"));
    }
}
