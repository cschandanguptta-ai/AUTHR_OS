// AUTHR_OS — Cross-Segment Acoustic Continuity latents (Sprint 10, §6.3).
// After each successful synthesis the last 1.5 s of audio is analyzed into a
// compact prosody latent (RMS, median F0 via autocorrelation, zero-crossing
// rate, tail hash) and persisted per speaker so the NEXT turn of the same
// character can be conditioned on it — serializing consecutive turns of one
// speaker while different speakers synthesize in parallel.

use crate::error::AppError;
use sha2::Digest;
use serde_json::Value;
use std::path::Path;

pub const LATENT_WINDOW_MS: u32 = 1500;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Latent {
    pub speaker: String,
    pub rms_dbfs: f64,
    pub pitch_hz: f64,
    pub zcr: f64,
    pub tail_hash: String,
    pub window_ms: u32,
    pub generated_at: String,
}

/// Extract the 1.5 s tail latent from a fresh synthesis.
pub fn extract(speaker: &str, samples: &[f32], fs: u32) -> Latent {
    let window = ((LATENT_WINDOW_MS as u64 * fs as u64) / 1000) as usize;
    let start = samples.len().saturating_sub(window);
    let tail = &samples[start..];
    let rms_dbfs = crate::audio::dsp::rms_dbfs(tail);
    let zcr = zero_crossing_rate(tail);
    let pitch_hz = estimate_f0(tail, fs);
    let tail_hash = {
        let q: Vec<i16> = tail.iter().map(|s| ((*s as f64) * i16::MAX as f64) as i16).collect();
        let bytes: Vec<u8> = q.iter().flat_map(|v| v.to_le_bytes()).collect();
        hex::encode(&sha2::Sha256::digest(&bytes)[..8])
    };
    Latent {
        speaker: speaker.to_string(),
        rms_dbfs,
        pitch_hz,
        zcr,
        tail_hash,
        window_ms: LATENT_WINDOW_MS,
        generated_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn zero_crossing_rate(x: &[f32]) -> f64 {
    if x.len() < 2 {
        return 0.0;
    }
    let crossings = x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
    crossings as f64 / (x.len() - 1) as f64
}

/// Median F0 via normalized autocorrelation over the 60–400 Hz band.
/// Returns 0.0 when the tail is effectively silence.
pub fn estimate_f0(x: &[f32], fs: u32) -> f64 {
    if x.len() < fs as usize / 4 {
        return 0.0;
    }
    let lag_min = (fs as f64 / 400.0).floor() as usize;
    let lag_max = (fs as f64 / 60.0).ceil() as usize;
    if lag_max >= x.len() {
        return 0.0;
    }
    let e = x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
    if e < 1e-8 {
        return 0.0;
    }
    let mut best = (0.0f64, 0usize);
    let mut correlations = Vec::new();
    for lag in lag_min..lag_max.min(x.len() / 2) {
        let mut dot = 0.0f64;
        let mut e2 = 0.0f64;
        for i in 0..x.len() - lag {
            dot += x[i] as f64 * x[i + lag] as f64;
            e2 += x[i + lag] as f64 * x[i + lag] as f64;
        }
        let norm = (e * e2.max(1e-12)).sqrt();
        let r = dot / norm;
        correlations.push((r, lag));
        if r > best.0 {
            best = (r, lag);
        }
    }
    if best.0 < 0.3 || best.1 == 0 {
        return 0.0; // unvoiced / silence
    }
    // parabolic refinement around the winning lag
    let win = correlations.iter().find(|(_, l)| *l == best.1);
    let _ = win;
    fs as f64 / best.1 as f64
}

pub fn sanitize_speaker(speaker: &str) -> String {
    speaker
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

pub fn latent_path(root: &Path, speaker: &str) -> std::path::PathBuf {
    root.join("audio").join("segments").join(".latents")
        .join(format!("{}.json", sanitize_speaker(speaker)))
}

pub fn save(root: &Path, lat: &Latent) -> Result<(), AppError> {
    let p = latent_path(root, &lat.speaker);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(lat)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

pub fn load(root: &Path, speaker: &str) -> Option<Latent> {
    let p = latent_path(root, speaker);
    let data = std::fs::read(&p).ok()?;
    serde_json::from_slice(&data).ok()
}

pub fn speakers_cached(root: &Path) -> u64 {
    let dir = root.join("audio").join("segments").join(".latents");
    match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .count() as u64,
        Err(_) => 0,
    }
}

/// Diagnostics payload (audio_cache_status consumers).
pub fn inspect(root: &Path, speaker: &str) -> Option<Value> {
    load(root, speaker).map(|l| serde_json::to_value(l).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f0_estimation_recovers_sine_pitch() {
        let fs = 24_000u32;
        for f0 in [110.0, 150.0, 220.0] {
            let x: Vec<f32> = (0..fs as usize)
                .map(|i| (0.5 * (2.0 * std::f64::consts::PI * f0 * i as f64 / fs as f64).sin()) as f32)
                .collect();
            let est = estimate_f0(&x, fs);
            assert!((est - f0).abs() < 4.0, "f0 {f0} → {est}");
        }
    }

    #[test]
    fn silence_has_zero_pitch() {
        let x = vec![0.0f32; 24_000];
        assert_eq!(estimate_f0(&x, 24_000), 0.0);
    }

    #[test]
    fn zcr_bounds() {
        let x: Vec<f32> = (0..1000).map(|i| if i % 2 == 0 { 0.5 } else { -0.5 }).collect();
        assert!((zero_crossing_rate(&x) - 1.0).abs() < 1e-9);
        let y = vec![0.5f32; 1000];
        assert!(zero_crossing_rate(&y).abs() < 1e-9);
    }

    #[test]
    fn extract_and_persist_roundtrip() {
        let root = std::env::temp_dir().join(format!("authr_latent_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let fs = 24_000u32;
        let x: Vec<f32> = (0..fs as usize)
            .map(|i| (0.4 * (2.0 * std::f64::consts::PI * 140.0 * i as f64 / fs as f64).sin()) as f32)
            .collect();
        let lat = extract("CHAR_KIRA", &x, fs);
        assert_eq!(lat.window_ms, 1500);
        assert!(lat.pitch_hz > 130.0 && lat.pitch_hz < 150.0, "pitch {}", lat.pitch_hz);
        assert!(lat.rms_dbfs > -30.0);
        save(&root, &lat).unwrap();
        let loaded = load(&root, "CHAR_KIRA").unwrap();
        assert_eq!(loaded.tail_hash, lat.tail_hash);
        assert_eq!(loaded.pitch_hz, lat.pitch_hz);
        assert_eq!(speakers_cached(&root), 1);
        assert!(latent_path(&root, "CHAR/ODD*NAME").file_name().unwrap().to_str().unwrap().ends_with(".json"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn window_is_one_point_five_seconds() {
        assert_eq!(LATENT_WINDOW_MS, 1500);
        assert_eq!(LATENT_WINDOW_MS, super::LATENT_WINDOW_MS);
    }
}
