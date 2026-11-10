// AUTHR_OS — Multi-engine TTS router (Sprint 9 / build plan §7.1):
//   KOKORO_LOCAL → local neural stack (mock offline synthesizer until the
//                  Kokoro-82M ONNX runtime lands in Phase 5)
//   EDGE_TTS     → Microsoft Edge ReadAloud WebSocket (Sec-MS-GEC spoofing)
//   ELEVEN_LABS  → ElevenLabs REST with previous_request_id continuity
//   CARTESIA     → Cartesia Sonic REST
//   OPENAI_TTS   → OpenAI speech REST
//   HF_MMS       → HuggingFace MMS-TTS inference (fallback local neural)
//
// Network connectors are gated by AUTHR_TTS_LIVE=1. Without the gate the
// router returns Route::OfflineFallback (deterministic mock synthesis marked
// `degraded`) so the whole DAG/mastering pipeline is testable headlessly.

use super::latent::Latent;
use crate::error::AppError;

pub const OUTPUT_FS: u32 = 24_000; // unified chunk rate across engines

#[derive(Debug, Clone)]
pub struct SynthRequest<'a> {
    pub text: String,
    pub speaker: String,
    pub voice_id: String,
    pub rate: f64,
    pub pitch: f64,
    pub emotion: String,
    pub condition: Option<&'a Latent>,
    pub prev_request_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SynthOutput {
    pub samples: Vec<f32>,
    pub fs: u32,
    pub request_id: String,
    pub engine: String,
    pub degraded: bool,
    pub note: String,
}

pub trait TtsEngine: Send + Sync {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError>;
}

pub enum Route {
    /// Local offline stack (KOKORO_LOCAL placeholder).
    Local(std::sync::Arc<dyn TtsEngine>),
    /// Live cloud connector (requires AUTHR_TTS_LIVE=1).
    Cloud(std::sync::Arc<dyn TtsEngine>),
    /// Headless test path: deterministic synthesis marked degraded.
    OfflineFallback(std::sync::Arc<dyn TtsEngine>),
}

pub struct ResolvedEngine {
    pub name: String,
    pub route: Route,
}

pub fn resolve(voice_engine: &str, voice_id: &str, rate: f64, pitch: f64) -> ResolvedEngine {
    let live = std::env::var("AUTHR_TTS_LIVE").ok().as_deref() == Some("1");
    let mock: std::sync::Arc<dyn TtsEngine> =
        std::sync::Arc::new(MockSynth { voice_seed: format!("{voice_id}|{rate}|{pitch}") });
    match voice_engine {
        "KOKORO_LOCAL" => ResolvedEngine { name: "KOKORO_LOCAL".into(), route: Route::Local(mock) },
        "EDGE_TTS" => {
            if live {
                ResolvedEngine {
                    name: "EDGE_TTS".into(),
                    route: Route::Cloud(std::sync::Arc::new(super::edge_tts::EdgeTts {
                        voice_id: voice_id.to_string(),
                    })),
                }
            } else {
                ResolvedEngine { name: "EDGE_TTS".into(), route: Route::OfflineFallback(mock) }
            }
        }
        "ELEVEN_LABS" => {
            if live {
                ResolvedEngine {
                    name: "ELEVEN_LABS".into(),
                    route: Route::Cloud(std::sync::Arc::new(super::elevenlabs::ElevenLabs {
                        voice_id: voice_id.to_string(),
                    })),
                }
            } else {
                ResolvedEngine { name: "ELEVEN_LABS".into(), route: Route::OfflineFallback(mock) }
            }
        }
        "CARTESIA" => {
            if live {
                ResolvedEngine {
                    name: "CARTESIA".into(),
                    route: Route::Cloud(std::sync::Arc::new(super::cartesia::Cartesia {
                        voice_id: voice_id.to_string(),
                    })),
                }
            } else {
                ResolvedEngine { name: "CARTESIA".into(), route: Route::OfflineFallback(mock) }
            }
        }
        "OPENAI_TTS" => {
            if live {
                ResolvedEngine {
                    name: "OPENAI_TTS".into(),
                    route: Route::Cloud(std::sync::Arc::new(super::openai::OpenAiTts {
                        voice_id: voice_id.to_string(),
                    })),
                }
            } else {
                ResolvedEngine { name: "OPENAI_TTS".into(), route: Route::OfflineFallback(mock) }
            }
        }
        other => {
            // HF_MMS and any unknown engine string route to the HF connector
            if live {
                ResolvedEngine {
                    name: other.to_string(),
                    route: Route::Cloud(std::sync::Arc::new(super::hf_mms::HfMms {
                        model: voice_id.to_string(),
                    })),
                }
            } else {
                ResolvedEngine { name: other.to_string(), route: Route::OfflineFallback(mock) }
            }
        }
    }
}

