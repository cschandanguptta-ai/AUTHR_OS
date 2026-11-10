// AUTHR_OS — ACX mastering pipeline (Phase 4, Sprint 11).
//
// Chain: 3-layer mix → two-pass normalize (default −20.0 dB RMS target,
// EBU R128/BS.1770 loudness reported alongside) → lookahead peak limiter
// (−3.1 dBFS ceiling) → ACX compliance report (RMS −23…−18 dB, peak ≤ −3 dB,
// noise floor ≤ −60 dB).

use crate::audio::dsp;
use serde_json::{json, Value};

pub const DEFAULT_TARGET_RMS_DB: f32 = -20.0;
pub const DEFAULT_CEILING_DB: f32 = -3.1;

pub struct MasterMetrics {
    pub target_rms_db: f64,
    pub rms_db: f64,
    pub peak_db: f64,
    pub lufs: f64,
    pub noise_floor_db: f64,
    pub gain_applied_db: f64,
    pub limited: bool,
}

/// Two-pass mastering: measure → gain → limit → re-measure.
pub fn master(
    samples: &[f32],
    rate: u32,
    target_rms_db: f32,
    ceiling_db: f32,
) -> (Vec<f32>, MasterMetrics) {
    let rms_before = dsp::rms_dbfs(samples);
    let gain = dsp::db_to_linear((target_rms_db as f64 - rms_before).clamp(-24.0, 24.0));
    let mut out = samples.to_vec();
    dsp::apply_gain(&mut out, gain as f32);
    let limited = dsp::lookahead_limiter(&mut out, rate, ceiling_db, 5.0);
    let metrics = MasterMetrics {
        target_rms_db: target_rms_db as f64,
        rms_db: dsp::rms_dbfs(&out),
        peak_db: dsp::peak_dbfs(&out),
        lufs: dsp::integrated_loudness_lufs(&out, rate),
        noise_floor_db: dsp::noise_floor_dbfs(&out, rate),
        gain_applied_db: 20.0 * gain.log10(),
        limited,
    };
    (out, metrics)
}

pub struct AcxReport {
    pub pass: bool,
    pub rms_db: f64,
    pub peak_db: f64,
    pub noise_floor_db: f64,
    pub rms_ok: bool,
    pub peak_ok: bool,
    pub noise_ok: bool,
    pub issues: Vec<String>,
}

/// ACX/Audible audiobook delivery check (mono 16-bit):
///   RMS within −23.0…−18.0 dB, peak ≤ −3.0 dBFS, noise floor ≤ −60 dB.
pub fn validate_acx(samples: &[f32], rate: u32) -> AcxReport {
    let rms_db = dsp::rms_dbfs(samples);
    let peak_db = dsp::peak_dbfs(samples);
    let noise_floor_db = dsp::noise_floor_dbfs(samples, rate);
    let rms_ok = (-23.0..=-18.0).contains(&rms_db);
    let peak_ok = peak_db <= -3.0;
    let noise_ok = noise_floor_db <= -60.0;
    let mut issues = Vec::new();
    if !rms_ok {
        issues.push(format!("RMS {rms_db:.2} dB outside ACX range [-23, -18]"));
    }
    if !peak_ok {
        issues.push(format!("peak {peak_db:.2} dBFS exceeds ACX ceiling -3 dBFS"));
    }
    if !noise_ok {
        issues.push(format!("noise floor {noise_floor_db:.2} dB above -60 dB"));
    }
    AcxReport { pass: issues.is_empty(), rms_db, peak_db, noise_floor_db, rms_ok, peak_ok, noise_ok, issues }
}

impl MasterMetrics {
    pub fn to_json(&self) -> Value {
        json!({
            "target_rms_db": self.target_rms_db,
            "rms_db": round2(self.rms_db),
            "peak_db": round2(self.peak_db),
            "lufs": round2(self.lufs),
            "noise_floor_db": round2(self.noise_floor_db),
            "gain_applied_db": round2(self.gain_applied_db),
            "limited": self.limited,
        })
    }
}

