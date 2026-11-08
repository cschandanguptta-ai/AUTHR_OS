// AUTHR_OS — WAV I/O (hound). The pipeline works in mono f32 internally;
// files are written as 16-bit PCM mono (ACX delivery format).

use crate::error::AppError;
use std::path::Path;

pub struct WavData {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// Read any hound-supported WAV (i8/i16/i32/f32, any channel count) and
/// mix down to mono f32 in [-1.0, 1.0].
pub fn read_wav(path: &Path) -> Result<WavData, AppError> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| AppError::audio(format!("open wav {}: {e}", path.display())))?;
    let spec = reader.spec();
    let rate = spec.sample_rate;
    let channels = spec.channels.max(1) as f32;
    let mut mono: Vec<f32> = Vec::new();
    // Channel-aware conversion per sample format.
    mono.clear();
    let sample_iter: Box<dyn Iterator<Item = Result<f32, hound::Error>>> = {
        if spec.sample_format == hound::SampleFormat::Int
            && !matches!(spec.bits_per_sample, 8 | 16 | 24 | 32)
        {
            return Err(AppError::audio(format!(
                "unsupported bit depth {}",
                spec.bits_per_sample
            )));
        }
        match spec.sample_format {
            hound::SampleFormat::Float => Box::new(reader.into_samples::<f32>()),
            hound::SampleFormat::Int => match spec.bits_per_sample {
                8 => Box::new(reader.into_samples::<i8>().map(|r| r.map(|v| v as f32 / 128.0))),
                16 => Box::new(reader.into_samples::<i16>().map(|r| r.map(|v| v as f32 / 32768.0))),
                _ => Box::new(
                    reader
                        .into_samples::<i32>()
                        .map(|r| r.map(|v| v as f32 / 2147483648.0)),
                ),
            },
        }
    };
    // Average channels into mono.
    let mut acc = 0.0f32;
    let mut n = 0usize;
    for s in sample_iter {
        let v = s.map_err(|e| AppError::audio(format!("decode wav: {e}")))?;
        acc += v;
        n += 1;
        if n == spec.channels as usize {
            mono.push(acc / channels);
            acc = 0.0;
            n = 0;
        }
    }
    if n > 0 {
        mono.push(acc / n as f32);
    }
    Ok(WavData { sample_rate: rate, samples: mono })
}

/// Write mono f32 samples as 16-bit PCM WAV (atomic via `<name>.tmp` rename).
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), AppError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let tmp = path.with_extension("wav.tmp");
    let mut writer = hound::WavWriter::create(&tmp, spec)
        .map_err(|e| AppError::audio(format!("create wav {}: {e}", tmp.display())))?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        writer
            .write_sample(v)
            .map_err(|e| AppError::audio(format!("write sample: {e}")))?;
    }
    writer
        .finalize()
        .map_err(|e| AppError::audio(format!("finalize wav: {e}")))?;
    std::fs::rename(&tmp, path).map_err(|e| AppError::audio(format!("rename wav: {e}")))?;
    Ok(())
}
