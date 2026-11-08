// AUTHR_OS — bare-metal DSP kernels (Phase 4, Sprint 11).
//
// All functions are pure (no I/O, no allocation beyond the working buffers)
// and fully deterministic, so mastering output is byte-stable for a given
// input — a hard requirement of the E2E suite.

/// One-pole-friendly biquad (direct form I transposed via state pair).
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// K-weighting stage 1 — high shelf (+4 dB @ ~1.68 kHz), BS.1770/pyloudnorm
/// parameterization, valid at arbitrary sample rates via tan() pre-warp.
fn high_shelf(fs: f64) -> Biquad {
    let f0 = 1681.974_450_955_533;
    let g = 3.999_843_853_973_347;
    let q = 0.707_175_236_955_419_6;
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    Biquad {
        b0: (vh + vb * k / q + k * k) / a0,
        b1: 2.0 * (k * k - vh) / a0,
        b2: (vh - vb * k / q + k * k) / a0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / q + k * k) / a0,
        z1: 0.0,
        z2: 0.0,
    }
}

/// K-weighting stage 2 — RLB high-pass (~38 Hz).
fn high_pass(fs: f64) -> Biquad {
    let f0 = 38.135_470_876_024_44;
    let q = 0.500_327_037_323_877_3;
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let a0 = 1.0 + k / q + k * k;
    Biquad {
        b0: 1.0,
        b1: -2.0,
        b2: 1.0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / q + k * k) / a0,
        z1: 0.0,
        z2: 0.0,
    }
}

/// ITU-R BS.1770-4 integrated loudness (LUFS), mono channel weight 1.0.
/// 400 ms blocks / 100 ms hop, absolute gate −70 LUFS + relative gate −10 LU.
pub fn integrated_loudness_lufs(samples: &[f32], sample_rate: u32) -> f64 {
    let fs = sample_rate as f64;
    let mut shelf = high_shelf(fs);
    let mut rlb = high_pass(fs);
    let filtered: Vec<f64> = samples
        .iter()
        .map(|&s| rlb.run(shelf.run(s as f64)))
        .collect();
    if filtered.is_empty() {
        return -f64::INFINITY;
    }
    let block = ((0.4 * fs) as usize).max(1);
    let hop = ((0.1 * fs) as usize).max(1);
    let mut block_ms: Vec<f64> = Vec::new();
    let mut start = 0usize;
    while start + block <= filtered.len() {
        let sum: f64 = filtered[start..start + block].iter().map(|v| v * v).sum();
        block_ms.push(sum / block as f64);
        start += hop;
    }
    if block_ms.is_empty() {
        // Signal shorter than one block: measure the whole thing.
        let sum: f64 = filtered.iter().map(|v| v * v).sum();
        block_ms.push(sum / filtered.len() as f64);
    }
    let loudness = |ms: f64| -0.691 + 10.0 * ms.max(1e-20).log10();
    // Absolute gate.
    let kept: Vec<f64> = block_ms
        .iter()
        .copied()
        .filter(|ms| loudness(*ms) >= -70.0)
        .collect();
    if kept.is_empty() {
        return -f64::INFINITY;
    }
    // Relative gate (−10 LU below the ungated mean).
    let mean_ms: f64 = kept.iter().sum::<f64>() / kept.len() as f64;
    let rel_threshold = loudness(mean_ms) - 10.0;
    let kept2: Vec<f64> = block_ms
        .iter()
        .copied()
        .filter(|ms| loudness(*ms) >= rel_threshold)
        .collect();
    let mean2: f64 = kept2.iter().sum::<f64>() / kept2.len() as f64;
    loudness(mean2)
}

/// RMS level in dBFS (20·log10 of the root-mean-square).
pub fn rms_dbfs(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return -f64::INFINITY;
    }
    let sum: f64 = samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
    let rms = (sum / samples.len() as f64).sqrt();
    20.0 * rms.max(1e-12).log10()
}

/// True peak magnitude in dBFS (sample peak, no oversampling).
pub fn peak_dbfs(samples: &[f32]) -> f64 {
    let peak = samples.iter().fold(0.0f64, |m, s| m.max(s.abs() as f64));
    20.0 * peak.max(1e-12).log10()
}

