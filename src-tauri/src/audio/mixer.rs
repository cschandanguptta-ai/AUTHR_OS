// AUTHR_OS — 3-layer studio mixer (Phase 4, Sprint 11).
//
// Layers: Speech (dialogue bed, also the ducking sidechain key),
// Sfx (spot Foley events placed at offsets), Ambient (looped bed).
// All layers are resampled to the session rate with rubato before mixing.

use crate::audio::dsp;
use crate::error::AppError;
use rubato::Resampler;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Speech,
    Sfx,
    Ambient,
}

impl LayerKind {
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s.to_uppercase().as_str() {
            "SPEECH" => Ok(LayerKind::Speech),
            "SFX" | "SPOT" | "FOLEY" => Ok(LayerKind::Sfx),
            "AMBIENT" | "BED" => Ok(LayerKind::Ambient),
            other => Err(AppError::audio(format!(
                "unknown layer kind `{other}` (expected SPEECH|SFX|AMBIENT)"
            ))),
        }
    }
}

pub struct LayerSpec {
    pub kind: LayerKind,
    pub samples: Vec<f32>,
    pub rate: u32,
    pub gain_db: f32,
    pub offset_ms: f32,
}

pub struct DuckParams {
    pub depth_db: f32,   // 18.0
    pub attack_ms: f32,  // 120.0
    pub release_ms: f32, // 750.0
}

impl Default for DuckParams {
    fn default() -> Self {
        DuckParams { depth_db: 18.0, attack_ms: 120.0, release_ms: 750.0 }
    }
}

/// High-quality resampling (rubato FFT fixed-in/out).
pub fn resample(samples: &[f32], rate_in: u32, rate_out: u32) -> Result<Vec<f32>, AppError> {
    if rate_in == rate_out || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    let chunk_out = 1024usize;
    let mut resampler = rubato::FftFixedInOut::new(
        rate_in as usize,
        rate_out as usize,
        chunk_out,
        1,
    )
    .map_err(|e| AppError::audio(format!("resampler init {rate_in}->{rate_out}: {e}")))?;
    let chunk_in = resampler.input_frames_next();
    let mut padded = samples.to_vec();
    let pad = (chunk_in - padded.len() % chunk_in) % chunk_in;
    padded.extend(std::iter::repeat_n(0.0f32, pad));
    let mut out = Vec::with_capacity(padded.len() * rate_out as usize / rate_in as usize + chunk_out);
    for chunk in padded.chunks(chunk_in) {
        let processed = resampler
            .process(&[chunk.to_vec()], None)
            .map_err(|e| AppError::audio(format!("resample: {e}")))?;
        out.extend_from_slice(&processed[0]);
    }
    // Trim the interpolation padding back to the expected length.
    let expected = (samples.len() as u128 * rate_out as u128 / rate_in as u128) as usize;
    out.truncate(expected);
    Ok(out)
}

pub struct MixResult {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub ducked_samples: u32,
    pub layers: usize,
}

/// Mix the layer stack to `out_rate`, applying sidechain ducking of the
/// ambient layer(s) under the combined speech signal.
pub fn mix(layers: &[LayerSpec], out_rate: u32, duck: Option<&DuckParams>) -> Result<MixResult, AppError> {
    let mut speech_len = 0usize;
    let mut prepared: Vec<(LayerKind, Vec<f32>, f32, f64)> = Vec::new(); // (kind, resampled, gain_lin, offset_samples)
    for (n, layer) in layers.iter().enumerate() {
        let rs = resample(&layer.samples, layer.rate, out_rate)?;
        if matches!(layer.kind, LayerKind::Speech) && rs.len() > speech_len {
            speech_len = rs.len();
        }
        let gain = dsp::db_to_linear(layer.gain_db as f64) as f32;
        let offset = ((layer.offset_ms.max(0.0) * out_rate as f32 / 1000.0).round()) as usize;
        if !matches!(layer.kind, LayerKind::Sfx) && offset > 0 {
            return Err(AppError::audio(format!(
                "layer {n}: only SFX layers may carry an offset"
            )));
        }
        prepared.push((layer.kind, rs, gain, offset as f64));
    }

    // Output length: speech length if any speech, else longest layer.
    let mut total = speech_len;
    if total == 0 {
        for (kind, rs, _, off) in &prepared {
            if matches!(kind, LayerKind::Ambient | LayerKind::Sfx) {
                total = total.max(*off as usize + rs.len());
            }
        }
    }
    // Ambient loops up to the speech length.
    if speech_len > 0 {
        for (kind, rs, _, _) in prepared.iter_mut() {
            if matches!(kind, LayerKind::Ambient) && rs.len() < speech_len {
                let base = rs.clone();
                rs.reserve(speech_len - rs.len());
                while rs.len() < speech_len {
                    let take = (speech_len - rs.len()).min(base.len());
                    rs.extend_from_slice(&base[..take]);
                }
            }
        }
    }

    // Sum speech (sidechain key) first.
    let mut speech_signal = vec![0.0f32; total];
    for (kind, rs, gain, off) in &prepared {
        if matches!(kind, LayerKind::Speech) {
            for (i, &s) in rs.iter().enumerate() {
                let pos = *off as usize + i;
                if pos < total {
                    speech_signal[pos] += s * gain;
                }
            }
        }
    }

    let mut out = speech_signal.clone();
    let mut ducked_samples = 0u32;
    for (kind, rs, gain, off) in &prepared {
        match kind {
            LayerKind::Speech => {}
            LayerKind::Ambient => {
                let mut bed = rs.clone();
                if let Some(d) = duck {
                    ducked_samples += dsp::sidechain_duck(
                        &mut bed,
                        &speech_signal,
                        out_rate,
                        d.depth_db,
                        d.attack_ms,
                        d.release_ms,
                    );
                }
                for (i, &s) in bed.iter().enumerate() {
                    let pos = *off as usize + i;
                    if pos < total {
                        out[pos] += s * gain;
                    }
                }
            }
            LayerKind::Sfx => {
                for (i, &s) in rs.iter().enumerate() {
                    let pos = *off as usize + i;
                    if pos < total {
                        out[pos] += s * gain;
                    }
                }
            }
        }
    }

    Ok(MixResult { samples: out, rate: out_rate, ducked_samples, layers: layers.len() })
}

