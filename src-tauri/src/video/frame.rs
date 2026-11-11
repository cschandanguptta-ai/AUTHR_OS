// AUTHR_OS — Scripted video frame engine (contract v1.5, ADR-0011).
//
// The `SCRIPTED` image engine: a deterministic, fully-offline painter that is
// to the video pipeline what the scripted TTS engine is to the audio pipeline.
// Zero dependencies, zero network, byte-identical output for a given seed —
// the same book rendered twice produces the same pixels.
//
//   Surface/BMP   — minimal 24-bit BMP writer (BITMAPFILEHEADER + INFOHEADER)
//   FONT5X7       — embedded 5×7 column-bitmask font (A-Z 0-9 punctuation)
//   Palette       — SHA-256-seeded HSV-derived 4-color palette
//   render_scene  — 1280×720 chapter scene card (gradient, horizon glow,
//                   vignette, focal-safe composition, caption band)
//   render_sheet  — 1024×1024 R2I-style character reference sheet (3-view
//                   grid + palette swatches + name/role/bio band; the offline
//                   tier of the novelcast consistency ladder, ADR-0011)
//   render_title  — chapter title card
//
// Everything is a pure function of (seed bytes, geometry) — no wall-clock,
// no RNG state, no host info. Unit-tested for header legality, determinism
// and palette stability.

use sha2::{Digest, Sha256};
use std::path::Path;

pub const SCENE_W: u32 = 1280;
pub const SCENE_H: u32 = 720;
pub const SHEET_SIZE: u32 = 1024;

// ─────────────────────────────────────────────────────────────────────────────
// 5×7 column-bitmask font (bit 0 = top row). Uppercase + digits + punctuation.
// ─────────────────────────────────────────────────────────────────────────────

const FONT5X7: &[(&str, [u8; 5])] = &[
    ("A", [0x7C, 0x12, 0x11, 0x12, 0x7C]),
    ("B", [0x7F, 0x49, 0x49, 0x49, 0x36]),
    ("C", [0x3E, 0x41, 0x41, 0x41, 0x22]),
    ("D", [0x7F, 0x41, 0x41, 0x22, 0x1C]),
    ("E", [0x7F, 0x49, 0x49, 0x49, 0x41]),
    ("F", [0x7F, 0x09, 0x09, 0x09, 0x01]),
    ("G", [0x3E, 0x41, 0x49, 0x49, 0x7A]),
    ("H", [0x7F, 0x08, 0x08, 0x08, 0x7F]),
    ("I", [0x00, 0x41, 0x7F, 0x41, 0x00]),
    ("J", [0x20, 0x40, 0x41, 0x3F, 0x04]),
    ("K", [0x7F, 0x08, 0x14, 0x22, 0x41]),
    ("L", [0x7F, 0x40, 0x40, 0x40, 0x40]),
    ("M", [0x7F, 0x02, 0x0C, 0x02, 0x7F]),
    ("N", [0x7F, 0x04, 0x08, 0x10, 0x7F]),
    ("O", [0x3E, 0x41, 0x41, 0x41, 0x3E]),
    ("P", [0x7F, 0x09, 0x09, 0x09, 0x06]),
    ("Q", [0x3E, 0x41, 0x51, 0x21, 0x5E]),
    ("R", [0x7F, 0x09, 0x19, 0x29, 0x46]),
    ("S", [0x46, 0x49, 0x49, 0x49, 0x31]),
    ("T", [0x01, 0x01, 0x7F, 0x01, 0x01]),
    ("U", [0x3F, 0x40, 0x40, 0x40, 0x3F]),
    ("V", [0x1F, 0x20, 0x40, 0x20, 0x1F]),
    ("W", [0x3F, 0x40, 0x38, 0x40, 0x3F]),
    ("X", [0x63, 0x14, 0x08, 0x14, 0x63]),
    ("Y", [0x07, 0x08, 0x70, 0x08, 0x07]),
    ("Z", [0x61, 0x51, 0x49, 0x45, 0x43]),
    ("0", [0x3E, 0x51, 0x49, 0x45, 0x3E]),
    ("1", [0x00, 0x42, 0x7F, 0x40, 0x00]),
    ("2", [0x42, 0x61, 0x51, 0x49, 0x46]),
    ("3", [0x21, 0x41, 0x45, 0x4B, 0x31]),
    ("4", [0x18, 0x14, 0x12, 0x7F, 0x10]),
    ("5", [0x27, 0x45, 0x45, 0x45, 0x39]),
    ("6", [0x1C, 0x22, 0x51, 0x49, 0x06]),
    ("7", [0x01, 0x01, 0x31, 0x0D, 0x07]),
    ("8", [0x36, 0x49, 0x49, 0x49, 0x36]),
    ("9", [0x06, 0x49, 0x49, 0x29, 0x1E]),
    (" ", [0x00, 0x00, 0x00, 0x00, 0x00]),
    ("-", [0x00, 0x00, 0x08, 0x00, 0x00]),
    ("'", [0x00, 0x03, 0x03, 0x00, 0x00]),
    (".", [0x00, 0x40, 0x40, 0x00, 0x00]),
    (",", [0x00, 0x00, 0x40, 0x40, 0x20]),
    (":", [0x00, 0x24, 0x24, 0x00, 0x00]),
    ("!", [0x00, 0x00, 0x5F, 0x00, 0x00]),
    ("?", [0x02, 0x01, 0x51, 0x09, 0x06]),
    ("(", [0x08, 0x14, 0x22, 0x41, 0x00]),
    (")", [0x00, 0x41, 0x22, 0x14, 0x08]),
    ("/", [0x70, 0x38, 0x1C, 0x0E, 0x07]),
    ("&", [0x08, 0x14, 0x2A, 0x15, 0x08]),
];

