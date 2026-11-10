// AUTHR_OS — Layer 3: procedural Ambient Bed generators (build plan §5.4).
// Beds are loopable: an 8 s loop is made seamless with the spec's 2.5 s loop
// crossfade, then tiled for the chapter length. Fully deterministic.

use crate::audio::dsp::{fnv64, lin_to_db, lowpass, Biquad, Rng};
use serde_json::json;

pub const LOOP_SECONDS: usize = 8;
pub const LOOP_CROSSFADE_S: f64 = 2.5;

pub const BEDS: &[&str] = &[
    "room_tone",
    "cyberpunk_rain_heavy",
    "rain_medium",
    "rain_window",
    "wind_gentle",
    "wind_howling",
    "desert_wind",
    "engine_hum",
    "station_hum",
    "server_room",
    "city_distant",
    "traffic_light",
    "ocean_waves",
    "waves_calm",
    "forest_night",
    "crickets_field",
    "birds_dawn",
    "cave_drip",
    "crowd_murmur",
    "spaceship_bridge",
];

pub fn find(id: &str) -> bool {
    BEDS.contains(&id)
}

/// Render `total` samples of a seamless ambient bed at `fs`.
/// `raw_loop` already crossfades tail→head (2.5 s), so straight tiling is
/// click-free; the bed is then level-normalized with short edge fades.
pub fn render_loop(id: &str, fs: u32, total: usize, level_dbfs: f64) -> Vec<f32> {
    let loop_len = LOOP_SECONDS * fs as usize;
    let one = raw_loop(id, fs, loop_len);
    let mut bed = Vec::with_capacity(total);
    while bed.len() < total {
        let take = (total - bed.len()).min(one.len());
        bed.extend_from_slice(&one[..take]);
    }
    bed.truncate(total);
    // Target level + short edge fades so the bed starts/ends transparently.
    let cur = bed_rms(&bed);
    let gain = if cur > 1e-7 { crate::audio::dsp::db_to_lin(level_dbfs - lin_to_db(cur)) } else { 0.0 };
    let fade = (fs as f64 * 0.15) as usize;
    let mut out = Vec::with_capacity(total);
    for (i, &s) in bed.iter().enumerate() {
        let mut v = (s as f64 * gain) as f32;
        if i < fade {
            v *= i as f32 / fade as f32;
        }
        if total.saturating_sub(i) < fade {
            v *= (total - i) as f32 / fade as f32;
        }
        out.push(v);
    }
    out
}

fn bed_rms(x: &[f32]) -> f64 {
    if x.is_empty() { return 0.0; }
    (x.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / x.len() as f64).sqrt()
}

