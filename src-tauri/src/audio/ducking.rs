// AUTHR_OS — Dynamic sidechain ducking (build plan §5.4, exact parameters):
//   • Speech RMS monitored against the ambient bed sidechain.
//   • Speech above -42 dBFS → ambient attenuated by -18 dB.
//   • Attack 120 ms · Hold 200 ms · Release 750 ms.
// The gain envelope ramps LINEARLY IN dB (log-domain ducker): the dip reaches
// the full -18 dB at the end of the attack window and recovers to 0 dB over
// the release window after the 200 ms hold. A 10 ms block RMS detector drives
// the target; the per-sample ramp is click-free by construction.

use crate::audio::dsp::{db_to_lin, lin_to_db};

#[derive(Clone, Debug)]
pub struct DuckingParams {
    pub threshold_dbfs: f64,
    pub attenuation_db: f64,
    pub attack_ms: f64,
    pub hold_ms: f64,
    pub release_ms: f64,
    pub block_ms: f64,
}

impl Default for DuckingParams {
    fn default() -> Self {
        DuckingParams {
            threshold_dbfs: -42.0,
            attenuation_db: -18.0,
            attack_ms: 120.0,
            hold_ms: 200.0,
            release_ms: 750.0,
            block_ms: 10.0,
        }
    }
}

/// Duck `ambient` against the `speech` envelope. Returns the ducked ambient
/// and the linear gain envelope applied (same length as `ambient`).
pub fn sidechain_duck_env(
    speech: &[f32],
    ambient: &[f32],
    fs: u32,
    p: &DuckingParams,
) -> (Vec<f32>, Vec<f32>) {
    let n = ambient.len();
    let block = ((fs as f64 * p.block_ms / 1000.0).round() as usize).max(1);
    let dip = p.attenuation_db; // negative dB
    let hold_blocks = ((p.hold_ms / p.block_ms).round() as i64).max(0);
    // dB-per-sample ramp rates
    let rate_attack = dip / (p.attack_ms / 1000.0 * fs as f64).max(1.0); // dip / attack_samples
    let rate_release = (-dip) / (p.release_ms / 1000.0 * fs as f64).max(1.0);

    let mut out = Vec::with_capacity(n);
    let mut env = Vec::with_capacity(n);
    let mut dip_db = 0.0f64; // current attenuation (≤ 0)
    let mut hold_left: i64 = 0;
    let mut pos = 0usize;

    while pos < n {
        let end = (pos + block).min(n);
        let s_end = end.min(speech.len());
        let window = &speech[pos.min(speech.len())..s_end];
        let rms = if window.is_empty() {
            -120.0
        } else {
            let ms = window.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>()
                / window.len() as f64;
            lin_to_db(ms.sqrt())
        };

        let target_dip: f64 = if rms > p.threshold_dbfs {
            hold_left = hold_blocks;
            dip
        } else if hold_left > 0 {
            hold_left -= 1;
            dip
        } else {
            0.0
        };

        for i in pos..end {
            if target_dip < dip_db {
                dip_db = (dip_db + rate_attack).max(target_dip); // dive toward dip
            } else if target_dip > dip_db {
                dip_db = (dip_db + rate_release).min(target_dip); // recover toward 0
            }
            let gain = db_to_lin(dip_db) as f32;
            env.push(gain);
            out.push(ambient[i] * gain);
        }
        pos = end;
    }
    (out, env)
}