fn glyph(ch: char) -> [u8; 5] {
    let up = ch.to_ascii_uppercase();
    for (c, cols) in FONT5X7 {
        if c.starts_with(up) {
            return *cols;
        }
    }
    // Unknown characters render as a light dash so nothing silently vanishes.
    [0x00, 0x00, 0x08, 0x00, 0x00]
}

/// Draw `text` with its top-left at (x, y). Advances 6 px per glyph × scale.
/// Clips silently at the surface edges (safe-area bands guarantee room).
pub fn draw_text(s: &mut Surface, x: i64, y: i64, text: &str, color: [u8; 3], scale: u32) {
    let mut cx = x;
    for ch in text.chars() {
        let cols = glyph(ch);
        for (ci, byte) in cols.iter().enumerate() {
            for row in 0..7u32 {
                if byte & (1 << row) != 0 {
                    for sx in 0..scale {
                        for sy in 0..scale {
                            s.put(cx + (ci as u32 * scale + sx) as i64, y + (row * scale + sy) as i64, color);
                        }
                    }
                }
            }
        }
        cx += 6 * scale as i64;
    }
}

pub fn text_width(text: &str, scale: u32) -> u32 {
    (text.chars().count() as u32 * 6).saturating_sub(1) * scale
}

// ─────────────────────────────────────────────────────────────────────────────
// Surface + pixels
// ─────────────────────────────────────────────────────────────────────────────

/// RGB surface, row-major, top-down (converted to bottom-up BGR on write).
pub struct Surface {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Surface {
    pub fn new(w: u32, h: u32) -> Surface {
        Surface { w, h, px: vec![0; (w * h * 3) as usize] }
    }
    #[inline]
    pub fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let idx = ((y as u32 * self.w + x as u32) * 3) as usize;
        self.px[idx] = c[0];
        self.px[idx + 1] = c[1];
        self.px[idx + 2] = c[2];
    }
    pub fn fill(&mut self, c: [u8; 3]) {
        for i in (0..self.px.len()).step_by(3) {
            self.px[i] = c[0];
            self.px[i + 1] = c[1];
            self.px[i + 2] = c[2];
        }
    }
    /// Vertical gradient between two colors (y=0 → top).
    pub fn v_gradient(&mut self, top: [u8; 3], bottom: [u8; 3]) {
        let last = self.h.saturating_sub(1) as f64;
        for y in 0..self.h {
            let t = if last > 0.0 { y as f64 / last } else { 0.0 };
            let c = lerp(top, bottom, t);
            for x in 0..self.w {
                self.put(x as i64, y as i64, c);
            }
        }
    }
    /// Filled circle (head/shoulders silhouettes, glow dots).
    pub fn circle(&mut self, cx: i64, cy: i64, r: i64, c: [u8; 3]) {
        if r <= 0 {
            return;
        }
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f64).sqrt().floor() as i64;
            for dx in -half..=half {
                self.put(cx + dx, cy + dy, c);
            }
        }
    }
    /// Rectangular fill (bands, rules).
    pub fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }
}

#[inline]
pub fn put(px: &mut [u8], w: u32, h: u32, x: i64, y: i64, c: [u8; 3]) {
    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
        return;
    }
    let idx = ((y as u32 * w + x as u32) * 3) as usize;
    px[idx] = c[0];
    px[idx + 1] = c[1];
    px[idx + 2] = c[2];
}

