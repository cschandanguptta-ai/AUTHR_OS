// AUTHR_OS — Bundled 40-cue Foley library (build plan §5.4 Layer 2:
// "Spot-SFX Foley, 40-Cue Bundled Library, point-in-time timestamp").
// Every cue is generated procedurally and deterministically (FNV-seeded
// xorshift) at the requested sample rate — no binary assets shipped.

use crate::audio::dsp::{fnv64, lin_to_db, lowpass, Biquad, Rng};
use serde_json::json;

pub struct Cue {
    pub id: &'static str,
    pub kind: &'static str,
    pub dur_ms: u32,
}

/// The 40 bundled Foley cues (Sprint 11 deliverable — `audio_foley_list`).
pub const CUES: &[Cue] = &[
    Cue { id: "airlock_hiss_heavy",      kind: "noise_burst", dur_ms: 900 },
    Cue { id: "airlock_cycle",           kind: "noise_burst", dur_ms: 1200 },
    Cue { id: "biometric_beep_accept",   kind: "beep",        dur_ms: 260 },
    Cue { id: "biometric_beep_reject",   kind: "beep",        dur_ms: 380 },
    Cue { id: "thermal_cutter_ignite",   kind: "sweep",       dur_ms: 800 },
    Cue { id: "heartbeat_low_thump",     kind: "thump",       dur_ms: 700 },
    Cue { id: "door_hydraulic_close",    kind: "thump",       dur_ms: 500 },
    Cue { id: "door_hydraulic_open",     kind: "noise_burst", dur_ms: 450 },
    Cue { id: "console_chirp_soft",      kind: "beep",        dur_ms: 120 },
    Cue { id: "console_chirp_error",     kind: "beep",        dur_ms: 200 },
    Cue { id: "alarm_klaxon_distant",    kind: "beep",        dur_ms: 900 },
    Cue { id: "alarm_klaxon_close",      kind: "beep",        dur_ms: 900 },
    Cue { id: "power_up_sequence",       kind: "sweep",       dur_ms: 1400 },
    Cue { id: "power_down_sequence",     kind: "sweep",       dur_ms: 1200 },
    Cue { id: "servo_whir_short",        kind: "servo",       dur_ms: 300 },
    Cue { id: "servo_whir_long",         kind: "servo",       dur_ms: 900 },
    Cue { id: "footstep_metal_single",   kind: "click",       dur_ms: 90 },
    Cue { id: "footstep_metal_double",   kind: "click",       dur_ms: 160 },
    Cue { id: "glass_crack_small",       kind: "static",      dur_ms: 140 },
    Cue { id: "glass_shatter",           kind: "static",      dur_ms: 600 },
    Cue { id: "thunder_distant",         kind: "noise_burst", dur_ms: 2200 },
    Cue { id: "thunder_close",           kind: "noise_burst", dur_ms: 1800 },
    Cue { id: "rain_window_tap",         kind: "click",       dur_ms: 300 },
    Cue { id: "cup_set_counter",         kind: "thump",       dur_ms: 200 },
    Cue { id: "pen_click",               kind: "click",       dur_ms: 60 },
    Cue { id: "paper_shuffled",          kind: "static",      dur_ms: 450 },
    Cue { id: "knife_chop_board",        kind: "thump",       dur_ms: 160 },
    Cue { id: "sword_draw_steel",        kind: "sweep",       dur_ms: 700 },
    Cue { id: "bow_release",             kind: "click",       dur_ms: 220 },
    Cue { id: "arrow_impact_wood",       kind: "thump",       dur_ms: 180 },
    Cue { id: "magic_chime_rise",        kind: "chime",       dur_ms: 1300 },
    Cue { id: "spell_burst_low",         kind: "thump",       dur_ms: 800 },
    Cue { id: "portal_open_whoosh",      kind: "sweep",       dur_ms: 1100 },
    Cue { id: "portal_close_pop",        kind: "thump",       dur_ms: 240 },
    Cue { id: "coin_drop_table",         kind: "click",       dur_ms: 300 },
    Cue { id: "lock_click_open",         kind: "click",       dur_ms: 120 },
    Cue { id: "lock_click_denied",       kind: "click",       dur_ms: 160 },
    Cue { id: "radio_static_burst",      kind: "static",      dur_ms: 350 },
    Cue { id: "engine_ignition_roar",    kind: "roar",        dur_ms: 1500 },
    Cue { id: "treefall_crackle",        kind: "noise_burst", dur_ms: 1600 },
];