pub fn sidechain_duck(speech: &[f32], ambient: &[f32], fs: u32, p: &DuckingParams) -> Vec<f32> {
    sidechain_duck_env(speech, ambient, fs, p).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::dsp::MASTER_FS;

    fn const_tone(dbfs: f64, samples: usize) -> Vec<f32> {
        let a = db_to_lin(dbfs) as f32;
        vec![a; samples]
    }

    fn rms_of(x: &[f32]) -> f64 {
        (x.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt()
    }

    fn gain_db(g: f32) -> f64 {
        20.0 * (g as f64).log10()
    }

    #[test]
    fn steady_speech_attenuates_by_18db() {
        let fs = MASTER_FS;
        let dur = 2 * fs as usize;
        let speech = const_tone(-20.0, dur); // -20 dBFS > -42 dBFS threshold
        let ambient = const_tone(-30.0, dur);
        let (out, env) = sidechain_duck_env(&speech, &ambient, fs, &DuckingParams::default());
        let tail = &out[(1.6 * fs as f64) as usize..];
        let lvl = lin_to_db(rms_of(tail));
        assert!((lvl - (-48.0)).abs() < 1.0, "ducked level {lvl} vs expected -48 dBFS");
        let g = env[(1.6 * fs as f64) as usize];
        assert!((gain_db(g) + 18.0).abs() < 0.5, "gain {} dB", gain_db(g));
    }

    #[test]
    fn attack_time_is_about_120ms() {
        let fs = MASTER_FS;
        let silence = vec![0.0f32; fs as usize];
        let mut speech = silence;
        speech.extend(const_tone(-20.0, fs as usize));
        let ambient = const_tone(-30.0, 2 * fs as usize, );
        let (_, env) = sidechain_duck_env(&speech, &ambient, fs, &DuckingParams::default());
        let at_onset = env[fs as usize];
        let at_120 = env[fs as usize + (0.119 * fs as f64) as usize];
        assert!(gain_db(at_onset) > -0.5, "onset gain {} dB — ducked too early", gain_db(at_onset));
        assert!(gain_db(at_120) < -16.5, "attack too slow: {} dB after 120 ms", gain_db(at_120));
    }

    #[test]
    fn hold_and_release_recover_after_750ms() {
        let fs = MASTER_FS;
        let speech = const_tone(-20.0, fs as usize); // 1 s speech
        let tail_silence = vec![0.0f32; 2 * fs as usize]; // 2 s gap
        let mut sp = speech;
        sp.extend(tail_silence);
        let ambient = const_tone(-30.0, 3 * fs as usize, );
        let (_, env) = sidechain_duck_env(&sp, &ambient, fs, &DuckingParams::default());
        // 200 ms hold + release: 600 ms after end → still ≥ 6 dB dipped
        let during_release = env[(1.6 * fs as f64) as usize];
        assert!(gain_db(during_release) < -6.0, "released too fast: {} dB", gain_db(during_release));
        // hold 200 ms + release 750 ms ≈ 0.95 s → fully recovered
        let recovered = env[(1.98 * fs as f64) as usize];
        assert!(gain_db(recovered) > -1.0, "release too slow: {} dB", gain_db(recovered));
    }

    #[test]
    fn hold_prevents_flutter_between_syllables() {
        let fs = MASTER_FS;
        // syllable-like 50 ms bursts with 50 ms gaps — hold must keep gain dipped
        let mut speech = Vec::new();
        for _ in 0..10 {
            speech.extend(const_tone(-20.0, (0.05 * fs as f64) as usize));
            speech.extend(vec![0.0f32; (0.05 * fs as f64) as usize]);
        }
        let ambient = const_tone(-30.0, speech.len());
        let (_, env) = sidechain_duck_env(&speech, &ambient, fs, &DuckingParams::default());
        // deep inside a gap (t = 0.30 s): hold keeps the dip
        let in_gap = env[(0.30 * fs as f64) as usize];
        assert!(gain_db(in_gap) < -10.0, "flutter in gap: {} dB", gain_db(in_gap));
    }

    #[test]
    fn quiet_speech_does_not_duck() {
        let fs = MASTER_FS;
        let speech = const_tone(-60.0, fs as usize); // below -42 threshold
        let ambient = const_tone(-30.0, fs as usize);
        let (out, _) = sidechain_duck_env(&speech, &ambient, fs, &DuckingParams::default());
        for (&o, &a) in out.iter().zip(ambient.iter()) {
            assert!((o - a).abs() < 1e-6);
        }
    }

    #[test]
    fn length_matches_ambient_and_handles_empty_speech() {
        let fs = MASTER_FS;
        let ambient = const_tone(-30.0, fs as usize);
        let out = sidechain_duck(&[], &ambient, fs, &DuckingParams::default());
        assert_eq!(out.len(), ambient.len());
    }
}