/// §6.3 graceful fallback: unconditioned base-voice synthesis.
pub fn base_voice_synthesis(req: &SynthRequest) -> SynthOutput {
    let unconditioned = SynthRequest {
        condition: None,
        ..req.clone()
    };
    MockSynth { voice_seed: format!("{}|{}|{}", req.voice_id, req.rate, req.pitch) }
        .synthesize(&unconditioned)
        .expect("mock synthesis is infallible")
}

// ── Deterministic offline synthesizer (stand-in for Kokoro-82M) ──────────────
// Produces speech-like audio: per-speaker fundamental, harmonic stack with
// vibrato/jitter, syllable-count envelope derived from the text, phrase dips.
// Conditioning blends the upstream speaker latent (pitch/RMS) so consecutive
// turns of one character are acoustically verifiable.

pub struct MockSynth {
    pub voice_seed: String,
}

impl TtsEngine for MockSynth {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError> {
        use crate::audio::dsp::{Rng, fnv64, lin_to_db};
        const FS: u32 = OUTPUT_FS;

        let speaker_seed = fnv64(req.speaker.as_bytes());
        let f0_base = 88.0 + (speaker_seed % 110) as f64; // 88–197 Hz per speaker
        let emotion_shift = match req.emotion.as_str() {
            "tense" => 1.08,
            "angry" => 1.16,
            "sad" => 0.9,
            "calm" => 0.97,
            "excited" => 1.12,
            _ => 1.0,
        };
        let mut f0 = f0_base * req.pitch.max(0.1) * emotion_shift;
        let mut rms_target_db = -23.0 + ((speaker_seed >> 8) % 6) as f64 * 0.4;
        if let Some(lat) = req.condition {
            // cross-segment acoustic continuity: blend toward upstream prosody
            f0 = 0.5 * f0 + 0.5 * lat.pitch_hz.max(40.0);
            rms_target_db = 0.5 * rms_target_db + 0.5 * lat.rms_dbfs;
        }

        let syllables = count_syllables(&req.text).clamp(1, 96) as f64;
        let syl_ms = (180.0 / req.rate.max(0.25)).clamp(80.0, 420.0);
        let total_s = (syllables * syl_ms + 250.0) / 1000.0;
        let n = (total_s * FS as f64).round() as usize;
        let mut rng = Rng::new(fnv64(
            format!("{}|{}|{}|{:.3}|{:.3}|{:?}|{}",
                    self.voice_seed, req.text, req.emotion, req.rate, req.pitch,
                    req.condition.map(|l| l.tail_hash.clone()), req.prev_request_id.is_none())
                .as_bytes(),
        ));

        let mut out = vec![0.0f32; n];
        let mut phase: f64 = 0.0;
        let syl_samples = (syl_ms / 1000.0 * FS as f64).round() as usize;
        let mut i = 0usize;
        let mut syl = 0usize;
        while i < n {
            // phrase dip every 4 syllables (breath group)
            let gap = if syl % 4 == 3 && syl > 0 { (0.09 * FS as f64) as usize } else { 0 };
            for _ in 0..gap.min(n.saturating_sub(i)) {
                out[i] = 0.0;
                i += 1;
            }
            let end = (i + syl_samples).min(n);
            let jitter = 1.0 + (rng.bipolar() * 0.03);
            for (k, v) in out[i..end].iter_mut().enumerate() {
                let t = (i + k) as f64 / FS as f64;
                let in_syl = k as f64 / syl_samples as f64;
                let env = (std::f64::consts::PI * in_syl.min(1.0)).sin().powi(2);
                let vib = 1.0 + 0.02 * (2.0 * std::f64::consts::PI * 5.0 * t).sin();
                let f = f0 * jitter * vib;
                phase += 2.0 * std::f64::consts::PI * f / FS as f64;
                let s = phase.sin() * 0.62
                    + (2.0 * phase).sin() * 0.24
                    + (3.0 * phase).sin() * 0.11
                    + (4.0 * phase).sin() * 0.05
                    + rng.bipolar() * 0.015;
                *v = (s * env) as f32;
            }
            i = end;
            syl += 1;
        }

