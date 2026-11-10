// AUTHR_OS — Two-Pass EBU R128 loudness normalizer (build plan §5.4 / Sprint 11).
// Pass 1 measures the BS.1770-4 K-weighted integrated loudness (400 ms blocks,
// 75 % overlap, absolute −70 LUFS + relative −10 LU gating); pass 2 applies the
// linear gain that lands the program at −20.0 LUFS (ACX target band −23…−18).
//
// The K-weighting biquads are derived with the pre-warped bilinear transform so
// they generalize to any sample rate. Numerically pinned (see tests): a 0 dBFS
// 997 Hz mono sine measures −3.05 LUFS at 24k/44.1k/48k, and the level→LUFS
// delta is exactly the input delta.

use crate::audio::dsp::{Biquad, db_to_lin};

pub const TARGET_INTEGRATED_LUFS: f64 = -20.0;
pub const ACX_LOUDNESS_MIN: f64 = -23.0;
pub const ACX_LOUDNESS_MAX: f64 = -18.0;

/// K-weighted signal (high shelf → RLB high-pass), f64 intermediate.
pub fn k_weight(x: &[f32], fs: u32) -> Vec<f64> {
    let mut shelf = Biquad::k_shelf(fs);
    let mut hp = Biquad::k_highpass(fs);
    x.iter().map(|&s| hp.process(shelf.process(s as f64))).collect()
}

/// Block mean-squares per BS.1770-4 (400 ms window, 100 ms hop).
fn block_ms(y: &[f64], fs: u32) -> Vec<f64> {
    let block = (fs as f64 * 0.4) as usize;
    let hop = (fs as f64 * 0.1) as usize;
    if y.len() < block {
        return vec![];
    }
    // sliding window via incremental sums: O(n)
    let mut out = Vec::with_capacity((y.len() - block) / hop + 1);
    let mut sum: f64 = y[..block].iter().map(|v| v * v).sum();
    out.push(sum / block as f64);
    let mut start = 0usize;
    while start + block + hop <= y.len() {
        let leaving: f64 = y[start..start + hop].iter().map(|v| v * v).sum();
        let entering: f64 = y[start + block..start + block + hop].iter().map(|v| v * v).sum();
        sum += entering - leaving;
        start += hop;
        out.push(sum / block as f64);
    }
    out
}

/// Integrated loudness (LUFS) with the two BS.1770 gates.
pub fn integrated_lufs(x: &[f32], fs: u32) -> f64 {
    let y = k_weight(x, fs);
    let ms = block_ms(&y, fs);
    if ms.is_empty() {
        return f64::NEG_INFINITY;
    }
    // absolute gate: −70 LUFS  (block loudness L = −0.691 + 10·log10(ms))
    let abs_floor = db_to_lin(-70.0 + 0.691).powi(2);
    let kept: Vec<f64> = ms.iter().copied().filter(|&m| m > abs_floor).collect();
    let pool = if kept.is_empty() { &ms[..] } else { &kept[..] };
    // relative gate: −10 LU below the ungated loudness of survivors
    let mean: f64 = pool.iter().sum::<f64>() / pool.len() as f64;
    let rel_loudness = -0.691 + 10.0 * mean.log10();
    let rel_floor = db_to_lin(rel_loudness - 10.0 + 0.691).powi(2);
    let final_pool: Vec<f64> = pool.iter().copied().filter(|&m| m > rel_floor).collect();
    let pool2 = if final_pool.is_empty() { pool } else { &final_pool[..] };
    let mean2: f64 = pool2.iter().sum::<f64>() / pool2.len() as f64;
    -0.691 + 10.0 * mean2.log10()
}

/// Two-pass linear normalization to `target_lufs`. Returns the mastered
/// buffer plus the measurement/gain audit trail.
pub struct TwoPass {
    pub before_lufs: f64,
    pub applied_gain_db: f64,
    pub after_lufs: f64,
}

pub fn normalize_two_pass(x: &[f32], fs: u32, target_lufs: f64) -> (Vec<f32>, TwoPass) {
    let before = integrated_lufs(x, fs);
    let gain_db = if before.is_finite() { target_lufs - before } else { 0.0 };
    let g = db_to_lin(gain_db) as f32;
    let out: Vec<f32> = x.iter().map(|&s| s * g).collect();
    let after = integrated_lufs(&out, fs);
    (out, TwoPass { before_lufs: before, applied_gain_db: gain_db, after_lufs: after })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::dsp::MASTER_FS;

    fn sine(dbfs: f64, freq: f64, seconds: f64, fs: u32) -> Vec<f32> {
        let a = db_to_lin(dbfs) as f32;
        (0..(seconds * fs as f64) as usize)
            .map(|i| a * (2.0 * std::f64::consts::PI * freq * i as f64 / fs as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn full_scale_sine_reference() {
        // Pinned numerically in scripts/phase4_verify_constants.py
        for fs in [24_000u32, 44_100, 48_000] {
            let x = sine(0.0, 997.0, 5.0, fs);
            let l = integrated_lufs(&x, fs);
            assert!((l - (-3.05)).abs() < 0.1, "fs={fs}: {l} LUFS");
        }
    }

    #[test]
    fn level_delta_is_exact() {
        let fs = MASTER_FS;
        let a = integrated_lufs(&sine(0.0, 997.0, 4.0, fs), fs);
        let b = integrated_lufs(&sine(-20.0, 997.0, 4.0, fs), fs);
        assert!((a - b - 20.0).abs() < 0.05, "delta {a} - {b}");
    }

    #[test]
    fn gates_ignore_silence_and_beneath_noise() {
        let fs = MASTER_FS;
        let mut x = sine(-10.0, 997.0, 4.0, fs); // loud part
        x.extend(vec![0.0f32; 3 * fs as usize]); // digital silence
        x.extend(sine(-60.0, 997.0, 3.0, fs)); // beneath-noise floor
        let l = integrated_lufs(&x, fs);
        assert!((l - (-13.05)).abs() < 0.3, "gating failed: {l} LUFS");
    }

    #[test]
    fn two_pass_lands_on_target() {
        let fs = MASTER_FS;
        let x = sine(-9.0, 997.0, 4.0, fs); // ≈ -12 LUFS
        let (y, rep) = normalize_two_pass(&x, fs, TARGET_INTEGRATED_LUFS);
        assert!((rep.after_lufs - TARGET_INTEGRATED_LUFS).abs() < 0.05,
            "after {} target {}", rep.after_lufs, TARGET_INTEGRATED_LUFS);
        assert!(rep.applied_gain_db < 0.0 && rep.applied_gain_db > -15.0);
        assert!((integrated_lufs(&y, fs) - TARGET_INTEGRATED_LUFS).abs() < 0.05);
    }

    #[test]
    fn peak_not_touched_by_normalize() {
        let fs = MASTER_FS;
        let x = sine(-3.0, 997.0, 2.0, fs);
        let (y, rep) = normalize_two_pass(&x, fs, TARGET_INTEGRATED_LUFS);
        let peak_in = x.iter().fold(0.0f32, |m, v| m.max(*v)).abs();
        let peak_out = y.iter().fold(0.0f32, |m, v| m.max(*v)).abs();
        assert!((peak_out - peak_in * db_to_lin(rep.applied_gain_db) as f32).abs() < 1e-3);
    }
}
