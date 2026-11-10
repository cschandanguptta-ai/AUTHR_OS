// AUTHR_OS — Local neural TTS boundary + offline synthesis (Phase 5 / Sprint 13).
//
// Engine matrix (selection via `AUTHR_TTS_ENGINE`, default `auto`):
//   offline-formant — deterministic source-filter synthesizer over the G2P
//                     phoneme stream (formant pair per phoneme class, pitch
//                     contour from the voice profile, declination + vibrato,
//                     24 kHz 16-bit mono RIFF WAV). Always available offline.
//   kokoro          — Kokoro-82M v1.0 ONNX via ort 2.0 fed by the same G2P
//                     tokenizer. Adapter contract: docs/PHASE5_NOTES.md.
//
// WAV chunks are content-addressed by `ai::audio_cache` (SHA-256), the same
// layout the Phase 4 lookahead scheduler consumes.

use crate::ai::audio_cache;
use crate::ai::g2p;
use crate::error::AppError;
use crate::ipc::{args, Ctx};
use rusqlite::Connection;
use serde_json::{json, Value};

pub const SAMPLE_RATE: u32 = 24_000;

/// Voice profile resolved from `character_profiles` (contract §4.4).
#[derive(Debug, Clone)]
pub struct VoiceProfile {
    pub character_key: String,
    pub voice_id: String,
    pub engine: String,
    pub rate: f64,
    pub pitch: f64,
    /// (term, IPA) proper-noun overrides: the character itself + lore keys
    /// that literally appear in the synthesized text.
    pub ipa_overrides: Vec<(String, String)>,
}

pub struct SynthResult {
    pub wav: Vec<u8>,
    pub duration_ms: i64,
    pub engine: &'static str,
    pub voice_id: String,
    pub phonemes: Vec<String>,
}

pub trait LocalTts: Send + Sync {
    fn name(&self) -> &'static str;
    fn offline(&self) -> bool;
    fn synthesize(
        &self,
        text: &str,
        voice: &VoiceProfile,
    ) -> Result<SynthResult, AppError>;
}

// ── offline formant engine ───────────────────────────────────────────────────

#[derive(Default)]
pub struct OfflineFormantTts;

/// Formant center pairs (F1, F2) in Hz per phoneme base.
const FORMANTS: &[(&str, f64, f64)] = &[
    // Vowels
    ("AA", 730.0, 1090.0), ("AE", 660.0, 1720.0), ("AH", 640.0, 1190.0),
    ("AO", 570.0, 840.0), ("EH", 530.0, 1840.0), ("ER", 490.0, 1350.0),
    ("IH", 390.0, 1990.0), ("IY", 270.0, 2290.0), ("UH", 640.0, 1190.0),
    ("UW", 300.0, 870.0), ("AY", 600.0, 1900.0), ("AW", 620.0, 1100.0),
    ("EY", 500.0, 2000.0), ("OW", 480.0, 900.0), ("OY", 520.0, 1600.0),
    // Consonants (voiced/approximant formants; fricatives use noise centers)
    ("M", 250.0, 1100.0), ("N", 250.0, 1700.0), ("NG", 250.0, 2300.0),
    ("L", 360.0, 1300.0), ("R", 350.0, 1100.0), ("W", 300.0, 610.0),
    ("Y", 270.0, 2290.0), ("B", 400.0, 900.0), ("D", 400.0, 1700.0),
    ("G", 400.0, 2000.0), ("V", 400.0, 1200.0), ("DH", 400.0, 1400.0),
    ("Z", 400.0, 1400.0), ("ZH", 400.0, 2000.0),
];

/// Noise-dominant fricatives/affricates: (center Hz, relative bandwidth).
const NOISE_PHONES: &[(&str, f64)] = &[
    ("F", 1200.0), ("TH", 6000.0), ("S", 5500.0), ("SH", 3000.0),
    ("HH", 1500.0), ("CH", 3200.0), ("JH", 2600.0), ("P", 900.0),
    ("T", 4000.0), ("K", 2200.0),
];

const MS_PER_SAMPLE: f64 = 1000.0 / SAMPLE_RATE as f64;

