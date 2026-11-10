// AUTHR_OS — Microsoft Edge ReadAloud WebSocket client (Sprint 9, build plan
// §7.1/`EdgeTtsManager.kt` mapping). Implements the Sec-MS-GEC GEC SHA-256
// token handshake, SSML prosody framing, and the binary MP3 chunk parser.
// Live use requires AUTHR_TTS_LIVE=1 (see engines::resolve); the token
// generator, SSML builder and frame parser are unit-tested offline.

use super::engines::{OUTPUT_FS, SynthOutput, SynthRequest, TtsEngine};
use crate::audio::dsp::fnv64;
use crate::audio::wav::resample;
use crate::error::AppError;
use sha2::{Digest, Sha256};

pub const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
pub const SEC_MS_GEC_VERSION: &str = "1-130.0.2849.68";
pub const SPEECH_HOST: &str = "speech.platform.bing.com";
pub const OUTPUT_FORMAT: &str = "audio-24khz-48kbitrate-mono-mp3";
pub const CONNECT_TIMEOUT_SECS: u64 = 8;
pub const RECV_TIMEOUT_SECS: u64 = 10;

/// GEC token: seconds since 1601-01-01 (Windows epoch) rounded down to the
/// 5-minute boundary, concatenated with the trusted client token, SHA-256
/// uppercased. Verified vectors in the unit tests.
pub fn sec_ms_gec(unix_ts: u64) -> String {
    const WIN_EPOCH: u64 = 11_644_473_600;
    let mut secs = unix_ts + WIN_EPOCH;
    secs -= secs % 300;
    let to_hash = format!("{secs}{TRUSTED_CLIENT_TOKEN}");
    let digest = Sha256::digest(to_hash.as_bytes());
    hex::encode(digest).to_uppercase()
}

pub fn sec_ms_gec_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    sec_ms_gec(now)
}

pub fn wss_url() -> String {
    format!(
        "wss://{SPEECH_HOST}/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={TRUSTED_CLIENT_TOKEN}&Sec-MS-GEC={}&Sec-MS-GEC-Version={SEC_MS_GEC_VERSION}",
        sec_ms_gec_now()
    )
}

pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// SSML with Edge prosody: rate as % change, pitch as Hz change.
pub fn ssml(voice: &str, rate: f64, pitch: f64, text: &str) -> String {
    let rate_pct = ((rate - 1.0) * 100.0).round() as i64;
    let pitch_hz = ((pitch - 1.0) * 40.0).round() as i64;
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{voice}'><prosody pitch='{pitch_hz:+}Hz' rate='{rate_pct:+}%'>{}</prosody></voice></speak>",
        escape_xml(text)
    )
}

fn request_message(ssml_text: &str) -> String {
    let ts = chrono::Utc::now().format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)");
    let request_id = format!("{:032x}", fnv64(ssml_text.as_bytes()));
    format!(
        "X-RequestId:{request_id}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{ts}\r\nPath:ssml\r\n\r\n{ssml_text}"
    )
}

fn config_message() -> String {
    let ts = chrono::Utc::now().format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)");
    format!(
        "X-Timestamp:{ts}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
         {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"{OUTPUT_FORMAT}\"}}}}}}}}"
    )
}

/// Parse a stream of WebSocket frames: collect MP3 payloads from binary
/// frames whose header carries `Path:audio`; text frames (turn.end etc.)
/// are skipped. Unit-tested offline.
pub fn collect_audio_bytes(frames: &[(bool, Vec<u8>)]) -> Vec<u8> {
    let mut audio = Vec::new();
    for (binary, data) in frames {
        if !*binary {
            continue;
        }
        if let Some(pos) = find_audio_header(data) {
            audio.extend_from_slice(&data[pos..]);
        }
    }
    audio
}

fn find_audio_header(data: &[u8]) -> Option<usize> {
    const NEEDLE: &[u8] = b"Path:audio\r\n";
    data.windows(NEEDLE.len())
        .position(|w| w == NEEDLE)
        .map(|p| p + NEEDLE.len())
}