pub fn find(id: &str) -> Option<&'static Cue> {
    CUES.iter().find(|c| c.id == id)
}

pub fn render(id: &str, fs: u32) -> Option<Vec<f32>> {
    let cue = find(id)?;
    let n = (cue.dur_ms as u64 * fs as u64 / 1000) as usize;
    let seed = fnv64(cue.id.as_bytes());
    let mut rng = Rng::new(seed);
    let mut out = vec![0.0f32; n];

    match cue.kind {
        "noise_burst" => {
            let cutoff = 400.0 + (seed % 2600) as f64;
            let mut lp = lowpass(fs, cutoff, 0.707);
            let mut hp = Biquad::highpass(fs, 45.0, 0.707);
            let attack = 0.05 * n as f64;
            for (i, v) in out.iter_mut().enumerate() {
                let env = if (i as f64) < attack {
                    i as f64 / attack.max(1.0)
                } else {
                    (-(i as f64 - attack) / (0.55 * n as f64)).exp()
                };
                *v = (hp.process(lp.process(rng.bipolar())) * env * 0.8) as f32;
            }
        }
        "beep" => {
            let f = 320.0 + (seed % 1800) as f64;
            let f2 = if seed % 2 == 0 { f * 1.5 } else { f * 0.66 };
            let ramp = (fs as f64 * 0.004) as usize;
            for (i, v) in out.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                let mut s = 0.6 * (2.0 * std::f64::consts::PI * f * t).sin()
                    + 0.35 * (2.0 * std::f64::consts::PI * f2 * t).sin();
                s *= gate(i, ramp, n);
                *v = (s * 0.55) as f32;
            }
        }
        "sweep" => {
            let f0 = 80.0 + (seed % 200) as f64;
            let f1 = 600.0 + (seed % 2400) as f64;
            let mut phase = 0.0f64;
            for (i, v) in out.iter_mut().enumerate() {
                let p = i as f64 / n as f64;
                let f = f0 * (f1 / f0).powf(p);
                phase += 2.0 * std::f64::consts::PI * f / fs as f64;
                let env = sin_env(p);
                *v = ((phase.sin() * 0.7 + rng.bipolar() * 0.25) * env * 0.6) as f32;
            }
        }
        "thump" => {
            let f = 48.0 + (seed % 70) as f64;
            let mut phase = 0.0f64;
            for (i, v) in out.iter_mut().enumerate() {
                let p = i as f64 / n as f64;
                let fdrop = f * (1.0 - 0.35 * p);
                phase += 2.0 * std::f64::consts::PI * fdrop / fs as f64;
                let env = (-p * 6.0).exp();
                *v = ((phase.sin() * 0.85 + rng.bipolar() * 0.06) * env) as f32;
            }
        }
        "click" => {
            let clicks = 1 + (seed % 3) as usize;
            for c in 0..clicks {
                let at = (n as u64 * (c as u64 + 1) / (clicks as u64 + 1)) as usize;
                let width = (fs as f64 * 0.003) as usize;
                for i in at..at.saturating_add(width).min(n) {
                    let env = 1.0 - (i - at) as f64 / width as f64;
                    out[i] += (rng.bipolar() * env * 0.7) as f32;
                }
            }
            let mut lp = lowpass(fs, 6500.0, 0.707);
            for v in out.iter_mut() {
                *v = lp.process_f32(*v);
            }
        }
        "chime" => {
            let f = 700.0 + (seed % 900) as f64;
            for (i, v) in out.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                let mut s = (2.0 * std::f64::consts::PI * f * t).sin() * (-t * 3.0).exp()
                    + (2.0 * std::f64::consts::PI * f * 2.756 * t).sin() * (-t * 5.0).exp() * 0.4
                    + (2.0 * std::f64::consts::PI * f * 5.404 * t).sin() * (-t * 8.0).exp() * 0.2;
                s *= (i as f64 / (fs as f64 * 0.01)).min(1.0); // 10 ms fade-in
                *v = (s * 0.5) as f32;
            }
        }
        "servo" => {
            let f = 180.0 + (seed % 220) as f64;
            let mut phase = 0.0f64;
            for (i, v) in out.iter_mut().enumerate() {
                let p = i as f64 / n as f64;
                let fsw = f * (if p < 0.5 { 1.0 + p } else { 2.0 - p });
                phase += 2.0 * std::f64::consts::PI * fsw / fs as f64;
                let trem = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 40.0 * (i as f64 / fs as f64)).sin();
                *v = ((phase.sin() * trem) * sin_env(p) * 0.4) as f32;
            }
        }
        "static" => {
            let mut lp = lowpass(fs, 3200.0 + (seed % 3000) as f64, 0.707);
            let mut hp = Biquad::highpass(fs, 300.0, 0.707);
            let mut gated = vec![0.0f64; n];
            let mut gate_on = 0usize;
            for g in gated.iter_mut() {
                if gate_on == 0 {
                    gate_on = ((fs as u64 * (20 + rng.next_u64() % 60) / 1000) as usize).max(4);
                }
                gate_on -= 1;
                *g = if rng.next_f64() < 0.55 { 1.0 } else { 0.0 };
            }
            for (i, v) in out.iter_mut().enumerate() {
                let env = (-(i as f64) / (0.7 * n as f64)).exp();
                *v = (hp.process(lp.process(rng.bipolar() * gated[i])) * env * 0.6) as f32;
            }
        }
        "roar" => {
            let mut lp = lowpass(fs, 900.0, 0.707);
            let mut phase = 0.0f64;
            let attack = (fs as f64 * 0.25) as usize;
            for (i, v) in out.iter_mut().enumerate() {
                let env = if i < attack { i as f64 / attack as f64 }
                    else { (-((i - attack) as f64) / (0.8 * n as f64)).exp() };
                phase += 2.0 * std::f64::consts::PI * (70.0 + rng.next_f64() * 30.0) / fs as f64;
                let saw = 2.0 * ((i as f64 * 0.023) % 1.0) - 1.0;
                *v = ((lp.process(rng.bipolar()) * 0.6 + phase.sin() * 0.3 + saw * 0.15) * env * 0.7) as f32;
            }
        }
        _ => {}
    }
    Some(out)
}