impl LocalTts for OfflineFormantTts {
    fn name(&self) -> &'static str {
        "offline-formant"
    }

    fn offline(&self) -> bool {
        true
    }

    fn synthesize(&self, text: &str, voice: &VoiceProfile) -> Result<SynthResult, AppError> {
        if text.trim().is_empty() {
            return Err(AppError::storage("cannot synthesize empty text"));
        }
        let phonemes = g2p::phonemize(text, &voice.ipa_overrides);
        if phonemes.iter().all(|p| p == g2p::PAUSE) {
            return Err(AppError::storage("text produced no speakable phonemes"));
        }

        // Voice pitch from voice_id hash (stable per voice), scaled by profile.
        let h = crate::ai::embedder::fnv1a64(voice.voice_id.as_bytes());
        let base_f0 = 100.0 + (h % 10_000) as f64 / 10_000.0 * 90.0; // 100–190 Hz
        let f0 = base_f0 * voice.pitch.clamp(0.25, 4.0);
        let rate = voice.rate.clamp(0.25, 4.0);

        let total_vowels = g2p::vowel_count(&phonemes).max(1) as f64;
        let mut pcm: Vec<i16> = Vec::with_capacity(48_000);
        let mut rng = (h ^ 0x9e3779b97f4a7c15) | 1;
        let mut vowel_i = 0f64;

        for phone in &phonemes {
            let base = phone.trim_end_matches(|c: char| c.is_ascii_digit());
            let stressed = phone.ends_with('1');

            if base == g2p::PAUSE {
                append_silence(&mut pcm, 40.0 / rate);
                continue;
            }

            let is_vowel = matches!(
                base,
                "IY" | "IH" | "EH" | "AE" | "AA" | "AO" | "UW" | "UH" | "AH" | "ER"
            );
            let is_diphthong = matches!(base, "AY" | "AW" | "EY" | "OW" | "OY");
            let dur_ms = if is_vowel {
                110.0 / rate
            } else if is_diphthong {
                95.0 / rate
            } else if FORMANTS.iter().any(|(p, _, _)| *p == base) {
                80.0 / rate
            } else if NOISE_PHONES.iter().any(|(p, _)| *p == base) {
                65.0 / rate
            } else {
                70.0 / rate
            };

            let samples = ((dur_ms) / MS_PER_SAMPLE).max(8.0) as usize;
            let declination = 1.0 - 0.15 * (vowel_i / total_vowels);
            let local_f0 = f0 * declination * if stressed { 1.08 } else { 1.0 };

            if let Some((_, center)) = NOISE_PHONES.iter().find(|(p, _)| *p == base) {
                append_noise(&mut pcm, *center, samples, &mut rng, 0.32);
            } else {
                let (f1, f2) = FORMANTS
                    .iter()
                    .find(|(p, _, _)| *p == base)
                    .map(|(_, a, b)| (*a, *b))
                    .unwrap_or((600.0, 1500.0));
                let voiced = !matches!(base, "P" | "T" | "K" | "F" | "TH" | "HH" | "S" | "SH" | "CH" | "JH");
                let amp = if voiced { 0.62 } else { 0.10 };
                append_formant(&mut pcm, local_f0, f1, f2, samples, amp, &mut rng);
            }

            if FORMANTS.iter().any(|(p, _, _)| *p == base) {
                vowel_i += 1.0;
            }
            append_silence(&mut pcm, 12.0 / rate);
        }

        // Soft-clip + fade-in/out to avoid clicks at chunk boundaries.
        fade_edges(&mut pcm, SAMPLE_RATE, 15);
        let duration_ms = (pcm.len() as f64 * MS_PER_SAMPLE) as i64;
        let wav = wav_bytes(&pcm, SAMPLE_RATE);
        Ok(SynthResult {
            wav,
            duration_ms,
            engine: self.name(),
            voice_id: voice.voice_id.clone(),
            phonemes,
        })
    }
}

fn append_silence(pcm: &mut Vec<i16>, dur_ms: f64) {
    let n = (dur_ms / MS_PER_SAMPLE) as usize;
    pcm.resize(pcm.len() + n, 0);
}

/// Sum of two resonated harmonics tracking the formant pair, glottal-ish
/// excitation at f0, deterministic micro-jitter (no float RNG state).
fn append_formant(pcm: &mut Vec<i16>, f0: f64, f1: f64, f2: f64, n: usize, amp: f64, rng: &mut u64) {
    let two_pi = std::f64::consts::TAU;
    let jitter = (next_noise(rng) - 0.5) * 0.02;
    for i in 0..n {
        let t = i as f64 / SAMPLE_RATE as f64;
        // Attack/release envelope (15 ms / 25 ms).
        let env = envelope(i, n, 15.0, 25.0);
        let excite = (two_pi * f0 * (1.0 + jitter) * t).sin();
        let f1s = (two_pi * f1 * t).sin() * 0.55;
        let f2s = (two_pi * f2 * t).sin() * 0.28;
        let vib = 1.0 + 0.02 * (two_pi * 5.0 * t).sin();
        let v = amp * env * (excite * 0.8 + (f1s + f2s) * vib);
        push_sample(pcm, v);
    }
}

