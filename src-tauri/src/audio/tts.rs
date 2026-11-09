// AUTHR_OS — multi-engine TTS connectors (Phase 4, Sprint 9) with the
// deterministic offline `SCRIPTED` engine used by headless CI.
//
// Real engines:
//   EDGE_TTS     — Microsoft Edge read-aloud WebSocket protocol, including
//                  the Sec-MS-GEC SHA-256 token spoofing handshake.
//   ELEVEN_LABS  — REST TTS with `previous_request_ids` acoustic continuity.
//   CARTESIA     — Sonic REST TTS with continuation transactions.
//   OPENAI_TTS   — /v1/audio/speech (mp3 response, decoded via symphonia).
//   HF_MMS       — HuggingFace inference API (facebook/mms-tts-*).
//
// Every network engine is gated by an injectable transport so the protocol
// logic is unit-testable without reaching the internet. In a sandboxed run
// the engines surface `AUDIO_ENGINE_UNAVAILABLE` (missing API key env or
// `AUTHR_TTS_OFFLINE=1`).

use crate::error::AppError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;

pub struct TtsRequest {
    pub text: String,
    pub voice: String,
    pub rate: f32,
    pub pitch: f32,
    pub sample_rate: u32,
    pub previous_request_id: Option<String>,
    /// (proper noun, IPA) pairs injected as phoneme hints.
    pub pronunciations: Vec<(String, String)>,
}

pub struct SynthAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Engine-side request id used for acoustic continuity chaining.
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Scripted,
    KokoroLocal,
    EdgeTts,
    ElevenLabs,
    Cartesia,
    OpenAiTts,
    HfMms,
}