/// Noise-floor estimate: RMS of the trailing 500 ms window in dBFS.
pub fn noise_floor_dbfs(samples: &[f32], sample_rate: u32) -> f64 {
    let tail = (sample_rate as usize / 2).min(samples.len());
    if tail == 0 {
        return -f64::INFINITY;
    }
    rms_dbfs(&samples[samples.len() - tail..])
}

/// Linear gain for a dB value.
pub fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// Sidechain ducking — the ambient layer follows a smoothed envelope of the
/// speech signal: −18 dB duck depth, 120 ms attack, 750 ms release (§Sprint 11).
pub fn sidechain_duck(
    ambient: &mut [f32],
    speech: &[f32],
    sample_rate: u32,
    depth_db: f32,
    attack_ms: f32,
    release_ms: f32,
) -> u32 {
    let depth = db_to_linear(-depth_db.abs() as f64) as f32;
    let silence_gate = db_to_linear(-50.0) as f32;
    let alpha = |ms: f32| (-(1.0 / (ms.max(0.01) * sample_rate as f32 / 1000.0))).exp();
    let a_att = alpha(attack_ms);
    let a_rel = alpha(release_ms);
    let a_env = alpha(10.0); // 10 ms RMS-ish envelope
    let mut env = 0.0f32;
    let mut gain = 1.0f32;
    let mut ducked = 0u32;
    for (i, out) in ambient.iter_mut().enumerate() {
        let key = speech.get(i).copied().unwrap_or(0.0).abs();
        env = a_env * env + (1.0 - a_env) * key;
        let active = env > silence_gate;
        let target = if active { depth } else { 1.0 };
        gain = if target < gain {
            a_att * gain + (1.0 - a_att) * target
        } else {
            a_rel * gain + (1.0 - a_rel) * target
        };
        if gain < 0.99 {
            ducked += 1;
        }
        *out *= gain;
    }
    ducked
}