/// Band-limited deterministic noise burst (fricatives/plosive releases).
fn append_noise(pcm: &mut Vec<i16>, center: f64, n: usize, rng: &mut u64, amp: f64) {
    // One-pole low-pass toward the noise center via ring-mod approximation.
    let mut lp = 0.0f64;
    let cutoff = (center / (SAMPLE_RATE as f64 / 2.0)).clamp(0.02, 0.9);
    for i in 0..n {
        let env = envelope(i, n, 8.0, 30.0);
        let white = next_noise(rng) * 2.0 - 1.0;
        lp += cutoff * (white - lp);
        push_sample(pcm, amp * env * lp);
    }
}

fn envelope(i: usize, n: usize, attack_ms: f64, release_ms: f64) -> f64 {
    let a = ((attack_ms / MS_PER_SAMPLE) as usize).max(1);
    let r = ((release_ms / MS_PER_SAMPLE) as usize).max(1);
    let attack = (i as f64 / a as f64).min(1.0);
    let release = ((n - i) as f64 / r as f64).min(1.0);
    attack.min(release).max(0.0)
}

fn next_noise(rng: &mut u64) -> f64 {
    // xorshift64 — deterministic across runs.
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    (*rng >> 11) as f64 / (1u64 << 53) as f64
}

fn push_sample(pcm: &mut Vec<i16>, v: f64) {
    let clamped = v.clamp(-1.0, 1.0);
    pcm.push((clamped * i16::MAX as f64) as i16);
}

fn fade_edges(pcm: &mut [i16], _rate: u32, ms: u32) {
    let n = ((SAMPLE_RATE as u32 * ms) / 1000) as usize;
    let len = pcm.len();
    if len < 2 * n {
        return;
    }
    for i in 0..n {
        let f = i as f64 / n as f64;
        pcm[i] = (pcm[i] as f64 * f) as i16;
        pcm[len - 1 - i] = (pcm[len - 1 - i] as f64 * f) as i16;
    }
}

