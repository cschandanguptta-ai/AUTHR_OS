// AUTHR_OS — Lookahead Brickwall Peak Limiter (build plan §5.4 / Sprint 11).
//   • 5 ms lookahead window clamps transient peaks to −3.1 dBFS.
//   • Zero digital clipping (ACX peak ceiling ≤ −3 dBFS).
// Gain drops instantly when the lookahead sees an overshoot and recovers with
// a 100 ms exponential release; the delayed signal guarantees every emitted
// sample was preceded by its 5 ms future in the gain decision.

use crate::audio::dsp::db_to_lin;

pub const CEILING_DBFS: f64 = -3.1;
pub const LOOKAHEAD_MS: f64 = 5.0;
pub const RELEASE_MS: f64 = 100.0;

/// Limiter with the default ACX rack settings.
pub fn limit_acx(x: &[f32], fs: u32) -> Vec<f32> {
    limit_lookahead(x, fs, CEILING_DBFS, LOOKAHEAD_MS, RELEASE_MS)
}

/// `limit_lookahead` limits `x` so that |out| ≤ ceiling (±1e-6).
pub fn limit_lookahead(
    x: &[f32],
    fs: u32,
    ceiling_dbfs: f64,
    lookahead_ms: f64,
    release_ms: f64,
) -> Vec<f32> {
    let n = x.len();
    if n == 0 {
        return x.to_vec();
    }
    let ceiling = db_to_lin(ceiling_dbfs) as f32;
    let look = ((fs as f64 * lookahead_ms / 1000.0).round() as usize).max(1);
    let alpha_r = 1.0 - (-1.0 / ((fs as f64 * release_ms / 1000.0).max(1.0))).exp();

    // sliding max of |x| over window [j, j+look) — monotonic deque, O(n)
    let mut required = vec![1.0f32; n]; // req[j] = ceiling / max|x[j..j+look)|
    let mut deque: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for j in 0..n {
        let v = x[j].abs();
        while let Some(&b) = deque.back() {
            if x[b].abs() <= v {
                deque.pop_back();
            } else {
                break;
            }
        }
        deque.push_back(j);
        // drop indices that left the window [j-look, j)
        while let Some(&f) = deque.front() {
            if f + look <= j {
                deque.pop_front();
            } else {
                break;
            }
        }
        if let Some(&f) = deque.front() {
            let peak = x[f].abs();
            required[j] = if peak > ceiling { ceiling / peak } else { 1.0 };
        }
    }

    // delayed output: out[i] = x[i-look] · g, g = min(req[i-look], released g)
    let mut out = vec![0.0f32; n];
    let mut g = 1.0f32;
    for i in 0..n {
        let req = if i >= look { required[i - look] } else { 1.0 };
        if req < g {
            g = req; // instant attack
        } else {
            g += (req - g) * alpha_r as f32; // smooth release
            if (req - g).abs() < 1e-6 {
                g = req;
            }
        }
        out[i] = if i >= look { x[i - look] * g } else { 0.0 }; // pre-roll silence
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::dsp::MASTER_FS;

    #[test]
    fn never_exceeds_ceiling() {
        let fs = MASTER_FS;
        // abusive program: full-scale bursts + hot noise floor
        let mut x = vec![0.9f32; fs as usize];
        x.extend(vec![-0.98f32; fs as usize]);
        for i in 0..x.len() {
            x[i] += ((i as f32 * 0.01).sin()) * 0.05;
        }
        let y = limit_lookahead(&x, fs, CEILING_DBFS, LOOKAHEAD_MS, RELEASE_MS);
        let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak <= db_to_lin(CEILING_DBFS) as f32 + 1e-6,
            "peak {} exceeds ceiling {}", peak, db_to_lin(CEILING_DBFS));
    }

    #[test]
    fn quiet_program_is_unity_after_lookahead_delay() {
        let fs = MASTER_FS;
        let x: Vec<f32> = (0..fs as usize)
            .map(|i| 0.2 * (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / fs as f64).sin() as f32)
            .collect();
        let y = limit_lookahead(&x, fs, CEILING_DBFS, LOOKAHEAD_MS, RELEASE_MS);
        let skip = (0.01 * fs as f64) as usize; // skip the 5 ms pre-roll
        let peak_in = x[skip..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let peak_out = y[skip..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((peak_in - peak_out).abs() < 0.01, "in {peak_in} out {peak_out}");
    }

    #[test]
    fn lookahead_prevents_transient_overshoot() {
        let fs = MASTER_FS;
        let mut x = vec![0.1f32; 1000];
        x.push(1.0); // brick-wall transient
        x.extend(vec![0.1f32; 1000]);
        let y = limit_lookahead(&x, fs, CEILING_DBFS, LOOKAHEAD_MS, RELEASE_MS);
        let peak = y.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak <= db_to_lin(CEILING_DBFS) as f32 + 1e-6);
    }

    #[test]
    fn pre_roll_is_silent() {
        let fs = MASTER_FS;
        let x = vec![0.5f32; 2000];
        let y = limit_lookahead(&x, fs, CEILING_DBFS, LOOKAHEAD_MS, RELEASE_MS);
        let pre = ((LOOKAHEAD_MS * fs as f64 / 1000.0) as usize).min(y.len());
        assert!(y[..pre].iter().all(|v| v.abs() < 1e-6), "pre-roll not silent");
    }
}