/// Sliding window maximum of |x|: `out[i] = max(|x[i..i+w]|)` (deque, O(n)).
fn sliding_max_abs(x: &[f32], w: usize) -> Vec<f32> {
    let n = x.len();
    let mut out = vec![0.0f32; n];
    let w = w.max(1);
    let mut dq: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for j in 0..n {
        let a = x[j].abs();
        while let Some(&b) = dq.back() {
            if x[b].abs() <= a {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(j);
        if j >= w - 1 {
            let i = j + 1 - w;
            while dq.front().map_or(false, |&f| f < i) {
                dq.pop_front();
            }
            if let Some(&f) = dq.front() {
                out[i] = x[f].abs();
            }
        }
    }
    for i in n.saturating_sub(w - 1)..n {
        while dq.front().map_or(false, |&f| f < i) {
            dq.pop_front();
        }
        if let Some(&f) = dq.front() {
            out[i] = x[f].abs();
        }
    }
    out
}

/// Lookahead brickwall peak limiter. Output sample *i* plays the signal
/// delayed by the lookahead window; its gain derives from the peak of the
/// trailing window ending at the *current* input sample, so the delayed
/// sample is always inside the measured window and the output can never
/// exceed the ceiling (−3.1 dBFS ACX ceiling by default).
pub fn lookahead_limiter(samples: &mut [f32], sample_rate: u32, ceiling_db: f32, lookahead_ms: f32) -> bool {
    let ceiling = db_to_linear(ceiling_db as f64) as f32;
    let w = ((lookahead_ms.max(0.0) * sample_rate as f32 / 1000.0) as usize).max(1);
    let slide = sliding_max_abs(samples, w); // slide[i] = max |x[i..i+w]|
    let fs = sample_rate as f32;
    let a_att = (-(1.0 / (0.5 * fs / 1000.0))).exp(); // 0.5 ms attack
    let a_rel = (-(1.0 / (50.0 * fs / 1000.0))).exp(); // 50 ms release
    let mut gain = 1.0f32;
    let mut limited = false;
    let mut delayed = vec![0.0f32; w];
    delayed.extend_from_slice(samples);
    delayed.truncate(samples.len());
    for (j, out) in samples.iter_mut().enumerate() {
        // Trailing peak ending at input sample j (partial window clamps to 0).
        let p_idx = (j + 1).saturating_sub(w);
        let peak = slide[p_idx];
        let target = (ceiling / peak.max(1e-9)).min(1.0);
        gain = if target < gain {
            a_att * gain + (1.0 - a_att) * target
        } else {
            a_rel * gain + (1.0 - a_rel) * target
        };
        if gain < 0.999 {
            limited = true;
        }
        *out = delayed[j] * gain;
    }
    limited
}

/// Apply a linear gain in place.
pub fn apply_gain(samples: &mut [f32], gain: f32) {
    for s in samples.iter_mut() {
        *s *= gain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, secs: f64, fs: u32, amp: f32) -> Vec<f32> {
        (0..(secs * fs as f64) as usize)
            .map(|i| (((amp as f64) * (2.0 * std::f64::consts::PI * freq * i as f64 / fs as f64).sin())) as f32)
            .collect()
    }

    #[test]
    fn k_weighting_sine_at_reference() {
        // BS.1770 mono: a 0 dBFS (amp 1.0) 1 kHz sine ≈ −3.01 LUFS, so a
        // −20 dBFS sine lands at ≈ −23.0 LUFS (net K-weight gain ≈ +0.7 dB).
        let x = sine(1000.0, 1.0, 48000, db_to_linear(-20.0) as f32);
        let lufs = integrated_loudness_lufs(&x, 48000);
        assert!((lufs - -23.0).abs() < 0.4, "lufs={lufs}");
        // Cross-check the reference point directly.
        let full = sine(1000.0, 1.0, 48000, 1.0);
        let l_full = integrated_loudness_lufs(&full, 48000);
        assert!((l_full - -3.01).abs() < 0.4, "l_full={l_full}");
    }

    #[test]
    fn loudness_scales_with_amplitude() {
        let a = sine(1000.0, 0.5, 44100, 0.1);
        let b = sine(1000.0, 0.5, 44100, 0.2);
        let la = integrated_loudness_lufs(&a, 44100);
        let lb = integrated_loudness_lufs(&b, 44100);
        assert!((lb - la - 6.02).abs() < 0.3, "la={la} lb={lb}");
    }

    #[test]
    fn rms_and_peak_match_definitions() {
        let x = sine(440.0, 0.1, 48000, 0.5);
        assert!((rms_dbfs(&x) - (-9.03)).abs() < 0.1);
        assert!((peak_dbfs(&x) - (-6.02)).abs() < 0.05);
    }

    #[test]
    fn limiter_caps_peak_at_ceiling() {
        let mut x = sine(1000.0, 0.2, 48000, 0.9); // ≈ −0.9 dBFS
        let was_limited = lookahead_limiter(&mut x, 48000, -3.1, 5.0);
        assert!(was_limited);
        assert!(peak_dbfs(&x) <= -3.05, "peak={}", peak_dbfs(&x));
        // Quiet signal passes untouched.
        let mut q = sine(1000.0, 0.2, 48000, 0.01);
        assert!(!lookahead_limiter(&mut q, 48000, -3.1, 5.0));
    }

    #[test]
    fn ducking_attenuates_ambient_under_speech() {
        let speech = sine(220.0, 0.4, 48000, 0.5);
        let mut ambient = sine(110.0, 0.4, 48000, 0.3);
        let ducked_n = sidechain_duck(&mut ambient, &speech, 48000, 18.0, 120.0, 750.0);
        assert!(ducked_n > 0);
        // After attack the ambient is ≈ −18 dB down (steady state ~late window).
        let late = &ambient[ambient.len() - 4800..];
        assert!(
            (rms_dbfs(late) - (rms_dbfs(&sine(110.0, 0.1, 48000, 0.3)) - 18.0)).abs() < 3.0,
            "late rms={}",
            rms_dbfs(late)
        );
    }

    #[test]
    fn sliding_max_basic() {
        let x = vec![0.1, 0.5, -0.9, 0.2, 0.7, 0.0];
        let m = sliding_max_abs(&x, 3);
        // windows: [0.1,0.5,0.9]→0.9, [0.5,0.9,0.2]→0.9, [0.9,0.2,0.7]→0.9,
        //          [0.2,0.7,0.0]→0.7, tail [0.7,0.0]→0.7, [0.0]→0.0
        assert_eq!(m[0], 0.9);
        assert_eq!(m[1], 0.9);
        assert_eq!(m[2], 0.9);
        assert_eq!(m[3], 0.7);
        assert_eq!(m[4], 0.7);
        assert_eq!(m[5], 0.0);
    }
}