/// Minimal 44-byte RIFF/WAVE writer — 16-bit signed PCM mono.
pub fn wav_bytes(pcm: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    b.extend_from_slice(&2u16.to_le_bytes()); // block align
    b.extend_from_slice(&16u16.to_le_bytes()); // bits
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

// ── engine selection ─────────────────────────────────────────────────────────

pub fn default_tts() -> Result<std::sync::Arc<dyn LocalTts>, AppError> {
    let choice = std::env::var("AUTHR_TTS_ENGINE").unwrap_or_else(|_| "auto".into());
    match choice.as_str() {
        "auto" | "offline-formant" => Ok(std::sync::Arc::new(OfflineFormantTts)),
        "kokoro" => Err(AppError::storage(
            "AUTHR_TTS_ENGINE=kokoro requires an ONNX adapter build — see docs/PHASE5_NOTES.md",
        )),
        other => Err(AppError::storage(format!(
            "unknown tts engine `{other}` — available: offline-formant"
        ))),
    }
}

// ── command bodies ───────────────────────────────────────────────────────────

/// Resolve a voice profile for a segment's speaker, collecting proper-noun
/// IPA overrides from the character row and any lore keys mentioned in text.
fn resolve_voice(conn: &Connection, speaker_id: &str, text: &str) -> Result<VoiceProfile, AppError> {
    let row = conn
        .query_row(
            "SELECT character_key, voice_id, voice_engine, base_rate, base_pitch, ipa_phonetic, display_name
             FROM character_profiles WHERE character_key = ?1",
            rusqlite::params![speaker_id],
            |r| {
                Ok(VoiceProfile {
                    character_key: r.get(0)?,
                    voice_id: r.get(1)?,
                    engine: r.get(2)?,
                    rate: r.get::<_, f64>(3)?,
                    pitch: r.get::<_, f64>(4)?,
                    ipa_overrides: Vec::new(),
                })
            },
        )
        .map_err(|_| AppError::not_found(format!("speaker `{speaker_id}` not found")))?;

    let mut overrides: Vec<(String, String)> = Vec::new();
    // Character's own IPA (match display name words in text too).
    let ipa: String = conn
        .query_row(
            "SELECT ipa_phonetic FROM character_profiles WHERE character_key = ?1",
            rusqlite::params![speaker_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if !ipa.trim().is_empty() {
        overrides.push((speaker_id.to_string(), ipa.clone()));
        let display: String = conn
            .query_row(
                "SELECT display_name FROM character_profiles WHERE character_key = ?1",
                rusqlite::params![speaker_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        if !display.is_empty() {
            overrides.push((display, ipa));
        }
    }
    // Lore keys literally present in the text contribute their IPA.
    let mut stmt = conn
        .prepare("SELECT key, ipa_phonetic FROM lorebook WHERE ipa_phonetic != ''")
        .map_err(|e| AppError::storage(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| AppError::storage(e.to_string()))?;
    let lower = text.to_lowercase();
    for r in rows {
        let (key, ipa) = r.map_err(|e| AppError::storage(e.to_string()))?;
        if lower.contains(&key.to_lowercase()) {
            overrides.push((key, ipa));
        }
    }
    Ok(VoiceProfile { ipa_overrides: overrides, ..row })
}

/// `tts_synthesize {segment_id}` — synthesize one segment's text with its
/// speaker's voice profile, store the WAV in the content-addressable cache,
/// and stamp `audio_hash` / `audio_cache_path` / `audio_duration_ms`.
pub fn synthesize_segment(ctx: &mut Ctx, a: &Value) -> Result<Value, AppError> {
    let segment_id = args::req_str(a, "segment_id")?;
    let seg = crate::storage::segments::get(&ctx.conn, &segment_id)?;
    let text = seg
        .get("text_content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::storage("segment row missing text_content"))?
        .to_string();
    let speaker = seg
        .get("speaker_id")
        .and_then(|v| v.as_str())
        .unwrap_or("NARRATOR")
        .to_string();

    let engine = default_tts()?;
    let voice = resolve_voice(&ctx.conn, &speaker, &text)?;

    // Content-addressable key (Sprint 9 cache layout, reused by Phase 4).
    let hash = audio_cache::chunk_key(engine.name(), &voice, &text);
    let existing_path: Option<String> = ctx
        .conn
        .query_row(
            "SELECT audio_cache_path FROM chapter_segments WHERE segment_id = ?1",
            rusqlite::params![segment_id],
            |r| r.get(0),
        )
        .map_err(|e| AppError::storage(e.to_string()))?;

    // Cache hit only when the row already points at this exact chunk AND the
    // file survived on disk (corrupt-cache recovery re-synthesizes).
    if existing_path.as_deref() == Some(audio_cache::relative_path(&hash).as_str())
        && audio_cache::lookup(&ctx.root, &hash)?.is_some()
    {
        let abs = ctx.root.join(audio_cache::relative_path(&hash));
        let bytes = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
        return Ok(json!({
            "segment_id": segment_id,
            "audio_hash": hash,
            "path": audio_cache::relative_path(&hash),
            "bytes": bytes,
            "engine": engine.name(),
            "voice_id": voice.voice_id,
            "cached": true,
        }));
    }

    let synth = engine.synthesize(&text, &voice)?;
    audio_cache::store(&ctx.root, &hash, &synth.wav)?;
    let rel = audio_cache::relative_path(&hash);
    ctx.conn
        .execute(
            "UPDATE chapter_segments
             SET audio_hash = ?2, audio_cache_path = ?3, audio_duration_ms = ?4,
                 last_modified_at = strftime('%s','now')
             WHERE segment_id = ?1",
            rusqlite::params![segment_id, hash, rel, synth.duration_ms],
        )
        .map_err(|e| AppError::storage(e.to_string()))?;
    ctx.after_commit()?;

    Ok(json!({
        "segment_id": segment_id,
        "audio_hash": hash,
        "path": rel,
        "bytes": synth.wav.len(),
        "duration_ms": synth.duration_ms,
        "engine": engine.name(),
        "voice_id": voice.voice_id,
        "phoneme_count": synth.phonemes.len(),
        "cached": false,
        "offline": engine.offline(),
    }))
}

/// `audio_cache_stats {}` — file count + bytes under `audio/segments/`.
pub fn cache_stats(ctx: &Ctx) -> Result<Value, AppError> {
    let (files, bytes) = audio_cache::stats(&ctx.root)?;
    Ok(json!({ "dir": "audio/segments", "files": files, "bytes": bytes }))
}