/// Decode MP3 (or any symphonia-supported container) to mono f32 at its
/// native rate; resampled to the unified 24 kHz chunk rate by the caller.
pub fn decode_mp3_to_f32(bytes: &[u8]) -> Result<(Vec<f32>, u32), AppError> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| AppError::storage(format!("edge-tts stream probe failed: {e}")))?;
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AppError::storage("edge-tts stream has no audio track"))?
        .clone();
    let fs = track.codec_params.sample_rate.unwrap_or(OUTPUT_FS);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AppError::storage(format!("edge-tts decoder init failed: {e}")))?;

    let mut mono: Vec<f32> = Vec::new();
    loop {
        let packet = match probed.format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(AppError::storage(format!("edge-tts decode: {e}"))),
        };
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let frames = decoded.frames();
                let capacity = decoded.capacity() as u64;
                let mut buf = SampleBuffer::<f32>::new(capacity, spec);
                buf.copy_interleaved_ref(decoded);
                let channels = spec.channels.count();
                let s = buf.samples();
                if channels <= 1 {
                    mono.extend_from_slice(&s[..frames.min(s.len())]);
                } else {
                    for f in 0..frames {
                        let acc: f32 = s[f * channels..(f + 1) * channels]
                            .iter()
                            .map(|v| v.clamp(-1.0, 1.0))
                            .sum();
                        mono.push(acc / channels as f32);
                    }
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue, // skip bad packets
            Err(e) => return Err(AppError::storage(format!("edge-tts decode packet: {e}"))),
        }
    }
    Ok((mono, fs))
}

pub struct EdgeTts {
    pub voice_id: String,
}

impl TtsEngine for EdgeTts {
    fn synthesize(&self, req: &SynthRequest) -> Result<SynthOutput, AppError> {
        let ssml_text = ssml(&self.voice_id, req.rate, req.pitch, &req.text);
        let frames = ws_exchange(&config_message(), &request_message(&ssml_text))?;
        let mp3 = collect_audio_bytes(&frames);
        if mp3.is_empty() {
            return Err(AppError::storage("edge-tts returned no audio payload"));
        }
        let (mono, native_fs) = decode_mp3_to_f32(&mp3)?;
        let samples = resample(&mono, native_fs, OUTPUT_FS);
        Ok(SynthOutput {
            samples,
            fs: OUTPUT_FS,
            request_id: format!("edge_{:016x}", fnv64(req.text.as_bytes())),
            engine: "EDGE_TTS".into(),
            degraded: false,
            note: format!("live edge-tts `{}`", self.voice_id),
        })
    }
}