/// RMS level in dBFS of a rendered cue (diagnostics / tests).
pub fn cue_level_dbfs(id: &str, fs: u32) -> Option<f64> {
    render(id, fs).map(|x| lin_to_db(rms(&x)))
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() { return 0.0; }
    (x.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / x.len() as f64).sqrt()
}


fn sin_env(p: f64) -> f64 {
    (std::f64::consts::PI * p).sin().powi(2)
}

fn gate(i: usize, ramp: usize, n: usize) -> f64 {
    let mut g = 1.0;
    if i < ramp {
        g *= i as f64 / ramp as f64;
    }
    if n.saturating_sub(i) < ramp {
        g *= (n - i) as f64 / ramp as f64;
    }
    g
}

/// IPC #41 `audio_foley_list` — the bundled cue registry.
pub fn list() -> serde_json::Value {
    let cues: Vec<serde_json::Value> = CUES
        .iter()
        .map(|c| json!({"cue": c.id, "generator": c.kind, "duration_ms": c.dur_ms}))
        .collect();
    json!({ "cues": cues, "count": cues.len(), "sample_rate_hz": crate::audio::dsp::MASTER_FS })
}

/// Level sanity: every cue must render finite samples at the given rate.
pub fn all_cues_renderable(fs: u32) -> bool {
    CUES.iter().all(|c| {
        render(c.id, fs)
            .map(|x| x.iter().all(|v| v.is_finite()))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_exactly_40_cues() {
        assert_eq!(CUES.len(), 40);
        let mut ids: Vec<_> = CUES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 40, "duplicate cue ids");
    }

    #[test]
    fn every_cue_renders_deterministically() {
        for c in CUES {
            let a = render(c.id, 44_100).unwrap();
            let b = render(c.id, 44_100).unwrap();
            assert_eq!(a.len(), b.len(), "cue {} not deterministic length", c.id);
            assert_eq!(a, b, "cue {} not deterministic", c.id);
            let lvl = lin_to_db(rms(&a));
            assert!(lvl > -60.0 && lvl < -3.0, "cue {} level {lvl} dBFS out of range", c.id);
        }
    }

    #[test]
    fn unknown_cue_is_none() {
        assert!(find("no_such_cue").is_none());
        assert!(render("no_such_cue", 44_100).is_none());
    }

    #[test]
    fn list_shape() {
        let v = list();
        assert_eq!(v["count"], json!(40));
        assert_eq!(v["cues"].as_array().unwrap().len(), 40);
    }
}