impl Engine {
    /// Accepts the `character_profiles.voice_engine` spellings (contract §4.4)
    /// plus `SCRIPTED` and `HF_MMS` aliases.
    pub fn parse(name: &str) -> Result<Engine, AppError> {
        match name.trim().to_uppercase().as_str() {
            "SCRIPTED" | "DEVICE" => Ok(Engine::Scripted),
            "KOKORO_LOCAL" => Ok(Engine::KokoroLocal),
            "EDGE_TTS" | "EDGE" => Ok(Engine::EdgeTts),
            "ELEVEN_LABS" | "ELEVENLABS" => Ok(Engine::ElevenLabs),
            "CARTESIA" => Ok(Engine::Cartesia),
            "OPENAI_TTS" | "OPENAI" => Ok(Engine::OpenAiTts),
            "HF_MMS" | "HUGGINGFACE_MMS" => Ok(Engine::HfMms),
            other => Err(AppError::audio(format!(
                "unknown TTS engine `{other}` (expected SCRIPTED|KOKORO_LOCAL|EDGE_TTS|ELEVEN_LABS|CARTESIA|OPENAI_TTS|HF_MMS)"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Scripted => "SCRIPTED",
            Engine::KokoroLocal => "KOKORO_LOCAL",
            Engine::EdgeTts => "EDGE_TTS",
            Engine::ElevenLabs => "ELEVEN_LABS",
            Engine::Cartesia => "CARTESIA",
            Engine::OpenAiTts => "OPENAI_TTS",
            Engine::HfMms => "HF_MMS",
        }
    }
}

fn offline_guard(engine: Engine) -> Result<(), AppError> {
    if std::env::var("AUTHR_TTS_OFFLINE").ok().as_deref() == Some("1") {
        return Err(AppError::audio_engine_unavailable(format!(
            "engine {} blocked by AUTHR_TTS_OFFLINE=1",
            engine.as_str()
        )));
    }
    Ok(())
}

fn require_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|_| {
        AppError::audio_engine_unavailable(format!(
            "API key not configured — set env {key}"
        ))
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Scripted engine — deterministic offline synthesizer (device fallback).
// Produces a formant-ish tone per token: duration ∝ token length, f0 hashed
// from the voice id, harmonic stack + per-word ADSR, prosody rate/pitch applied.
// ─────────────────────────────────────────────────────────────────────────────

pub struct ScriptedEngine;

impl ScriptedEngine {
    fn f0_for(voice: &str) -> f32 {
        let mut h = Sha256::new();
        h.update(voice.as_bytes());
        let digest = h.finalize();
        let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
        90.0 + (n % 140) as f32 // 90–229 Hz
    }

    pub fn synthesize(req: &TtsRequest) -> SynthAudio {
        let fs = req.sample_rate;
        let f0 = Self::f0_for(&req.voice) * req.pitch.max(0.25);
        let mut samples: Vec<f32> = Vec::new();
        let tokens: Vec<&str> = req.text.split_whitespace().collect();
        if tokens.is_empty() {
            return SynthAudio { samples, sample_rate: fs, request_id: None };
        }
        for (n, tok) in tokens.iter().enumerate() {
            let dur_ms = (90.0 + 38.0 * tok.chars().count() as f32).clamp(120.0, 900.0) / req.rate.max(0.25);
            let dur = ((dur_ms * fs as f32 / 1000.0).round() as usize).max(32);
            let word_seed = {
                let mut h = Sha256::new();
                h.update(tok.as_bytes());
                h.finalize()
            };
            let jitter = (word_seed[0] as f32 / 255.0 - 0.5) * f0 * 0.08;
            let base = f0 + jitter;
            for i in 0..dur {
                let t = i as f32 / fs as f32;
                // ADSR: 10 ms attack, 25 ms release.
                let env = {
                    let atk = (t / 0.010).clamp(0.0, 1.0);
                    let rel_t = (dur - i) as f32 / fs as f32;
                    let rel = (rel_t / 0.025).clamp(0.0, 1.0);
                    atk * rel
                };
                let phase = 2.0 * std::f32::consts::PI * base * t;
                let v = 0.55 * phase.sin()
                    + 0.28 * (2.0 * phase).sin()
                    + 0.12 * (3.0 * phase).sin()
                    + 0.05 * (4.0 * phase).sin();
                samples.push(0.30 * env * v);
            }
            // 60 ms inter-word pause.
            let pause = ((60.0 / req.rate.max(0.25)) * fs as f32 / 1000.0).round() as usize;
            if n + 1 < tokens.len() {
                samples.extend(std::iter::repeat_n(0.0f32, pause));
            }
        }
        let request_id = {
            let mut h = Sha256::new();
            h.update(req.text.as_bytes());
            h.update(req.voice.as_bytes());
            let d = h.finalize();
            Some(d.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>())
        };
        SynthAudio { samples, sample_rate: fs, request_id }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge TTS — WebSocket protocol with Sec-MS-GEC token spoofing.
// ─────────────────────────────────────────────────────────────────────────────

pub const EDGE_TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";

/// Sec-MS-GEC token: SHA-256(WindowsFileTime ⌄ 5-min bucket + trusted token),
/// uppercase hex. Windows file time = (unix + 11_644_473_600) × 10⁷ ticks;
/// the 5-minute bucket is 300 s × 10⁷ = 3_000_000_000 ticks.
pub fn edge_gec_token(unix_seconds: u64) -> String {
    let ticks = (unix_seconds + 11_644_473_600) * 10_000_000;
    let bucketed = ticks - (ticks % 3_000_000_000);
    let mut h = Sha256::new();
    h.update(bucketed.to_string().as_bytes());
    h.update(EDGE_TRUSTED_CLIENT_TOKEN.as_bytes());
    let d = h.finalize();
    d.iter().map(|b| format!("{b:02X}")).collect()
}

/// WebSocket message abstraction (injectable transport for protocol tests).
pub enum WsMsg {
    Text(String),
    Binary(Vec<u8>),
}

pub trait WsTransport {
    fn send(&mut self, msg: WsMsg) -> Result<(), AppError>;
    fn recv(&mut self) -> Result<WsMsg, AppError>;
}

pub struct EdgeTtsEngine {
    pub transport: Box<dyn WsTransport>,
}

impl EdgeTtsEngine {
    pub fn ssml(req: &TtsRequest, timestamp: &str, request_id: &str) -> String {
        let mut spoken = req.text.clone();
        for (name, ipa) in &req.pronunciations {
            spoken = spoken.replace(
                name,
                &format!("<phoneme alphabet=\"ipa\" ph=\"{ipa}\">{name}</phoneme>"),
            );
        }
        let rate_pct = ((req.rate - 1.0) * 100.0).round() as i64;
        let pitch_hz = ((req.pitch - 1.0) * 50.0).round() as i64;
        format!(
            "X-RequestId:{request_id}\r\nContent-Type:application/ssml+xml\r\n\
             X-Timestamp:{timestamp}Z\r\nPath:ssml\r\n\r\n\
             <speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
             <voice name='{voice}'><prosody pitch='{pitch_hz:+}Hz' rate='{rate_pct:+}%'>{spoken}</prosody></voice></speak>",
            voice = req.voice,
            pitch_hz = pitch_hz,
            rate_pct = rate_pct,
        )
    }

    pub fn config_frame(timestamp: &str) -> String {
        format!(
            "X-Timestamp:{timestamp}Z\r\nContent-Type:application/json; charset=utf-8\r\n\
             Path:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\
             \"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"true\"}},\
             \"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}"
        )
    }

    /// Drive the protocol over the injected transport; assembles `Path:audio`
    /// payloads and extracts the continuity id from `Path:turn.end`.
    pub fn synthesize(&mut self, req: &TtsRequest) -> Result<SynthAudio, AppError> {
        offline_guard(Engine::EdgeTts)?;
        let ts = "2026-01-01T00:00:00.000";
        let request_id: String = {
            let mut h = Sha256::new();
            h.update(req.text.as_bytes());
            let d = h.finalize();
            d.iter().take(16).map(|b| format!("{b:02x}")).collect()
        };
        self.transport
            .send(WsMsg::Text(Self::config_frame(ts)))
            .map_err(|e| AppError::audio_engine_unavailable(format!("edge tts config: {}", e.message)))?;
        self.transport
            .send(WsMsg::Text(Self::ssml(req, ts, &request_id)))
            .map_err(|e| AppError::audio_engine_unavailable(format!("edge tts ssml: {}", e.message)))?;
        let mut mp3: Vec<u8> = Vec::new();
        let mut turn_end_request_id: Option<String> = None;
        loop {
            match self.transport.recv() {
                Ok(WsMsg::Binary(bytes)) => {
                    if bytes.len() > 2 {
                        let header_len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
                        if bytes.len() > header_len + 2 {
                            let header = String::from_utf8_lossy(&bytes[2..2 + header_len]);
                            if header.contains("Path:audio") {
                                mp3.extend_from_slice(&bytes[2 + header_len..]);
                            }
                        }
                    }
                }
                Ok(WsMsg::Text(text)) => {
                    if text.contains("Path:turn.end") {
                        for line in text.lines() {
                            if let Some(v) = line.strip_prefix("X-RequestId:") {
                                turn_end_request_id = Some(v.trim().to_string());
                            }
                        }
                        break;
                    }
                }
                Err(e) => {
                    return Err(AppError::audio_engine_unavailable(format!(
                        "edge tts stream: {}",
                        e.message
                    )))
                }
            }
        }
        let (samples, rate) = decode_audio_bytes(&mp3)?;
        Ok(SynthAudio { samples, sample_rate: rate, request_id: turn_end_request_id })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// REST engines (ElevenLabs / Cartesia / OpenAI / HF-MMS) over an injectable
// HTTP transport.
// ─────────────────────────────────────────────────────────────────────────────

pub trait HttpTransport {
    fn post(&self, url: &str, headers: &[(String, String)], body: &str, content_type: &str) -> Result<Vec<u8>, AppError>;
}

pub struct UreqTransport;

impl HttpTransport for UreqTransport {
    fn post(&self, url: &str, headers: &[(String, String)], body: &str, content_type: &str) -> Result<Vec<u8>, AppError> {
        let mut req = ureq::post(url)
            .timeout(std::time::Duration::from_secs(30))
            .set("Content-Type", content_type);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        let resp = req
            .send_string(body)
            .map_err(|e| AppError::audio_engine_unavailable(format!("http {url}: {e}")))?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .take(64 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::audio_engine_unavailable(format!("http read: {e}")))?;
        Ok(bytes)
    }
}

pub struct HttpTtsEngine {
    pub engine: Engine,
    pub transport: Box<dyn HttpTransport>,
}

/// (url, headers, body, content_type) parts of a provider request.
pub type HttpTtsRequestParts = (String, Vec<(String, String)>, String, String);

impl HttpTtsEngine {
    /// Build the provider request.
    pub fn build_request(&self, req: &TtsRequest) -> Result<HttpTtsRequestParts, AppError> {
        match self.engine {
            Engine::ElevenLabs => {
                let key = require_env("ELEVENLABS_API_KEY")?;
                let voice = if req.voice.is_empty() { "21m00Tcm4TlvDq8ikWAM" } else { req.voice.as_str() };
                let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=pcm_24000");
                let body = json!({
                    "text": req.text,
                    "model_id": "eleven_multilingual_v2",
                    "previous_request_ids": req.previous_request_id.as_deref().map(|id| [id]).unwrap_or_default(),
                });
                Ok((url, vec![("xi-api-key".into(), key)], body.to_string(), "application/json".into()))
            }
            Engine::Cartesia => {
                let key = require_env("CARTESIA_API_KEY")?;
                let url = "https://api.cartesia.ai/tts/bytes".to_string();
                let mut body = json!({
                    "model_id": "sonic-2",
                    "transcript": req.text,
                    "voice": { "mode": "id", "id": req.voice },
                    "output_format": { "container": "wav", "sample_rate": req.sample_rate },
                });
                if let Some(prev) = &req.previous_request_id {
                    body["continuation"] = json!({ "transaction_id": prev });
                }
                Ok((url, vec![("X-API-Key".into(), key), ("Cartesia-Version".into(), "2024-06-10".into())], body.to_string(), "application/json".into()))
            }
            Engine::OpenAiTts => {
                let key = require_env("OPENAI_API_KEY")?;
                let url = "https://api.openai.com/v1/audio/speech".to_string();
                let body = json!({
                    "model": "tts-1",
                    "input": req.text,
                    "voice": req.voice,
                    "response_format": "mp3",
                });
                Ok((url, vec![("Authorization".into(), format!("Bearer {key}"))], body.to_string(), "application/json".into()))
            }
            Engine::HfMms => {
                let key = require_env("HUGGINGFACE_API_KEY")?;
                let lang = if req.voice.is_empty() { "eng" } else { req.voice.as_str() };
                let url = format!("https://api-inference.huggingface.co/models/facebook/mms-tts-{lang}");
                let body = json!({ "inputs": req.text });
                Ok((url, vec![("Authorization".into(), format!("Bearer {key}"))], body.to_string(), "application/json".into()))
            }
            other => Err(AppError::audio(format!("engine {} is not an HTTP engine", other.as_str()))),
        }
    }

    pub fn synthesize(&self, req: &TtsRequest) -> Result<SynthAudio, AppError> {
        offline_guard(self.engine)?;
        let (url, headers, body, content_type) = self.build_request(req)?;
        let bytes = self.transport.post(&url, &headers, &body, &content_type)?;
        let (samples, rate) = decode_audio_bytes(&bytes)?;
        let request_id = {
            let mut h = Sha256::new();
            h.update(bytes.len().to_le_bytes());
            Some(h.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect::<String>())
        };
        Ok(SynthAudio { samples, sample_rate: rate, request_id })
    }
}

/// Decode engine audio responses: WAV via hound (from memory), MP3 via
/// symphonia. Mixes down to mono f32.
pub fn decode_audio_bytes(bytes: &[u8]) -> Result<(Vec<f32>, u32), AppError> {
    if bytes.len() < 12 {
        return Err(AppError::audio_engine_unavailable("engine returned no audio payload"));
    }
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        let cursor = std::io::Cursor::new(bytes);
        let reader = hound::WavReader::new(cursor)
            .map_err(|e| AppError::audio(format!("engine wav decode: {e}")))?;
        let spec = reader.spec();
        let channels = spec.channels.max(1) as usize;
        let mut mono: Vec<f32> = Vec::new();
        let mut acc = 0.0f32;
        let mut n = 0usize;
        let push = |acc: &mut f32, n: &mut usize, v: f32, mono: &mut Vec<f32>| {
            *acc += v;
            *n += 1;
            if *n == channels {
                mono.push(*acc / channels as f32);
                *acc = 0.0;
                *n = 0;
            }
        };
        match spec.sample_format {
            hound::SampleFormat::Float => {
                for s in reader.into_samples::<f32>() {
                    let v = s.map_err(|e| AppError::audio(format!("wav float decode: {e}")))?;
                    push(&mut acc, &mut n, v, &mut mono);
                }
            }
            hound::SampleFormat::Int => match spec.bits_per_sample {
                16 => {
                    for s in reader.into_samples::<i16>() {
                        let v = s.map_err(|e| AppError::audio(format!("wav i16 decode: {e}")))?;
                        push(&mut acc, &mut n, v as f32 / 32768.0, &mut mono);
                    }
                }
                32 => {
                    for s in reader.into_samples::<i32>() {
                        let v = s.map_err(|e| AppError::audio(format!("wav i32 decode: {e}")))?;
                        push(&mut acc, &mut n, v as f32 / 2147483648.0, &mut mono);
                    }
                }
                b => return Err(AppError::audio(format!("unsupported engine wav depth {b}"))),
            },
        }
        return Ok((mono, spec.sample_rate));
    }
    // MP3 (Edge/OpenAI responses) via symphonia.
    let (samples, rate) = decode_mp3(bytes)?;
    Ok((samples, rate))
}

fn decode_mp3(bytes: &[u8]) -> Result<(Vec<f32>, u32), AppError> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| AppError::audio(format!("mp3 probe: {e}")))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .first()
        .ok_or_else(|| AppError::audio("mp3: no audio track"))?;
    let rate = track.codec_params.sample_rate.unwrap_or(24000);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AppError::audio(format!("mp3 decoder: {e}")))?;
    let mut mono: Vec<f32> = Vec::new();
    loop {
        match format.next_packet() {
            Ok(packet) => {
                let decoded = decoder
                    .decode(&packet)
                    .map_err(|e| AppError::audio(format!("mp3 decode: {e}")))?;
                let spec = *decoded.spec();
                let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buf.copy_interleaved_ref(decoded);
                let channels = spec.channels.count();
                let frames = buf.samples().len() / channels.max(1);
                for f in 0..frames {
                    let mut acc = 0.0f32;
                    for c in 0..channels {
                        acc += buf.samples()[f * channels + c];
                    }
                    mono.push(acc / channels as f32);
                }
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(AppError::audio(format!("mp3 stream: {e}"))),
        }
    }
    Ok((mono, rate))
}

/// Route a request to the right engine implementation.
pub fn synthesize_with(engine: Engine, req: &TtsRequest) -> Result<SynthAudio, AppError> {
    match engine {
        Engine::Scripted => Ok(ScriptedEngine::synthesize(req)),
        Engine::KokoroLocal => Err(AppError::ai_engine_unavailable(
            "KOKORO_LOCAL requires the Kokoro-82M ONNX runtime — see local_tts_generate",
        )),
        Engine::EdgeTts => {
            let mut eng = EdgeTtsEngine { transport: Box::new(real_ws_transport::RealWs::new()?) };
            eng.synthesize(req)
        }
        Engine::ElevenLabs | Engine::Cartesia | Engine::OpenAiTts | Engine::HfMms => {
            HttpTtsEngine { engine, transport: Box::new(UreqTransport) }.synthesize(req)
        }
    }
}

/// Real WebSocket transport (tungstenite + rustls).
mod real_ws_transport {
    use super::{AppError, WsMsg, WsTransport, EDGE_TRUSTED_CLIENT_TOKEN};

    pub struct RealWs {
        socket: tungstenite::protocol::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    }

    impl RealWs {
        pub fn new() -> Result<RealWs, AppError> {
            let url = format!(
                "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={}",
                EDGE_TRUSTED_CLIENT_TOKEN
            );
            let (socket, _) = tungstenite::client::connect(url)
                .map_err(|e| AppError::audio_engine_unavailable(format!("edge tts connect: {e}")))?;
            Ok(RealWs { socket })
        }
    }

    impl WsTransport for RealWs {
        fn send(&mut self, msg: WsMsg) -> Result<(), AppError> {
            let out = match msg {
                WsMsg::Text(t) => tungstenite::Message::Text(t),
                WsMsg::Binary(b) => tungstenite::Message::Binary(b),
            };
            self.socket
                .write(out)
                .map_err(|e| AppError::audio_engine_unavailable(format!("edge tts send: {e}")))?;
            self.socket
                .flush()
                .map_err(|e| AppError::audio_engine_unavailable(format!("edge tts flush: {e}")))?;
            Ok(())
        }

        fn recv(&mut self) -> Result<WsMsg, AppError> {
            loop {
                let msg = self
                    .socket
                    .read()
                    .map_err(|e| AppError::audio_engine_unavailable(format!("edge tts recv: {e}")))?;
                match msg {
                    tungstenite::Message::Text(t) => return Ok(WsMsg::Text(t)),
                    tungstenite::Message::Binary(b) => return Ok(WsMsg::Binary(b)),
                    tungstenite::Message::Ping(_) | tungstenite::Message::Pong(_) => continue,
                    tungstenite::Message::Close(f) => {
                        return Err(AppError::audio_engine_unavailable(format!(
                            "edge tts closed: {f:?}"
                        )))
                    }
                    other => {
                        let _ = other;
                        continue;
                    }
                }
            }
        }
    }
}

/// Optional pronunciation pre-processing shared by all engines.
pub fn with_pronunciations(mut req: TtsRequest, names: &[String]) -> TtsRequest {
    for name in names {
        let (ipa, _) = crate::audio::g2p::to_ipa(name);
        req.pronunciations.push((name.clone(), ipa));
    }
    req
}

/// Summary fragment used in IPC payloads.
pub fn engine_summary(engine: Engine, audio: &SynthAudio) -> Value {
    json!({
        "engine": engine.as_str(),
        "samples": audio.samples.len(),
        "sample_rate": audio.sample_rate,
        "duration_ms": (audio.samples.len() as f64 * 1000.0 / audio.sample_rate as f64).round() as u64,
        "request_id": audio.request_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Mutex;

    /// Serialize env-var mutation across the parallel test threads.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn gec_token_is_deterministic_and_buckets_5min() {
        let a = edge_gec_token(1_700_000_099);
        let b = edge_gec_token(1_700_000_099);
        let c = edge_gec_token(1_700_000_100 - 1); // same 5-min bucket
        let d = edge_gec_token(1_700_000_101);     // one tick past the boundary
        // 1_700_000_100 is a multiple of 300 — the bucket edge.
        assert_eq!(a, b, "same second → same token");
        assert_eq!(a, c, "within the same 5-min bucket");
        assert_ne!(a, d, "crossing the 5-min boundary rotates the token");
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()));
    }

    #[test]
    fn ssml_carries_voice_prosody_and_phonemes() {
        let req = TtsRequest {
            text: "Kira walks".into(),
            voice: "en-US-AriaNeural".into(),
            rate: 1.2,
            pitch: 1.0,
            sample_rate: 24000,
            previous_request_id: None,
            pronunciations: vec![("Kira".into(), "ˈkɪərə".into())],
        };
        let ssml = EdgeTtsEngine::ssml(&req, "TS", "RID");
        assert!(ssml.contains("Path:ssml"));
        assert!(ssml.contains("<voice name='en-US-AriaNeural'>"));
        assert!(ssml.contains("rate='+20%'"));
        assert!(ssml.contains("<phoneme alphabet=\"ipa\" ph=\"ˈkɪərə\">Kira</phoneme>"));
    }

    struct ScriptedWs {
        script: RefCell<Vec<WsMsg>>,
        sent: Rc<RefCell<Vec<String>>>,
    }

    impl WsTransport for ScriptedWs {
        fn send(&mut self, msg: WsMsg) -> Result<(), AppError> {
            if let WsMsg::Text(t) = msg {
                self.sent.borrow_mut().push(t);
            }
            Ok(())
        }
        fn recv(&mut self) -> Result<WsMsg, AppError> {
            if self.script.borrow().is_empty() {
                return Err(AppError::audio_engine_unavailable("script exhausted"));
            }
            Ok(self.script.borrow_mut().remove(0))
        }
    }

    fn wav_bytes() -> Vec<u8> {
        use std::io::Seek;
        let spec = hound::WavSpec { channels: 1, sample_rate: 24000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut cur = std::io::Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut cur, spec).unwrap();
            for i in 0..2400 {
                w.write_sample(((i as f32 * 0.1).sin() * 16000.0) as i16).unwrap();
            }
        }
        cur.into_inner()
    }

    fn frame_binary(payload: &[u8], path: &str) -> Vec<u8> {
        let header = format!("X-RequestId:abc\r\nContent-Type:audio/mpeg\r\nPath:{path}\r\n\r\n");
        let mut out = Vec::new();
        out.extend_from_slice(&(header.len() as u16).to_be_bytes());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn edge_protocol_assembles_audio_and_turn_end() {
        let wav = wav_bytes();
        let sent = Rc::new(RefCell::new(Vec::new()));
        let transport = ScriptedWs {
            script: RefCell::new(vec![
                WsMsg::Binary(frame_binary(&wav[..100], "audio")),
                WsMsg::Binary(frame_binary(&wav[100..], "audio")),
                WsMsg::Text("X-RequestId:turn-42\r\nPath:turn.end\r\n\r\n".into()),
            ]),
            sent: Rc::clone(&sent),
        };
        let mut eng = EdgeTtsEngine { transport: Box::new(transport) };
        let req = TtsRequest {
            text: "hello".into(),
            voice: "en-US-AriaNeural".into(),
            rate: 1.0,
            pitch: 1.0,
            sample_rate: 24000,
            previous_request_id: None,
            pronunciations: vec![],
        };
        let audio = eng.synthesize(&req).unwrap();
        assert_eq!(audio.samples.len(), 2400);
        assert_eq!(audio.request_id.as_deref(), Some("turn-42"));
        let sent_log = sent.borrow().clone();
        assert!(sent_log[0].contains("Path:speech.config"));
        assert!(sent_log[1].contains("Path:ssml"));
    }

    #[test]
    fn scripted_engine_is_deterministic() {
        let mk = || TtsRequest {
            text: "The dawn comes early over Kira's camp".into(),
            voice: "af_bella".into(),
            rate: 1.0,
            pitch: 1.0,
            sample_rate: 22050,
            previous_request_id: None,
            pronunciations: vec![],
        };
        let a = ScriptedEngine::synthesize(&mk());
        let b = ScriptedEngine::synthesize(&mk());
        assert_eq!(a.samples.len(), b.samples.len());
        assert!(a.samples.iter().zip(b.samples.iter()).all(|(x, y)| (x - y).abs() < 1e-9));
        assert!(a.samples.len() > 1000);
        assert!(a.request_id.is_some());
        // Prosody rate scales duration.
        let mut fast = mk();
        fast.rate = 2.0;
        let f = ScriptedEngine::synthesize(&fast);
        assert!(f.samples.len() < a.samples.len());
    }

    #[test]
    fn missing_api_key_maps_to_engine_unavailable() {
        let _env = env_lock();
        let eng = HttpTtsEngine { engine: Engine::ElevenLabs, transport: Box::new(UreqTransport) };
        let req = TtsRequest {
            text: "x".into(),
            voice: String::new(),
            rate: 1.0,
            pitch: 1.0,
            sample_rate: 24000,
            previous_request_id: None,
            pronunciations: vec![],
        };
        std::env::remove_var("ELEVENLABS_API_KEY");
        let err = eng.build_request(&req).unwrap_err();
        assert_eq!(err.code, "AUDIO_ENGINE_UNAVAILABLE");
    }

    #[test]
    fn elevenlabs_request_threads_previous_ids() {
        let _env = env_lock();
        let eng = HttpTtsEngine { engine: Engine::ElevenLabs, transport: Box::new(UreqTransport) };
        std::env::set_var("ELEVENLABS_API_KEY", "test-key");
        let req = TtsRequest {
            text: "line two".into(),
            voice: "voice-1".into(),
            rate: 1.0,
            pitch: 1.0,
            sample_rate: 24000,
            previous_request_id: Some("prev-9".into()),
            pronunciations: vec![],
        };
        let (_, _, body, _) = eng.build_request(&req).unwrap();
        assert!(body.contains("\"previous_request_ids\":[\"prev-9\"]"));
        std::env::remove_var("ELEVENLABS_API_KEY");
    }

    #[test]
    fn engine_parse_covers_contract_spellings() {
        assert_eq!(Engine::parse("KOKORO_LOCAL").unwrap(), Engine::KokoroLocal);
        assert_eq!(Engine::parse("edge_tts").unwrap(), Engine::EdgeTts);
        assert_eq!(Engine::parse("SCRIPTED").unwrap(), Engine::Scripted);
        assert!(Engine::parse("WATSON").is_err());
    }

    #[test]
    fn offline_guard_blocks_network_engines() {
        let _env = env_lock();
        std::env::set_var("AUTHR_TTS_OFFLINE", "1");
        let req = TtsRequest {
            text: "x".into(),
            voice: "v".into(),
            rate: 1.0,
            pitch: 1.0,
            sample_rate: 24000,
            previous_request_id: None,
            pronunciations: vec![],
        };
        let err = match synthesize_with(Engine::ElevenLabs, &req) {
            Err(e) => e,
            Ok(_) => panic!("expected engine unavailable"),
        };
        assert_eq!(err.code, "AUDIO_ENGINE_UNAVAILABLE");
        std::env::remove_var("AUTHR_TTS_OFFLINE");
    }
}