/// Blocking wrapper around the async WebSocket exchange (one tokio runtime
/// per call — the DAG scheduler drives engines from worker threads).
fn ws_exchange(config: &str, request: &str) -> Result<Vec<(bool, Vec<u8>)>, AppError> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AppError::storage(format!("edge-tts runtime: {e}")))?;

    rt.block_on(async move {
        let url = wss_url();
        let mut http_request = url
            .into_client_request()
            .map_err(|e| AppError::storage(format!("edge-tts request: {e}")))?;
        http_request.headers_mut().insert(
            "Origin",
            "azure.microsoft.com".parse().expect("static header"),
        );
        http_request.headers_mut().insert(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.2849.68".parse().expect("static header"),
        );

        let (ws, _resp) = tokio::time::timeout(
            std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
            tokio_tungstenite::connect_async(http_request),
        )
        .await
        .map_err(|_| AppError::storage("edge-tts connect timeout"))?
        .map_err(|e| AppError::storage(format!("edge-tts connect failed: {e}")))?;

        let (mut tx, mut rx) = ws.split();
        tx.send(Message::Text(config.to_string().into()))
            .await
            .map_err(|e| AppError::storage(format!("edge-tts send config: {e}")))?;
        tx.send(Message::Text(request.to_string().into()))
            .await
            .map_err(|e| AppError::storage(format!("edge-tts send ssml: {e}")))?;

        let mut frames: Vec<(bool, Vec<u8>)> = Vec::new();
        let deadline = std::time::Duration::from_secs(RECV_TIMEOUT_SECS);
        let _ = tokio::time::timeout(deadline, async {
            while let Some(msg) = rx.next().await {
                match msg {
                    Ok(Message::Text(t)) => {
                        if t.contains("Path:turn.end") {
                            break;
                        }
                        frames.push((false, t.into_bytes()));
                    }
                    Ok(Message::Binary(b)) => frames.push((true, b)),
                    Ok(_) => {}
                    Err(e) => {
                        let _ = tx.send(Message::Close(None)).await;
                        return Err(AppError::storage(format!("edge-tts stream: {e}")));
                    }
                }
            }
            let _ = tx.send(Message::Close(None)).await;
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|_| AppError::storage("edge-tts receive timeout"))??;

        Ok(frames)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gec_token_vectors() {
        // Pinned by scripts/phase4_verify_constants.py
        assert_eq!(
            sec_ms_gec(1_700_000_000),
            "C5576CCFA0891A429DD5488FAEC35CBFF011A573DF72295001372D8A9A2C234F"
        );
        assert_eq!(
            sec_ms_gec(1_700_000_001),
            "C5576CCFA0891A429DD5488FAEC35CBFF011A573DF72295001372D8A9A2C234F"
        );
        assert_eq!(
            sec_ms_gec(1_700_000_300),
            "BE51046D4DA5E22B7B8CECC9D55B343FBE3F9D7E933D4870E75479139269B324"
        );
        assert_eq!(
            sec_ms_gec(0),
            "8293933CFF86302B3691984C1125815F7AAC6991C796E51496B3548B82BE6F07"
        );
    }

    #[test]
    fn gec_is_64_uppercase_hex() {
        let t = sec_ms_gec_now();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn wss_url_contains_token_and_version() {
        let u = wss_url();
        assert!(u.starts_with("wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?"));
        assert!(u.contains(&format!("TrustedClientToken={TRUSTED_CLIENT_TOKEN}")));
        assert!(u.contains(&format!("Sec-MS-GEC-Version={SEC_MS_GEC_VERSION}")));
        assert!(u.contains("Sec-MS-GEC="));
    }

    #[test]
    fn ssml_escapes_and_formats_prosody() {
        let s = ssml("en-US-JennyNeural", 1.0, 1.0, "He said \"run\" & <left>");
        assert!(s.contains("<voice name='en-US-JennyNeural'>"));
        assert!(s.contains("rate='+0%'"));
        assert!(s.contains("pitch='+0Hz'"));
        assert!(s.contains("He said &quot;run&quot; &amp; &lt;left&gt;"));
        let fast = ssml("v", 1.5, 0.5, "x");
        assert!(fast.contains("rate='+50%'"));
        assert!(fast.contains("pitch='-20Hz'"));
    }

    #[test]
    fn binary_frame_parser_collects_audio_only() {
        let frames = vec![
            (true, b"X-RequestId:aa\r\nPath:audio\r\nMP3CHUNK1".to_vec()),
            (false, b"X-RequestId:aa\r\nPath:turn.start\r\n{}".to_vec()),
            (true, b"X-RequestId:aa\r\nPath:audio\r\nMP3CHUNK2".to_vec()),
        ];
        assert_eq!(collect_audio_bytes(&frames), b"MP3CHUNK1MP3CHUNK2");
        assert!(collect_audio_bytes(&[(true, b"Path:turn.end\r\n".to_vec())]).is_empty());
    }

    #[test]
    fn request_and_config_messages_carry_paths() {
        let cfg = config_message();
        assert!(cfg.contains("Path:speech.config"));
        assert!(cfg.contains(OUTPUT_FORMAT));
        let req = request_message("<speak>x</speak>");
        assert!(req.contains("Path:ssml"));
        assert!(req.contains("Content-Type:application/ssml+xml"));
    }
}