impl AcxReport {
    pub fn to_json(&self) -> Value {
        json!({
            "pass": self.pass,
            "rms_db": round2(self.rms_db),
            "peak_db": round2(self.peak_db),
            "noise_floor_db": round2(self.noise_floor_db),
            "checks": { "rms_ok": self.rms_ok, "peak_ok": self.peak_ok, "noise_ok": self.noise_ok },
            "issues": self.issues,
        })
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, secs: f64, fs: u32, amp: f32) -> Vec<f32> {
        (0..(secs * fs as f64) as usize)
            .map(|i| ((amp as f64) * (2.0 * std::f64::consts::PI * freq * i as f64 / fs as f64).sin()) as f32)
            .collect()
    }

    /// Full-scale 1-sample spikes — crest factor ≈ 20 dB, so after the −20 dB
    /// RMS normalize the peaks sit far above the −3.1 dBFS ceiling.
    fn spike_train(secs: f64, fs: u32, density: f64) -> Vec<f32> {
        let n = (secs * fs as f64) as usize;
        let step = (1.0 / density).max(1.0) as usize;
        (0..n).map(|i| if i % step == 0 { 1.0 } else { 0.0 }).collect()
    }

    #[test]
    fn master_hits_target_and_ceiling() {
        let x = sine(220.0, 1.0, 44100, 0.05); // peak −26 dB, RMS ≈ −29 dB
        let (out, m) = master(&x, 44100, DEFAULT_TARGET_RMS_DB, DEFAULT_CEILING_DB);
        assert!((m.rms_db - -20.0).abs() < 0.5, "rms={}", m.rms_db);
        assert!(m.peak_db <= -3.0, "peak={}", m.peak_db);
        assert!((m.gain_applied_db - 9.03).abs() < 0.6, "gain={}", m.gain_applied_db);
        assert_eq!(out.len(), x.len());
        assert!(m.lufs.is_finite());
    }

    #[test]
    fn hot_input_gets_limited_not_clipped() {
        let x = spike_train(1.0, 48000, 0.01); // RMS −20 dB, peaks at 0 dBFS
        let (out, m) = master(&x, 48000, DEFAULT_TARGET_RMS_DB, DEFAULT_CEILING_DB);
        assert!(m.limited, "spike peaks must engage the limiter");
        assert!(m.peak_db <= -3.0, "peak={}", m.peak_db);
        // Peak-shaving drops the RMS slightly below the target — still loud.
        assert!(m.rms_db > -24.0, "rms={}", m.rms_db);
        // No sample may exceed the ceiling in absolute terms.
        let max_lin = out.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        assert!(max_lin <= 0.708 + 1e-3, "max={max_lin}");
    }

    #[test]
    fn acx_report_flags_off_target_audio() {
        let quiet = sine(220.0, 0.5, 44100, 0.005); // ≈ −46 dB
        let rep = validate_acx(&quiet, 44100);
        assert!(!rep.pass);
        assert!(!rep.rms_ok);
        assert!(rep.peak_ok); // −46 dB peak is below ceiling
        assert!(rep.issues.iter().any(|i| i.contains("RMS")));
    }

    #[test]
    fn acx_report_passes_mastered_audio() {
        let x = sine(220.0, 1.0, 44100, 0.05);
        let (out, _) = master(&x, 44100, DEFAULT_TARGET_RMS_DB, DEFAULT_CEILING_DB);
        // Trailing 0.6 s of digital silence gives the 500 ms noise-floor
        // window a clean measurement bed without dropping RMS out of range.
        let mut deliverable = out.clone();
        deliverable.extend(vec![0.0f32; 44100 * 6 / 10]);
        let rep = validate_acx(&deliverable, 44100);
        assert!(rep.pass, "issues={:?}", rep.issues);
        assert!(rep.rms_ok && rep.peak_ok);
        assert!(rep.noise_floor_db < -60.0);
    }
}