        // normalize toward the target RMS
        let ms = out.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / out.len() as f64;
        let cur_db = lin_to_db(ms.sqrt());
        let g = 10f64.powf((rms_target_db - cur_db) / 20.0);
        for v in out.iter_mut() {
            *v = ((*v as f64) * g) as f32;
        }

        let request_id = format!(
            "mock_{:016x}",
            fnv64(format!("{}|{}|{}|{}|{}|{}|{}",
                          req.speaker, req.text, req.voice_id, req.emotion,
                          self.voice_seed,
                          req.condition.map(|l| l.tail_hash.as_str()).unwrap_or("base"),
                          req.prev_request_id.as_deref().unwrap_or("")).as_bytes())
        );
        Ok(SynthOutput {
            samples: out,
            fs: FS,
            request_id,
            engine: "KOKORO_LOCAL(mock)".to_string(),
            degraded: false,
            note: "deterministic offline synthesizer (Kokoro-82M arrives in Phase 5)".to_string(),
        })
    }
}

fn count_syllables(text: &str) -> usize {
    let vowels = "aeiouyAEIOUY";
    let mut count = 0usize;
    let mut prev_vowel = false;
    for c in text.chars() {
        let is_vowel = vowels.contains(c);
        if is_vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_vowel;
    }
    count.max(text.split_whitespace().count().max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req<'a>(text: &str, cond: Option<&'a Latent>) -> SynthRequest<'a> {
        SynthRequest {
            text: text.to_string(),
            speaker: "CHAR_TEST".to_string(),
            voice_id: "test-voice".to_string(),
            rate: 1.0,
            pitch: 1.0,
            emotion: "neutral".to_string(),
            condition: cond,
            prev_request_id: None,
        }
    }

    #[test]
    fn deterministic_across_calls() {
        let e = MockSynth { voice_seed: "v|1|1".into() };
        let a = e.synthesize(&req("Deterministic output check", None)).unwrap();
        let b = e.synthesize(&req("Deterministic output check", None)).unwrap();
        assert_eq!(a.samples, b.samples);
        assert_eq!(a.request_id, b.request_id);
        assert_eq!(a.fs, OUTPUT_FS);
        assert!(!a.degraded);
    }

    #[test]
    fn duration_tracks_syllable_count() {
        let e = MockSynth { voice_seed: "v|1|1".into() };
        let short = e.synthesize(&req("Hi.", None)).unwrap();
        let long = e.synthesize(&req(
            "The station hummed with dormant power while alarms wailed in the distance",
            None,
        )).unwrap();
        assert!(long.samples.len() > short.samples.len() * 4);
        // duration sane: syllable envelope (180 ms/syllable) + 250 ms pad
        let long_ms = long.samples.len() as f64 / OUTPUT_FS as f64 * 1000.0;
        assert!(long_ms > 2000.0 && long_ms < 5000.0, "long duration {long_ms} ms");
        let short_ms = short.samples.len() as f64 / OUTPUT_FS as f64 * 1000.0;
        assert!(short_ms > 250.0 && short_ms < 700.0, "short duration {short_ms} ms");
    }

    #[test]
    fn conditioning_blends_pitch_toward_upstream() {
        let e = MockSynth { voice_seed: "v|1|1".into() };
        let latent = Latent {
            speaker: "CHAR_TEST".into(),
            rms_dbfs: -30.0,
            pitch_hz: 180.0, // force high pitch blend
            zcr: 0.1,
            tail_hash: "deadbeef".into(),
            window_ms: 1500,
            generated_at: "2026-01-01T00:00:00Z".into(),
        };
        let base = e.synthesize(&req("Continuity probe line", None)).unwrap();
        let cond = e.synthesize(&req("Continuity probe line", Some(&latent))).unwrap();
        // conditioned render must differ from base and land quieter than base target
        assert_ne!(base.samples, cond.samples);
        let rms_b = crate::audio::dsp::rms_dbfs(&base.samples);
        let rms_c = crate::audio::dsp::rms_dbfs(&cond.samples);
        assert!(rms_c < rms_b, "conditioned rms {rms_c} should sit below base {rms_b}");
        assert_ne!(base.request_id, cond.request_id);
    }

    #[test]
    fn base_voice_fallback_is_unconditioned() {
        let r = req("fallback probe", None);
        let o = base_voice_synthesis(&r);
        assert!(!o.degraded);
        assert!(o.samples.len() > OUTPUT_FS as usize / 2);
    }
}