/// `audio_master` layer args: `{kind, wav, gain_db?, offset_ms?}` where `wav`
/// is bundle-relative and sandbox-resolved (traversal → PATH_TRAVERSAL_VIOLATION).
pub fn parse_layer_args_bundle(
    bundle_root: &std::path::Path,
    v: &Value,
) -> Result<LayerSpec, AppError> {
    let kind_s = v
        .get("kind")
        .and_then(|k| k.as_str())
        .ok_or_else(|| AppError::audio("layer missing `kind` (SPEECH|SFX|AMBIENT)"))?;
    let kind = LayerKind::parse(kind_s)?;
    let rel = v
        .get("wav")
        .and_then(|w| w.as_str())
        .ok_or_else(|| AppError::audio("layer missing `wav` path"))?;
    let path = crate::security::sandbox::resolve_in_root(bundle_root, std::path::Path::new(rel))?;
    let data = crate::audio::wav::read_wav(&path)?;
    Ok(LayerSpec {
        kind,
        samples: data.samples,
        rate: data.sample_rate,
        gain_db: v.get("gain_db").and_then(|g| g.as_f64()).unwrap_or(0.0) as f32,
        offset_ms: v.get("offset_ms").and_then(|o| o.as_f64()).unwrap_or(0.0) as f32,
    })
}

/// Ducking summary for the master payload.
pub fn duck_summary(ducked: u32, rate: u32) -> Value {
    json!({
        "ducked_samples": ducked,
        "ducked_ms": (ducked as f64 * 1000.0 / rate as f64).round() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, secs: f64, fs: u32, amp: f32) -> Vec<f32> {
        (0..(secs * fs as f64) as usize)
            .map(|i| ((amp as f64) * (2.0 * std::f64::consts::PI * freq * i as f64 / fs as f64).sin()) as f32)
            .collect()
    }

    #[test]
    fn resample_length_tracks_ratio() {
        let x = sine(440.0, 0.5, 48000, 0.5);
        let y = resample(&x, 48000, 44100).unwrap();
        assert!((y.len() as i64 - 22050).abs() < 10, "len={}", y.len());
        // Energy survives the conversion.
        assert!(dsp::rms_dbfs(&y) > -30.0);
    }

    #[test]
    fn three_layer_mix_ducks_ambient() {
        let speech = sine(220.0, 0.4, 48000, 0.5);
        let ambient = sine(110.0, 1.2, 48000, 0.4); // longer → looped/trimmed
        let sfx = sine(880.0, 0.05, 48000, 0.5);
        let layers = vec![
            LayerSpec { kind: LayerKind::Speech, samples: speech.clone(), rate: 48000, gain_db: 0.0, offset_ms: 0.0 },
            LayerSpec { kind: LayerKind::Ambient, samples: ambient, rate: 48000, gain_db: -6.0, offset_ms: 0.0 },
            LayerSpec { kind: LayerKind::Sfx, samples: sfx, rate: 48000, gain_db: -3.0, offset_ms: 100.0 },
        ];
        let mixed = mix(&layers, 48000, Some(&DuckParams::default())).unwrap();
        assert_eq!(mixed.samples.len(), speech.len());
        assert!(mixed.ducked_samples > 0);
        // SFX present around 100 ms.
        let probe: f32 = mixed.samples[4800..5800].iter().map(|s| s.abs()).sum();
        assert!(probe > 0.0);
    }

    #[test]
    fn speechless_mix_uses_longest_layer() {
        let ambient = sine(60.0, 0.3, 44100, 0.3);
        let layers = vec![LayerSpec {
            kind: LayerKind::Ambient,
            samples: ambient,
            rate: 44100,
            gain_db: 0.0,
            offset_ms: 0.0,
        }];
        let mixed = mix(&layers, 44100, None).unwrap();
        assert_eq!(mixed.samples.len(), 13230);
    }

    #[test]
    fn layer_kind_parse_rejects_garbage() {
        assert!(LayerKind::parse("NOPE").is_err());
        assert_eq!(LayerKind::parse("foley").unwrap(), LayerKind::Sfx);
    }
}