#[inline]
fn lerp(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f64 + (b[0] as f64 - a[0] as f64) * t).round() as u8,
        (a[1] as f64 + (b[1] as f64 - a[1] as f64) * t).round() as u8,
        (a[2] as f64 + (b[2] as f64 - a[2] as f64) * t).round() as u8,
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Palette — SHA-256-seeded, HSV-derived
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: [u8; 3],
    pub band: [u8; 3],
    pub accent: [u8; 3],
    pub fg: [u8; 3],
}

pub fn hsv(h_deg: f64, s: f64, v: f64) -> [u8; 3] {
    let h = ((h_deg.rem_euclid(360.0)) / 60.0) as i64;
    let f = h_deg.rem_euclid(60.0) / 60.0;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match h {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8]
}

/// Deterministic palette from arbitrary seed material (character key, scene
/// coordinates). Same seed → same palette, different seeds diverge.
pub fn palette_for(seed: &[u8]) -> Palette {
    let d = Sha256::digest(seed);
    let h1 = ((d[0] as u32) | ((d[1] as u32) << 8)) % 360;
    let accent_h = (h1 + 140 + (d[2] as u32) % 60) % 360;
    let glow_h = (h1 + 40 + (d[3] as u32) % 40) % 360;
    Palette {
        bg: hsv(h1 as f64, 0.42 + (d[4] % 12) as f64 / 100.0, 0.11 + (d[5] % 5) as f64 / 100.0),
        band: hsv(glow_h as f64, 0.50, 0.16 + (d[6] % 4) as f64 / 100.0),
        accent: hsv(accent_h as f64, 0.62, 0.58),
        fg: hsv(h1 as f64, 0.05, 0.93),
    }
}

pub fn darken(c: [u8; 3], k: f64) -> [u8; 3] {
    [(c[0] as f64 * k) as u8, (c[1] as f64 * k) as u8, (c[2] as f64 * k) as u8]
}

// ─────────────────────────────────────────────────────────────────────────────
// Renderers
// ─────────────────────────────────────────────────────────────────────────────