/// One 8 s seamless loop (crossfaded tail→head) before level normalization.
fn raw_loop(id: &str, fs: u32, n: usize) -> Vec<f32> {
    let seed = fnv64(id.as_bytes());
    let mut rng = Rng::new(seed);
    let mut x = vec![0.0f32; n];
    match id {
        "room_tone" => {
            let mut lp = lowpass(fs, 380.0, 0.707);
            for v in x.iter_mut() {
                *v = (lp.process(rng.bipolar()) * 0.8) as f32;
            }
        }
        "cyberpunk_rain_heavy" | "rain_medium" | "rain_window" => {
            let heaviness = match id { "cyberpunk_rain_heavy" => 3, "rain_medium" => 2, _ => 1 };
            let mut lp = lowpass(fs, 900.0 + 350.0 * heaviness as f64, 0.707);
            let mut hp = Biquad::highpass(fs, 120.0, 0.707);
            for v in x.iter_mut() {
                *v = hp.process(lp.process(rng.bipolar())) as f32 * 0.7;
            }
            // droplets: short high-frequency taps
            let drops = heaviness * (fs as usize / 800);
            for _ in 0..drops {
                let at = rng.next_f64() * (n as f64 - 200.0);
                let at = at as usize;
                for i in at..(at + 120).min(n) {
                    let e = 1.0 - (i - at) as f64 / 120.0;
                    x[i] += (rng.bipolar() * e * 0.45) as f32;
                }
            }
        }
        "wind_gentle" | "wind_howling" | "desert_wind" => {
            let howl = matches!(id, "wind_howling");
            let cutoff = match id { "wind_gentle" => 500.0, "wind_howling" => 900.0, _ => 700.0 };
            let mut lp = lowpass(fs, cutoff, 0.707);
            let mut lp_hi = lowpass(fs, cutoff * 1.8, 0.707);
            for (i, v) in x.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                let lfo = 0.45 + 0.55 * (2.0 * std::f64::consts::PI * 0.13 * t).sin();
                let s = lp.process(rng.bipolar()) * (1.0 - 0.5 * lfo)
                    + lp_hi.process(rng.bipolar()) * (0.5 * lfo);
                let tone = if howl {
                    0.14 * (2.0 * std::f64::consts::PI * 310.0 * t).sin()
                        * (0.5 + 0.5 * (2.0 * std::f64::consts::PI * 0.07 * t).sin())
                } else {
                    0.0
                };
                *v = ((s + tone) * 0.8) as f32;
            }
        }
        "engine_hum" | "station_hum" | "server_room" | "spaceship_bridge" => {
            let f0 = match id { "engine_hum" => 60.0, "station_hum" => 84.0, "server_room" => 120.0, _ => 48.0 };
            let mut lp = lowpass(fs, 2400.0, 0.707);
            for (i, v) in x.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                let s = (2.0 * std::f64::consts::PI * f0 * t).sin() * 0.55
                    + (2.0 * std::f64::consts::PI * f0 * 2.0 * t).sin() * 0.22
                    + (2.0 * std::f64::consts::PI * f0 * 3.01 * t).sin() * 0.10
                    + lp.process(rng.bipolar()) * 0.10;
                *v = s as f32 * 0.85;
            }
        }
        "city_distant" | "traffic_light" | "crowd_murmur" => {
            let mut bp = lowpass(fs, if id == "crowd_murmur" { 1400.0 } else { 800.0 }, 0.707);
            let mut hp = Biquad::highpass(fs, 90.0, 0.707);
            let mut phase = 0.0f64;
            for (i, v) in x.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                let lfo = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 0.09 * t).sin()
                    + 0.15 * (2.0 * std::f64::consts::PI * 0.31 * t).sin();
                let s = hp.process(bp.process(rng.bipolar())) * lfo;
                phase += 2.0 * std::f64::consts::PI * 55.0 / fs as f64;
                *v = (s + 0.06 * phase.sin()) as f32 * 0.8;
            }
        }
        "ocean_waves" | "waves_calm" => {
            let strength = matches!(id, "ocean_waves");
            let mut lp = lowpass(fs, 650.0, 0.707);
            let mut hp = Biquad::highpass(fs, 60.0, 0.707);
            for (i, v) in x.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                let swell = 0.5 + 0.5 * (2.0 * std::f64::consts::PI * 0.11 * t).sin()
                    .powi(2) * if strength { 1.0 } else { 0.5 };
                let s = hp.process(lp.process(rng.bipolar())) * (0.25 + swell);
                *v = (s * 0.8) as f32;
            }
        }
        "forest_night" | "crickets_field" | "birds_dawn" => {
            // ambient floor + rhythmic wildlife pulses
            let mut lp = lowpass(fs, 500.0, 0.707);
            for (i, v) in x.iter_mut().enumerate() {
                let t = i as f64 / fs as f64;
                *v = (lp.process(rng.bipolar()) * 0.5) as f32;
                let _ = t;
            }
            let period = match id { "birds_dawn" => 1.7, _ => 0.9 };
            let mut t = 0.4;
            while (t * fs as f64) < n as f64 {
                let at = (t * fs as f64) as usize;
                let chirps = 3 + (rng.next_u64() % 3) as usize;
                for c in 0..chirps {
                    let start = at + c * (fs as usize / 14);
                    for i in start..(start + fs as usize / 40).min(n) {
                        let e = 1.0 - (i - start) as f64 / (fs as f64 / 40.0);
                        x[i] += (rng.bipolar() * e * 0.35) as f32;
                    }
                }
                t += period * (0.7 + rng.next_f64() * 0.6);
            }
        }
        "cave_drip" => {
            let mut lp = lowpass(fs, 300.0, 0.707);
            for v in x.iter_mut() {
                *v = (lp.process(rng.bipolar()) * 0.5) as f32;
            }
            let mut t = 0.6;
            while (t * fs as f64) < n as f64 {
                let at = (t * fs as f64) as usize;
                let w = fs as usize / 90;
                for i in at..(at + w).min(n) {
                    let p = (i - at) as f64 / w as f64;
                    let f = 1500.0 - 600.0 * p;
                    let e = (-4.0 * p).exp();
                    x[i] += (f.sin() * e * 0.4) as f32;
                }
                t += 1.2 + rng.next_f64() * 1.8;
            }
        }
        _ => {
            for v in x.iter_mut() {
                *v = (rng.bipolar() * 0.3) as f32;
            }
        }
    }
    make_seamless(&mut x, fs);
    x
}

/// Crossfade tail into head and truncate: the loop's first LOOP_CROSSFADE_S
/// seconds blend out of the tail, so wrap-around is continuous by construction.
fn make_seamless(x: &mut Vec<f32>, fs: u32) {
    let fade = (LOOP_CROSSFADE_S * fs as f64) as usize;
    if fade == 0 || x.len() <= fade + 1 {
        return;
    }
    let n = x.len();
    let keep = n - fade;
    for i in 0..fade {
        let w = i as f64 / fade as f64; // 0 → 1 across the overlap
        let tail = x[keep + i] as f64;
        let head = x[i] as f64;
        x[i] = (head * w + tail * (1.0 - w)) as f32;
    }
    x.truncate(keep);
}

pub fn list() -> serde_json::Value {
    json!({
        "beds": BEDS,
        "count": BEDS.len(),
        "loop_seconds": LOOP_SECONDS,
        "loop_crossfade_s": LOOP_CROSSFADE_S,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beds_are_seamless_and_levelled() {
        let fs = 44_100u32;
        for id in BEDS {
            let total = 3 * LOOP_SECONDS * fs as usize; // 24 s bed
            let bed = render_loop(id, fs, total, -40.0);
            assert_eq!(bed.len(), total, "{id} length");
            // seam check: sample discontinuity at the loop wrap must be small
            let seam_at = LOOP_SECONDS * fs as usize
                - (LOOP_CROSSFADE_S * fs as f64) as usize;
            let d = (bed[seam_at] - bed[seam_at - 1]).abs();
            assert!(d < 0.02, "{id} click at loop seam: {d}");
            let lvl = lin_to_db(bed_rms(&bed));
            assert!((lvl - (-40.0)).abs() < 1.0, "{id} level {lvl} vs -40");
        }
    }

    #[test]
    fn room_tone_is_quiet() {
        let bed = render_loop("room_tone", 44_100, 44_100, -60.0);
        assert!(lin_to_db(bed_rms(&bed)) < -58.0);
    }

    #[test]
    fn determinism() {
        let a = render_loop("engine_hum", 24_000, 48_000, -40.0);
        let b = render_loop("engine_hum", 24_000, 48_000, -40.0);
        assert_eq!(a, b);
    }

    #[test]
    fn list_shape() {
        let v = list();
        assert_eq!(v["count"], json!(BEDS.len()));
    }
}