/// Chapter scene card: seeded gradient, horizon glow band, vignette, caption
/// band (scene number + speaker) and chapter tag.
pub fn render_scene(seed: &[u8], scene_index: usize, scene_total: usize, label: &str, tag: &str) -> Surface {
    let p = palette_for(seed);
    let mut s = Surface::new(SCENE_W, SCENE_H);
    s.v_gradient(p.bg, darken(p.bg, 0.55));

    // Horizon accent band at 62% height with a soft glow falloff above it.
    let band_y = (SCENE_H as f64 * 0.62) as i64;
    for glow in (1..=22).rev() {
        let c = lerp(darken(p.band, 0.8), darken(p.band, 0.2), (22 - glow) as f64 / 22.0);
        s.rect(0, band_y - glow, SCENE_W as i64, 1, c);
    }
    s.rect(0, band_y, SCENE_W as i64, (SCENE_H as f64 / 90.0) as i64, p.accent);

    // Vignette: radial darkening from center (storyboard painter signature).
    let cx = SCENE_W as f64 / 2.0;
    let cy = SCENE_H as f64 / 2.0;
    for y in 0..SCENE_H {
        for x in 0..SCENE_W {
            let dx = (x as f64 - cx) / cx;
            let dy = (y as f64 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt();
            let k = 1.0 - 0.45 * d * d;
            let idx = ((y * SCENE_W + x) * 3) as usize;
            s.px[idx] = (s.px[idx] as f64 * k).min(255.0) as u8;
            s.px[idx + 1] = (s.px[idx + 1] as f64 * k).min(255.0) as u8;
            s.px[idx + 2] = (s.px[idx + 2] as f64 * k).min(255.0) as u8;
        }
    }

    // Caption band (bottom) + chapter tag (top-right).
    let band_h = 92i64;
    s.rect(0, SCENE_H as i64 - band_h, SCENE_W as i64, band_h, darken(p.bg, 0.35));
    s.rect(0, SCENE_H as i64 - band_h, SCENE_W as i64, 2, p.accent);
    let caption = format!("SC {}/{} · {}", scene_index + 1, scene_total, label.to_uppercase());
    draw_text(&mut s, 48, SCENE_H as i64 - band_h + 26, &caption, p.fg, 3);
    let tag_up = tag.to_uppercase();
    draw_text(&mut s, (SCENE_W as i64 - text_width(&tag_up, 2) as i64 - 48).max(48), 40, &tag_up, p.fg, 2);
    s
}

/// R2I-style character reference sheet (offline tier): three-view silhouette
/// grid (FRONT / PROFILE / DETAIL geometry), palette swatch strip, name band
/// with role + verbatim bio excerpt. The bio rides along on every tier so the
/// consistency ladder degrades without losing the appearance contract
/// (novelcast ADR-0004 → ADR-0011 §ladder).
pub fn render_sheet(seed: &[u8], name: &str, role: &str, bio: &str) -> Surface {
    let p = palette_for(seed);
    let mut s = Surface::new(SHEET_SIZE, SHEET_SIZE);
    s.v_gradient(p.bg, darken(p.bg, 0.5));

    // Name band (top).
    s.rect(0, 0, SHEET_SIZE as i64, 170, darken(p.bg, 0.4));
    s.rect(0, 168, SHEET_SIZE as i64, 3, p.accent);
    draw_text(&mut s, 48, 40, &name.to_uppercase(), p.fg, 5);
    draw_text(&mut s, 48, 118, &role.to_uppercase(), p.accent, 2);
    let bio_line: String = bio.chars().take(52).collect();
    draw_text(&mut s, 48, 232, &bio_line.to_uppercase(), p.fg, 1);

    // Three-view grid: FRONT / PROFILE / DETAIL panels with silhouettes.
    let panel_y = 270i64;
    let panel_h = 520i64;
    let gutter = 24i64;
    let panel_w = (SHEET_SIZE as i64 - 2 * 48 - 2 * gutter) / 3;
    let views = ["FRONT", "PROFILE", "DETAIL"];
    for (i, view) in views.iter().enumerate() {
        let x0 = 48 + i as i64 * (panel_w + gutter);
        let shade = darken(palette_for(&[seed, view.as_bytes()].concat()).bg, 0.85);
        s.rect(x0, panel_y, panel_w, panel_h, shade);
        s.rect(x0, panel_y, panel_w, 2, p.accent);
        // Silhouette: head + shoulders, shifted per view for a multi-read feel.
        let hc_x = x0 + panel_w / 2 + if *view == "PROFILE" { panel_w / 6 } else { 0 };
        let hc_y = panel_y + panel_h / 3;
        s.circle(hc_x, hc_y, (panel_w as f64 * 0.16) as i64, darken(p.fg, 0.55));
        let sh_w = (panel_w as f64 * 0.42) as i64;
        s.rect(hc_x - sh_w / 2, hc_y + (panel_w as f64 * 0.22) as i64, sh_w, panel_h / 3, darken(p.fg, 0.55));
        draw_text(&mut s, x0 + 12, panel_y + panel_h - 40, view, p.accent, 2);
    }

    // Palette swatch strip (bottom).
    let sw_y = SHEET_SIZE as i64 - 150;
    let sw = 72i64;
    let colors = [p.bg, p.band, p.accent, p.fg, darken(p.accent, 0.6)];
    for (i, c) in colors.iter().enumerate() {
        s.rect(48 + i as i64 * (sw + 16), sw_y, sw, sw, *c);
    }
    draw_text(&mut s, 48, sw_y + sw + 18, "R2I REFERENCE SHEET · SCRIPTED OFFLINE TIER", darken(p.fg, 0.75), 1);
    s
}

/// Chapter title card.
pub fn render_title(seed: &[u8], book_title: &str, chapter_title: &str, chapter_number: i64) -> Surface {
    let p = palette_for(seed);
    let mut s = Surface::new(SCENE_W, SCENE_H);
    s.v_gradient(p.bg, darken(p.bg, 0.45));
    let rule_y = 250i64;
    s.rect(0, rule_y, SCENE_W as i64, 3, p.accent);

    let up = book_title.to_uppercase();
    draw_centered(&mut s, &up, 4, p.fg, rule_y + 90);
    let sub = format!("CHAPTER {} · {}", chapter_number, chapter_title.to_uppercase());
    draw_centered(&mut s, &sub, 2, p.accent, rule_y + 210);
    draw_centered(&mut s, "AUTHR_OS VIDEO NOVEL · SCRIPTED RENDER", 1, darken(p.fg, 0.7), SCENE_H as i64 - 80);
    s
}

fn draw_centered(s: &mut Surface, text: &str, scale: u32, c: [u8; 3], y: i64) {
    let tw = text_width(text, scale) as i64;
    draw_text(s, ((s.w as i64 - tw) / 2).max(16), y, text, c, scale);
}

// ─────────────────────────────────────────────────────────────────────────────
// BMP encoding — 24-bit, bottom-up BGR, rows padded to 4 bytes
// ─────────────────────────────────────────────────────────────────────────────

pub fn encode_bmp(s: &Surface) -> Vec<u8> {
    let w = s.w;
    let h = s.h;
    let row = (w * 3) as usize;
    let pad = (4 - (row % 4)) % 4;
    let stride = row + pad;
    let data_size = stride * h as usize;
    let file_size = 54 + data_size;
    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    let mut bgr = [0u8; 3];
    const PAD_BYTES: [u8; 3] = [0, 0, 0];
    for src_y in (0..h).rev() {
        for x in 0..w {
            let i = ((src_y * w + x) * 3) as usize;
            bgr[0] = s.px[i + 2]; // B
            bgr[1] = s.px[i + 1]; // G
            bgr[2] = s.px[i]; // R
            out.extend_from_slice(&bgr);
        }
        out.extend_from_slice(&PAD_BYTES[..pad]);
    }
    out
}

pub fn write_bmp(path: &Path, s: &Surface) -> std::io::Result<Vec<u8>> {
    let bytes = encode_bmp(s);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, &bytes)?;
    Ok(bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// Focal anchors (Ken Burns) — matches the VideoStudio focal selector
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the normalized focal point for a focal name. Unknown → center.
pub fn focal_point(focal: &str) -> (f64, f64) {
    match focal {
        "left" => (0.33, 0.5),
        "right" => (0.66, 0.5),
        "top" => (0.5, 0.33),
        "bottom" => (0.5, 0.66),
        _ => (0.5, 0.5),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bmp_size(w: u32, h: u32) -> usize {
        let row = (w * 3) as usize;
        let stride = row + (4 - (row % 4)) % 4;
        54 + stride * h as usize
    }

    #[test]
    fn bmp_header_is_legal_and_sized() {
        let s = render_scene(b"seed-a", 0, 5, "KIRA", "CH 1 · THE HARBOR");
        let bmp = encode_bmp(&s);
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()) as usize, bmp.len());
        assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
        assert_eq!(u32::from_le_bytes(bmp[14..18].try_into().unwrap()), 40);
        assert_eq!(bmp.len(), bmp_size(SCENE_W, SCENE_H));
    }

    #[test]
    fn scene_render_is_deterministic_and_seed_sensitive() {
        let a = encode_bmp(&render_scene(b"seed-a", 0, 5, "KIRA", "CH 1"));
        let a2 = encode_bmp(&render_scene(b"seed-a", 0, 5, "KIRA", "CH 1"));
        let b = encode_bmp(&render_scene(b"seed-b", 0, 5, "KIRA", "CH 1"));
        assert_eq!(a, a2, "same seed must reproduce identical bytes");
        assert_ne!(a[..400], b[..400], "different seeds must diverge");
    }

    #[test]
    fn sheet_is_square_and_view_sensitive() {
        let s = render_sheet(b"char-kira", "KIRA", "PROTAGONIST", "gray coat, sea-glass eyes");
        let bmp = encode_bmp(&s);
        assert_eq!(s.w, SHEET_SIZE);
        assert_eq!(s.h, SHEET_SIZE);
        assert_eq!(bmp.len(), bmp_size(SHEET_SIZE, SHEET_SIZE));
        let front = palette_for(b"char-kiraFRONT");
        let profile = palette_for(b"char-kiraPROFILE");
        assert_ne!(front.bg, profile.bg, "views must be visually distinct");
    }

    #[test]
    fn palette_is_stable_and_bounded() {
        let p1 = palette_for(b"stable-seed");
        let p2 = palette_for(b"stable-seed");
        assert_eq!(p1, p2);
        // Background is intentionally dark; foreground intentionally bright —
        // the caption/title text always has contrast.
        let bg_sum: u32 = p1.bg.iter().map(|v| *v as u32).sum();
        let fg_sum: u32 = p1.fg.iter().map(|v| *v as u32).sum();
        assert!(bg_sum < 500, "background must stay dark (sum {bg_sum})");
        assert!(fg_sum > 600, "foreground must stay bright (sum {fg_sum})");
    }

    #[test]
    fn text_drawing_clips_at_edges() {
        let mut s = Surface::new(40, 20);
        // Far off-canvas text must not panic and must not write out of bounds.
        draw_text(&mut s, 38, 0, "OVERFLOW!!!!!", [255, 0, 0], 2);
        let _ = encode_bmp(&s);
    }

    #[test]
    fn focal_points_match_video_studio_selectors() {
        assert_eq!(focal_point("center"), (0.5, 0.5));
        assert_eq!(focal_point("left"), (0.33, 0.5));
        assert_eq!(focal_point("right"), (0.66, 0.5));
        assert_eq!(focal_point("top"), (0.5, 0.33));
        assert_eq!(focal_point("bottom"), (0.5, 0.66));
        assert_eq!(focal_point("anything-else"), (0.5, 0.5));
    }
}
